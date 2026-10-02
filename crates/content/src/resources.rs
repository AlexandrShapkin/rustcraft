//! Generic authored image discovery and deterministic runtime atlas compilation.

use crate::{ContentHash, PackageId, ResourceId};
use std::{
    collections::{BTreeMap, HashMap},
    fmt, fs,
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const CACHE_MAGIC: &[u8; 8] = b"RCATL\0\x01\0";
const COMPILER_VERSION: u32 = 2;
const CACHE_DOMAIN: &str = "rustcraft.resource-atlas-cache.v1";
static CACHE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SamplerPolicy {
    Nearest,
    Linear,
}

impl SamplerPolicy {
    const fn code(self) -> u8 {
        match self {
            Self::Nearest => 0,
            Self::Linear => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureSource {
    pub path: PathBuf,
    pub crop: Option<PixelRect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredTexture {
    pub id: ResourceId,
    pub source: TextureSource,
    pub sampler: SamplerPolicy,
}

#[derive(Debug, Clone)]
pub struct ResourcePackage {
    pub id: PackageId,
    root: PathBuf,
    textures: Vec<AuthoredTexture>,
    discovery_time: Duration,
}

impl ResourcePackage {
    #[must_use]
    pub fn new(id: PackageId, root: impl Into<PathBuf>) -> Self {
        Self {
            id,
            root: root.into(),
            textures: Vec::new(),
            discovery_time: Duration::ZERO,
        }
    }

    pub fn add_texture(
        &mut self,
        id: ResourceId,
        relative_path: impl AsRef<Path>,
        crop: Option<PixelRect>,
        sampler: SamplerPolicy,
    ) -> Result<(), ResourceError> {
        let relative_path = relative_path.as_ref();
        validate_relative_path(relative_path).map_err(|cause| ResourceError::InvalidPath {
            package: self.id.clone(),
            path: relative_path.to_path_buf(),
            cause,
        })?;
        if self.textures.iter().any(|texture| texture.id == id) {
            return Err(ResourceError::DuplicateInPackage {
                id,
                package: self.id.clone(),
            });
        }
        self.textures.push(AuthoredTexture {
            id,
            source: TextureSource {
                path: self.root.join(relative_path),
                crop,
            },
            sampler,
        });
        Ok(())
    }

    #[must_use]
    pub fn textures(&self) -> &[AuthoredTexture] {
        &self.textures
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResourceLimits {
    pub max_files: usize,
    pub max_source_bytes: u64,
    pub max_dimension: u32,
    pub max_decoded_bytes: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_files: 16_384,
            max_source_bytes: 256 * 1024 * 1024,
            max_dimension: 16_384,
            max_decoded_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasPolicy {
    pub page_width: u32,
    pub page_height: u32,
    pub max_pages: u32,
    pub padding: u32,
    pub sampler: SamplerPolicy,
    pub mip_levels: u8,
    pub anisotropy: u16,
}

impl Default for AtlasPolicy {
    fn default() -> Self {
        Self {
            page_width: 1024,
            page_height: 1024,
            max_pages: 64,
            padding: 2,
            sampler: SamplerPolicy::Nearest,
            mip_levels: 1,
            anisotropy: 1,
        }
    }
}

impl AtlasPolicy {
    fn validate(self) -> Result<Self, ResourceError> {
        if self.page_width == 0
            || self.page_height == 0
            || self.max_pages == 0
            || self.mip_levels == 0
            || !self.page_width.is_power_of_two()
            || !self.page_height.is_power_of_two()
        {
            return Err(ResourceError::InvalidAtlasPolicy);
        }
        Ok(self)
    }

    /// Treat this policy's page dimensions as maxima and clamp them to a device-safe power of two.
    pub fn bounded_by_device_dimension(mut self, maximum: u32) -> Result<Self, ResourceError> {
        if maximum == 0 {
            return Err(ResourceError::InvalidAtlasPolicy);
        }
        let safe = 1 << (31 - maximum.leading_zeros());
        self.page_width = self.page_width.min(safe);
        self.page_height = self.page_height.min(safe);
        self.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceOverride {
    pub id: ResourceId,
    pub original_provider: PackageId,
    pub overriding_provider: PackageId,
}

#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct AtlasPage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub sampler: SamplerPolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledTexture {
    pub id: ResourceId,
    pub provider: PackageId,
    pub source_path: PathBuf,
    pub source_hash: ContentHash,
    pub page: u32,
    pub pixel_rect: PixelRect,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub padding: u32,
    pub sampler: SamplerPolicy,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceMetrics {
    pub discovery_time: Duration,
    pub hashing_time: Duration,
    pub decode_time: Duration,
    pub legacy_extraction_time: Duration,
    pub packing_time: Duration,
    pub cache_read_time: Duration,
    pub total_time: Duration,
    pub resource_count: usize,
    pub source_bytes: u64,
    pub decoded_rgba_bytes: u64,
    pub atlas_bytes: u64,
    /// Estimated device storage for the current uncompressed RGBA8 atlas format, not measured VRAM.
    pub gpu_format_estimated_atlas_bytes: u64,
    pub page_count: usize,
    pub occupancy: f32,
    pub cache_hit: bool,
    pub cache_write_attempted: bool,
    pub cache_write_succeeded: bool,
    pub cache_write_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompiledResources {
    pub pages: Vec<AtlasPage>,
    pub textures: Vec<CompiledTexture>,
    pub overrides: Vec<ResourceOverride>,
    pub metrics: ResourceMetrics,
    pub cache_key: ContentHash,
    by_id: HashMap<ResourceId, usize>,
}

impl CompiledResources {
    #[must_use]
    pub fn texture(&self, id: &ResourceId) -> Option<&CompiledTexture> {
        self.by_id.get(id).map(|index| &self.textures[*index])
    }

    #[must_use]
    pub fn report(&self) -> String {
        format!(
            "resources={} pages={} atlas={}x{} occupancy={:.1}% source_compressed_bytes={} decoded_rgba_bytes={} compiled_atlas_rgba_bytes={} gpu_format_estimated_atlas_bytes={} cache={} cache_write={} discovery_ms={:.3} hash_ms={:.3} decode_ms={:.3} extract_ms={:.3} pack_ms={:.3} total_ms={:.3}",
            self.metrics.resource_count,
            self.metrics.page_count,
            self.pages.first().map_or(0, |page| page.width),
            self.pages.first().map_or(0, |page| page.height),
            self.metrics.occupancy * 100.0,
            self.metrics.source_bytes,
            self.metrics.decoded_rgba_bytes,
            self.metrics.atlas_bytes,
            self.metrics.gpu_format_estimated_atlas_bytes,
            if self.metrics.cache_hit {
                "hit"
            } else {
                "miss"
            },
            if self.metrics.cache_write_succeeded {
                "ok"
            } else if self.metrics.cache_write_attempted {
                "failed"
            } else {
                "not-attempted"
            },
            self.metrics.discovery_time.as_secs_f64() * 1000.0,
            self.metrics.hashing_time.as_secs_f64() * 1000.0,
            self.metrics.decode_time.as_secs_f64() * 1000.0,
            self.metrics.legacy_extraction_time.as_secs_f64() * 1000.0,
            self.metrics.packing_time.as_secs_f64() * 1000.0,
            self.metrics.total_time.as_secs_f64() * 1000.0,
        )
    }
}

#[derive(Debug)]
pub enum ResourceError {
    Io {
        path: PathBuf,
        cause: String,
    },
    InvalidPath {
        package: PackageId,
        path: PathBuf,
        cause: &'static str,
    },
    InvalidResourcePath {
        path: PathBuf,
        cause: String,
    },
    UnsupportedExtension {
        path: PathBuf,
    },
    DuplicateInPackage {
        id: ResourceId,
        package: PackageId,
    },
    LimitExceeded {
        resource: Option<ResourceId>,
        cause: String,
    },
    Decode {
        resource: ResourceId,
        provider: PackageId,
        path: PathBuf,
        cause: String,
    },
    InvalidCrop {
        resource: ResourceId,
        provider: PackageId,
        path: PathBuf,
        crop: PixelRect,
    },
    InvalidAtlasPolicy,
    Packing {
        resource: ResourceId,
        cause: String,
    },
    UnsupportedPolicy(String),
}

impl fmt::Display for ResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, cause } => {
                write!(formatter, "resource I/O {}: {cause}", path.display())
            }
            Self::InvalidPath {
                package,
                path,
                cause,
            } => write!(
                formatter,
                "invalid path {} in {package}: {cause}",
                path.display()
            ),
            Self::InvalidResourcePath { path, cause } => write!(
                formatter,
                "invalid resource path {}: {cause}",
                path.display()
            ),
            Self::UnsupportedExtension { path } => write!(
                formatter,
                "unsupported resource extension: {} (only PNG is accepted)",
                path.display()
            ),
            Self::DuplicateInPackage { id, package } => {
                write!(formatter, "duplicate resource {id} in package {package}")
            }
            Self::LimitExceeded { resource, cause } => match resource {
                Some(id) => write!(formatter, "resource {id} exceeds safety limit: {cause}"),
                None => write!(formatter, "resource package exceeds safety limit: {cause}"),
            },
            Self::Decode {
                resource,
                provider,
                path,
                cause,
            } => write!(
                formatter,
                "decode {resource} from {provider} at {}: {cause}",
                path.display()
            ),
            Self::InvalidCrop {
                resource,
                provider,
                path,
                crop,
            } => write!(
                formatter,
                "invalid crop {crop:?} for {resource} from {provider} at {}",
                path.display()
            ),
            Self::InvalidAtlasPolicy => formatter.write_str("invalid atlas policy"),
            Self::Packing { resource, cause } => {
                write!(formatter, "cannot pack {resource}: {cause}")
            }
            Self::UnsupportedPolicy(cause) => {
                write!(formatter, "unsupported texture policy: {cause}")
            }
        }
    }
}

impl std::error::Error for ResourceError {}

pub fn discover_package(
    root: impl AsRef<Path>,
    package: PackageId,
    limits: ResourceLimits,
) -> Result<ResourcePackage, ResourceError> {
    let started = Instant::now();
    let root = root.as_ref().to_path_buf();
    let assets = root.join("assets");
    let mut files = Vec::new();
    collect_files(&assets, &mut files, limits.max_files)?;
    files.sort();
    let mut result = ResourcePackage::new(package.clone(), &root);
    for path in files {
        let relative =
            path.strip_prefix(&assets)
                .map_err(|_| ResourceError::InvalidResourcePath {
                    path: path.clone(),
                    cause: "resource escaped assets root".into(),
                })?;
        let mut components = relative.components();
        let namespace = normal_component(components.next(), &path, "missing namespace")?;
        let class = normal_component(components.next(), &path, "missing resource class")?;
        if class != "textures" {
            // Other resource classes are reserved for future compilers and must not make a
            // texture-only R1.1 package invalid.
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("png") {
            return Err(ResourceError::UnsupportedExtension { path });
        }
        let remainder = components
            .map(|component| normal_component(Some(component), &path, "invalid path component"))
            .collect::<Result<Vec<_>, _>>()?;
        if remainder.is_empty() {
            return Err(ResourceError::InvalidResourcePath {
                path,
                cause: "texture path is empty".into(),
            });
        }
        let mut semantic_path = remainder.join("/");
        semantic_path.truncate(semantic_path.len().saturating_sub(4));
        let id = ResourceId::parse(format!("{namespace}:textures/{semantic_path}")).map_err(
            |error| ResourceError::InvalidResourcePath {
                path: path.clone(),
                cause: error.to_string(),
            },
        )?;
        let relative_to_root =
            path.strip_prefix(&root)
                .map_err(|_| ResourceError::InvalidResourcePath {
                    path: path.clone(),
                    cause: "resource escaped package root".into(),
                })?;
        result.add_texture(id, relative_to_root, None, SamplerPolicy::Nearest)?;
    }
    result.discovery_time = started.elapsed();
    Ok(result)
}

fn normal_component<'a>(
    component: Option<Component<'a>>,
    path: &Path,
    cause: &'static str,
) -> Result<&'a str, ResourceError> {
    match component {
        Some(Component::Normal(value)) => {
            value
                .to_str()
                .ok_or_else(|| ResourceError::InvalidResourcePath {
                    path: path.to_path_buf(),
                    cause: "path is not UTF-8".into(),
                })
        }
        _ => Err(ResourceError::InvalidResourcePath {
            path: path.to_path_buf(),
            cause: cause.into(),
        }),
    }
}

fn collect_files(
    root: &Path,
    files: &mut Vec<PathBuf>,
    max_files: usize,
) -> Result<(), ResourceError> {
    let entries = fs::read_dir(root).map_err(|error| ResourceError::Io {
        path: root.to_path_buf(),
        cause: error.to_string(),
    })?;
    let mut paths = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| ResourceError::Io {
            path: root.to_path_buf(),
            cause: error.to_string(),
        })?;
    paths.sort();
    for path in paths {
        let metadata = fs::symlink_metadata(&path).map_err(|error| ResourceError::Io {
            path: path.clone(),
            cause: error.to_string(),
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ResourceError::InvalidResourcePath {
                path,
                cause: "symbolic links are not accepted in resource packages".into(),
            });
        }
        if metadata.is_dir() {
            collect_files(&path, files, max_files)?;
        } else if metadata.is_file() {
            files.push(path);
            if files.len() > max_files {
                return Err(ResourceError::LimitExceeded {
                    resource: None,
                    cause: format!("more than {max_files} files"),
                });
            }
        }
    }
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<(), &'static str> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err("path must be nonempty and package-relative");
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("path traversal and special components are forbidden");
    }
    Ok(())
}

#[derive(Clone)]
struct ResolvedTexture {
    texture: AuthoredTexture,
    provider: PackageId,
    source_hash: ContentHash,
}

pub fn compile_resources(
    packages: &[ResourcePackage],
    policy: AtlasPolicy,
    limits: ResourceLimits,
    cache_directory: Option<&Path>,
) -> Result<CompiledResources, ResourceError> {
    let total_started = Instant::now();
    let mut policy = policy.validate()?;
    let mut metrics = ResourceMetrics {
        discovery_time: packages.iter().map(|package| package.discovery_time).sum(),
        ..ResourceMetrics::default()
    };
    let mut resolved = BTreeMap::<ResourceId, (PackageId, AuthoredTexture)>::new();
    let mut overrides = Vec::new();
    for package in packages {
        for texture in &package.textures {
            if let Some((original, _)) =
                resolved.insert(texture.id.clone(), (package.id.clone(), texture.clone()))
            {
                overrides.push(ResourceOverride {
                    id: texture.id.clone(),
                    original_provider: original,
                    overriding_provider: package.id.clone(),
                });
            }
        }
    }
    if resolved.len() > limits.max_files {
        return Err(ResourceError::LimitExceeded {
            resource: None,
            cause: format!(
                "{} resolved resources exceeds {}",
                resolved.len(),
                limits.max_files
            ),
        });
    }

    let hash_started = Instant::now();
    let mut blobs = BTreeMap::<PathBuf, Arc<[u8]>>::new();
    let mut source_bytes = 0_u64;
    for (_, texture) in resolved.values() {
        if blobs.contains_key(&texture.source.path) {
            continue;
        }
        let bytes = fs::read(&texture.source.path).map_err(|error| ResourceError::Io {
            path: texture.source.path.clone(),
            cause: error.to_string(),
        })?;
        source_bytes = source_bytes.saturating_add(bytes.len() as u64);
        if source_bytes > limits.max_source_bytes {
            return Err(ResourceError::LimitExceeded {
                resource: None,
                cause: format!("source bytes exceed {}", limits.max_source_bytes),
            });
        }
        blobs.insert(texture.source.path.clone(), Arc::from(bytes));
    }
    let resolved = resolved
        .into_iter()
        .map(|(_, (provider, texture))| {
            let source_bytes = blobs
                .get(&texture.source.path)
                .expect("source blob loaded")
                .clone();
            ResolvedTexture {
                source_hash: ContentHash::from_content_bytes(&source_bytes),
                texture,
                provider,
            }
        })
        .collect::<Vec<_>>();
    if policy.mip_levels != 1 || policy.anisotropy != 1 {
        return Err(ResourceError::UnsupportedPolicy(
            "R1.1 keeps mipmaps disabled and anisotropy at 1 until atlas-safe mip generation is implemented".into(),
        ));
    }
    if let Some(resource) = resolved
        .iter()
        .find(|resource| resource.texture.sampler != policy.sampler)
    {
        return Err(ResourceError::UnsupportedPolicy(format!(
            "{} requests {:?} but its physical page uses {:?}",
            resource.texture.id, resource.texture.sampler, policy.sampler
        )));
    }
    policy = select_page_size(policy, &blobs, &resolved, limits)?;
    metrics.hashing_time = hash_started.elapsed();
    metrics.source_bytes = source_bytes;
    metrics.resource_count = resolved.len();
    let cache_key = cache_key(&resolved, policy);

    if let Some(directory) = cache_directory {
        let cache_started = Instant::now();
        let path = directory.join(format!("{cache_key}.rcatlas"));
        if let Ok(Some((pages, placements))) = read_cache(&path, cache_key, policy, &resolved) {
            metrics.cache_read_time = cache_started.elapsed();
            metrics.cache_hit = true;
            metrics.page_count = pages.len();
            metrics.atlas_bytes = pages.iter().map(|page| page.rgba.len() as u64).sum();
            metrics.gpu_format_estimated_atlas_bytes = metrics.atlas_bytes;
            metrics.decoded_rgba_bytes = placements
                .iter()
                .map(|placement| u64::from(placement.width) * u64::from(placement.height) * 4)
                .sum();
            metrics.occupancy = occupancy(&placements, &pages);
            metrics.total_time = total_started.elapsed();
            return Ok(finish_compiled(
                pages, placements, resolved, overrides, metrics, cache_key,
            ));
        }
        metrics.cache_read_time = cache_started.elapsed();
    }

    let decode_started = Instant::now();
    let decoded_sources = decode_unique_sources(&blobs, &resolved, limits)?;
    metrics.decode_time = decode_started.elapsed();
    let extraction_started = Instant::now();
    let mut decoded = Vec::with_capacity(resolved.len());
    let mut decoded_bytes = 0_u64;
    for resource in &resolved {
        let source = decoded_sources
            .get(&resource.texture.source.path)
            .expect("decoded source exists");
        let image = crop_image(source, resource)?;
        decoded_bytes = decoded_bytes.saturating_add(image.rgba.len() as u64);
        if decoded_bytes > limits.max_decoded_bytes {
            return Err(ResourceError::LimitExceeded {
                resource: Some(resource.texture.id.clone()),
                cause: format!("decoded bytes exceed {}", limits.max_decoded_bytes),
            });
        }
        decoded.push(image);
    }
    metrics.legacy_extraction_time = extraction_started.elapsed();
    metrics.decoded_rgba_bytes = decoded_bytes;

    let packing_started = Instant::now();
    let (pages, placements) = pack(&resolved, &decoded, policy)?;
    metrics.packing_time = packing_started.elapsed();
    metrics.page_count = pages.len();
    metrics.atlas_bytes = pages.iter().map(|page| page.rgba.len() as u64).sum();
    metrics.gpu_format_estimated_atlas_bytes = metrics.atlas_bytes;
    metrics.occupancy = occupancy(&placements, &pages);

    if let Some(directory) = cache_directory {
        let path = directory.join(format!("{cache_key}.rcatlas"));
        metrics.cache_write_attempted = true;
        let write_result = fs::create_dir_all(directory)
            .map_err(|error| ResourceError::Io {
                path: directory.to_path_buf(),
                cause: error.to_string(),
            })
            .and_then(|()| write_cache(&path, cache_key, policy, &resolved, &pages, &placements));
        match write_result {
            Ok(()) => metrics.cache_write_succeeded = true,
            Err(error) => {
                let message = error.to_string();
                eprintln!("resource cache write warning: {message}");
                metrics.cache_write_error = Some(message);
            }
        }
    }
    metrics.total_time = total_started.elapsed();
    Ok(finish_compiled(
        pages, placements, resolved, overrides, metrics, cache_key,
    ))
}

fn select_page_size(
    maximum: AtlasPolicy,
    blobs: &BTreeMap<PathBuf, Arc<[u8]>>,
    resolved: &[ResolvedTexture],
    limits: ResourceLimits,
) -> Result<AtlasPolicy, ResourceError> {
    if resolved.is_empty() {
        return Ok(maximum);
    }
    let mut source_dimensions = BTreeMap::new();
    for (path, bytes) in blobs {
        let (width, height, _) = inspect_png_header(bytes, path, limits)?;
        source_dimensions.insert(path.clone(), (width, height));
    }
    let mut dimensions = Vec::with_capacity(resolved.len());
    for resource in resolved {
        let &(source_width, source_height) = source_dimensions
            .get(&resource.texture.source.path)
            .expect("source header inspected");
        let (width, height) = if let Some(crop) = resource.texture.source.crop {
            let valid = crop.width > 0
                && crop.height > 0
                && crop
                    .x
                    .checked_add(crop.width)
                    .is_some_and(|value| value <= source_width)
                && crop
                    .y
                    .checked_add(crop.height)
                    .is_some_and(|value| value <= source_height);
            if !valid {
                return invalid_crop(resource, crop);
            }
            (crop.width, crop.height)
        } else {
            (source_width, source_height)
        };
        dimensions.push((width, height));
    }

    let square_maximum = maximum.page_width.min(maximum.page_height);
    let mut candidates = [128, 256, 512, 1024, 2048, 4096, 8192, 16_384]
        .into_iter()
        .filter(|value| *value <= square_maximum)
        .collect::<Vec<_>>();
    if candidates.is_empty() || maximum.page_width != maximum.page_height {
        candidates.push(square_maximum);
    }
    candidates.sort_unstable();
    candidates.dedup();
    for dimension in candidates {
        let candidate = AtlasPolicy {
            page_width: dimension,
            page_height: dimension,
            ..maximum
        };
        if predicted_page_count(resolved, &dimensions, candidate)
            .is_some_and(|pages| pages <= candidate.max_pages)
        {
            return Ok(candidate);
        }
    }
    Ok(maximum)
}

fn predicted_page_count(
    resolved: &[ResolvedTexture],
    dimensions: &[(u32, u32)],
    policy: AtlasPolicy,
) -> Option<u32> {
    let mut order = (0..dimensions.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        dimensions[*right]
            .1
            .cmp(&dimensions[*left].1)
            .then_with(|| dimensions[*right].0.cmp(&dimensions[*left].0))
            .then_with(|| resolved[*left].texture.id.cmp(&resolved[*right].texture.id))
    });
    let mut pages = 1_u32;
    let mut cursor = ShelfCursor::default();
    for index in order {
        let (width, height) = dimensions[index];
        let padding = policy.padding.checked_mul(2)?;
        let padded_width = width.checked_add(padding)?;
        let padded_height = height.checked_add(padding)?;
        if padded_width > policy.page_width || padded_height > policy.page_height {
            return None;
        }
        if cursor.x.checked_add(padded_width)? > policy.page_width {
            cursor.x = 0;
            cursor.y = cursor.y.checked_add(cursor.shelf_height)?;
            cursor.shelf_height = 0;
        }
        if cursor.y.checked_add(padded_height)? > policy.page_height {
            pages = pages.checked_add(1)?;
            cursor = ShelfCursor::default();
        }
        cursor.x = cursor.x.checked_add(padded_width)?;
        cursor.shelf_height = cursor.shelf_height.max(padded_height);
    }
    Some(pages)
}

fn finish_compiled(
    pages: Vec<AtlasPage>,
    placements: Vec<Placement>,
    resolved: Vec<ResolvedTexture>,
    overrides: Vec<ResourceOverride>,
    metrics: ResourceMetrics,
    cache_key: ContentHash,
) -> CompiledResources {
    let textures = resolved
        .into_iter()
        .zip(placements)
        .map(|(resource, placement)| {
            let page = &pages[placement.page as usize];
            CompiledTexture {
                id: resource.texture.id,
                provider: resource.provider,
                source_path: resource.texture.source.path,
                source_hash: resource.source_hash,
                page: placement.page,
                pixel_rect: PixelRect {
                    x: placement.x,
                    y: placement.y,
                    width: placement.width,
                    height: placement.height,
                },
                uv_min: [
                    placement.x as f32 / page.width as f32,
                    placement.y as f32 / page.height as f32,
                ],
                uv_max: [
                    (placement.x + placement.width) as f32 / page.width as f32,
                    (placement.y + placement.height) as f32 / page.height as f32,
                ],
                padding: placement.padding,
                sampler: resource.texture.sampler,
            }
        })
        .collect::<Vec<_>>();
    let by_id = textures
        .iter()
        .enumerate()
        .map(|(index, texture)| (texture.id.clone(), index))
        .collect();
    CompiledResources {
        pages,
        textures,
        overrides,
        metrics,
        cache_key,
        by_id,
    }
}

fn decode_unique_sources(
    blobs: &BTreeMap<PathBuf, Arc<[u8]>>,
    resolved: &[ResolvedTexture],
    limits: ResourceLimits,
) -> Result<BTreeMap<PathBuf, DecodedImage>, ResourceError> {
    let sources = blobs
        .iter()
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect::<Vec<_>>();
    if sources.is_empty() {
        return Ok(BTreeMap::new());
    }
    // Read every header before starting workers. The limit applies to the simultaneous set of
    // unique physical-source RGBA buffers, not independently to each worker allocation.
    let mut predicted_total = 0_u64;
    for (path, bytes) in &sources {
        let (_, _, predicted) = inspect_png_header(bytes, path, limits)?;
        predicted_total =
            predicted_total
                .checked_add(predicted)
                .ok_or_else(|| ResourceError::LimitExceeded {
                    resource: None,
                    cause: "aggregate decoded-byte prediction overflow".into(),
                })?;
        if predicted_total > limits.max_decoded_bytes {
            return Err(ResourceError::LimitExceeded {
                resource: None,
                cause: format!(
                    "aggregate decoded allocations {predicted_total} exceed {}",
                    limits.max_decoded_bytes
                ),
            });
        }
    }
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(8)
        .min(sources.len());
    let next = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let sender = sender.clone();
            let sources = &sources;
            let next = &next;
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some((path, bytes)) = sources.get(index) else {
                        break;
                    };
                    let result = decode_png(bytes, path, limits);
                    if sender.send((index, result)).is_err() {
                        break;
                    }
                }
            });
        }
    });
    drop(sender);
    let mut output = Vec::with_capacity(sources.len());
    output.resize_with(sources.len(), || None);
    for (index, image) in receiver {
        output[index] = Some(image);
    }
    let mut decoded = BTreeMap::new();
    for ((path, _), result) in sources.into_iter().zip(output) {
        let image = result
            .expect("worker reports every source")
            .map_err(|error| {
                let resource = resolved
                    .iter()
                    .find(|resource| resource.texture.source.path == path);
                if let Some(resource) = resource {
                    ResourceError::Decode {
                        resource: resource.texture.id.clone(),
                        provider: resource.provider.clone(),
                        path: path.clone(),
                        cause: error.to_string(),
                    }
                } else {
                    error
                }
            })?;
        decoded.insert(path, image);
    }
    Ok(decoded)
}

fn inspect_png_header(
    bytes: &[u8],
    path: &Path,
    limits: ResourceLimits,
) -> Result<(u32, u32, u64), ResourceError> {
    let reader = png::Decoder::new(Cursor::new(bytes))
        .read_info()
        .map_err(|error| decode_source_error(path, error))?;
    let info = reader.info();
    if info.width == 0
        || info.height == 0
        || info.width > limits.max_dimension
        || info.height > limits.max_dimension
    {
        return Err(ResourceError::LimitExceeded {
            resource: None,
            cause: format!(
                "{} dimensions {}x{} outside 1..={}",
                path.display(),
                info.width,
                info.height,
                limits.max_dimension
            ),
        });
    }
    let predicted = u64::from(info.width)
        .checked_mul(u64::from(info.height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| ResourceError::LimitExceeded {
            resource: None,
            cause: format!("{} decoded allocation overflows", path.display()),
        })?;
    if predicted > limits.max_decoded_bytes {
        return Err(ResourceError::LimitExceeded {
            resource: None,
            cause: format!(
                "{} decoded allocation {predicted} exceeds {}",
                path.display(),
                limits.max_decoded_bytes
            ),
        });
    }
    Ok((info.width, info.height, predicted))
}

fn decode_png(
    bytes: &[u8],
    path: &Path,
    limits: ResourceLimits,
) -> Result<DecodedImage, ResourceError> {
    inspect_png_header(bytes, path, limits)?;
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| decode_source_error(path, error))?;
    let mut data = vec![0; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut data)
        .map_err(|error| decode_source_error(path, error))?;
    let raw = &data[..frame.buffer_size()];
    let rgba = match frame.color_type {
        png::ColorType::Rgba => raw.to_vec(),
        png::ColorType::Rgb => raw
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        png::ColorType::Grayscale => raw
            .iter()
            .flat_map(|value| [*value, *value, *value, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => raw
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        png::ColorType::Indexed => {
            return Err(ResourceError::InvalidResourcePath {
                path: path.to_path_buf(),
                cause: "indexed PNG was not expanded".into(),
            });
        }
    };
    Ok(DecodedImage {
        width: frame.width,
        height: frame.height,
        rgba,
    })
}

fn decode_source_error(path: &Path, error: impl fmt::Display) -> ResourceError {
    ResourceError::InvalidResourcePath {
        path: path.to_path_buf(),
        cause: format!("invalid PNG: {error}"),
    }
}

fn crop_image(
    source: &DecodedImage,
    resource: &ResolvedTexture,
) -> Result<DecodedImage, ResourceError> {
    let Some(crop) = resource.texture.source.crop else {
        return Ok(source.clone());
    };
    let Some(max_x) = crop.x.checked_add(crop.width) else {
        return invalid_crop(resource, crop);
    };
    let Some(max_y) = crop.y.checked_add(crop.height) else {
        return invalid_crop(resource, crop);
    };
    if crop.width == 0 || crop.height == 0 || max_x > source.width || max_y > source.height {
        return invalid_crop(resource, crop);
    }
    let mut rgba = Vec::with_capacity((crop.width * crop.height * 4) as usize);
    for y in crop.y..max_y {
        let start = ((y * source.width + crop.x) * 4) as usize;
        let end = start + (crop.width * 4) as usize;
        rgba.extend_from_slice(&source.rgba[start..end]);
    }
    Ok(DecodedImage {
        width: crop.width,
        height: crop.height,
        rgba,
    })
}

fn invalid_crop<T>(resource: &ResolvedTexture, crop: PixelRect) -> Result<T, ResourceError> {
    Err(ResourceError::InvalidCrop {
        resource: resource.texture.id.clone(),
        provider: resource.provider.clone(),
        path: resource.texture.source.path.clone(),
        crop,
    })
}

#[derive(Debug, Clone, Copy)]
struct Placement {
    page: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    padding: u32,
}

fn pack(
    resolved: &[ResolvedTexture],
    images: &[DecodedImage],
    policy: AtlasPolicy,
) -> Result<(Vec<AtlasPage>, Vec<Placement>), ResourceError> {
    let mut order = (0..images.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        images[*right]
            .height
            .cmp(&images[*left].height)
            .then_with(|| images[*right].width.cmp(&images[*left].width))
            .then_with(|| resolved[*left].texture.id.cmp(&resolved[*right].texture.id))
    });
    let mut pages = Vec::<AtlasPage>::new();
    let mut placements = vec![
        Placement {
            page: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            padding: policy.padding
        };
        images.len()
    ];
    let mut cursor = ShelfCursor::default();
    for index in order {
        let image = &images[index];
        let padded_width = image
            .width
            .checked_add(policy.padding.saturating_mul(2))
            .ok_or_else(|| ResourceError::Packing {
                resource: resolved[index].texture.id.clone(),
                cause: "padded width overflow".into(),
            })?;
        let padded_height = image
            .height
            .checked_add(policy.padding.saturating_mul(2))
            .ok_or_else(|| ResourceError::Packing {
                resource: resolved[index].texture.id.clone(),
                cause: "padded height overflow".into(),
            })?;
        if padded_width > policy.page_width || padded_height > policy.page_height {
            return Err(ResourceError::Packing {
                resource: resolved[index].texture.id.clone(),
                cause: format!(
                    "{}x{} plus padding does not fit {}x{} page",
                    image.width, image.height, policy.page_width, policy.page_height
                ),
            });
        }
        if pages.is_empty() {
            pages.push(blank_page(policy));
        }
        if cursor.x + padded_width > policy.page_width {
            cursor.x = 0;
            cursor.y += cursor.shelf_height;
            cursor.shelf_height = 0;
        }
        if cursor.y + padded_height > policy.page_height {
            if pages.len() >= policy.max_pages as usize {
                return Err(ResourceError::Packing {
                    resource: resolved[index].texture.id.clone(),
                    cause: format!("atlas exceeds {} pages", policy.max_pages),
                });
            }
            pages.push(blank_page(policy));
            cursor = ShelfCursor::default();
        }
        let placement = Placement {
            page: (pages.len() - 1) as u32,
            x: cursor.x + policy.padding,
            y: cursor.y + policy.padding,
            width: image.width,
            height: image.height,
            padding: policy.padding,
        };
        blit_extruded(&mut pages[placement.page as usize], image, placement);
        placements[index] = placement;
        cursor.x += padded_width;
        cursor.shelf_height = cursor.shelf_height.max(padded_height);
    }
    Ok((pages, placements))
}

#[derive(Default)]
struct ShelfCursor {
    x: u32,
    y: u32,
    shelf_height: u32,
}

fn blank_page(policy: AtlasPolicy) -> AtlasPage {
    AtlasPage {
        width: policy.page_width,
        height: policy.page_height,
        rgba: vec![0; (policy.page_width * policy.page_height * 4) as usize],
        sampler: policy.sampler,
    }
}

fn blit_extruded(page: &mut AtlasPage, image: &DecodedImage, placement: Placement) {
    let padding = placement.padding as i32;
    for dy in -padding..image.height as i32 + padding {
        for dx in -padding..image.width as i32 + padding {
            let source_x = dx.clamp(0, image.width as i32 - 1) as u32;
            let source_y = dy.clamp(0, image.height as i32 - 1) as u32;
            let target_x = (placement.x as i32 + dx) as u32;
            let target_y = (placement.y as i32 + dy) as u32;
            let source = ((source_y * image.width + source_x) * 4) as usize;
            let target = ((target_y * page.width + target_x) * 4) as usize;
            page.rgba[target..target + 4].copy_from_slice(&image.rgba[source..source + 4]);
        }
    }
}

fn occupancy(placements: &[Placement], pages: &[AtlasPage]) -> f32 {
    let used = placements
        .iter()
        .map(|placement| u64::from(placement.width) * u64::from(placement.height))
        .sum::<u64>();
    let total = pages
        .iter()
        .map(|page| u64::from(page.width) * u64::from(page.height))
        .sum::<u64>();
    if total == 0 {
        0.0
    } else {
        used as f32 / total as f32
    }
}

fn cache_key(resolved: &[ResolvedTexture], policy: AtlasPolicy) -> ContentHash {
    cache_key_version(resolved, policy, COMPILER_VERSION)
}

fn cache_key_version(
    resolved: &[ResolvedTexture],
    policy: AtlasPolicy,
    compiler_version: u32,
) -> ContentHash {
    let mut hasher = blake3::Hasher::new_derive_key(CACHE_DOMAIN);
    hasher.update(&compiler_version.to_le_bytes());
    for value in [
        policy.page_width,
        policy.page_height,
        policy.max_pages,
        policy.padding,
    ] {
        hasher.update(&value.to_le_bytes());
    }
    hasher.update(&[policy.sampler.code(), policy.mip_levels]);
    hasher.update(&policy.anisotropy.to_le_bytes());
    write_hash_len(&mut hasher, resolved.len());
    for resource in resolved {
        write_hash_str(&mut hasher, resource.texture.id.as_str());
        write_hash_str(&mut hasher, resource.provider.as_str());
        hasher.update(resource.source_hash.as_bytes());
        if let Some(crop) = resource.texture.source.crop {
            hasher.update(&[1]);
            for value in [crop.x, crop.y, crop.width, crop.height] {
                hasher.update(&value.to_le_bytes());
            }
        } else {
            hasher.update(&[0]);
        }
        hasher.update(&[resource.texture.sampler.code()]);
    }
    ContentHash::from_digest_bytes(*hasher.finalize().as_bytes())
}

fn write_hash_len(hasher: &mut blake3::Hasher, value: usize) {
    hasher.update(&(value as u64).to_le_bytes());
}
fn write_hash_str(hasher: &mut blake3::Hasher, value: &str) {
    write_hash_len(hasher, value.len());
    hasher.update(value.as_bytes());
}

fn write_cache(
    path: &Path,
    key: ContentHash,
    policy: AtlasPolicy,
    resolved: &[ResolvedTexture],
    pages: &[AtlasPage],
    placements: &[Placement],
) -> Result<(), ResourceError> {
    let sequence = CACHE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("atlas");
    let temporary = path.with_file_name(format!(
        ".{file_name}.tmp-{}-{sequence}",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| ResourceError::Io {
            path: temporary.clone(),
            cause: error.to_string(),
        })?;
    let result = (|| {
        file.write_all(CACHE_MAGIC)
            .and_then(|_| file.write_all(key.as_bytes()))
            .map_err(|error| ResourceError::Io {
                path: temporary.clone(),
                cause: error.to_string(),
            })?;
        write_u32(&mut file, policy.page_width)?;
        write_u32(&mut file, policy.page_height)?;
        write_u32(&mut file, pages.len() as u32)?;
        for page in pages {
            write_bytes(&mut file, &page.rgba)?;
        }
        write_u32(&mut file, placements.len() as u32)?;
        for (resource, placement) in resolved.iter().zip(placements) {
            write_string(&mut file, resource.texture.id.as_str())?;
            for value in [
                placement.page,
                placement.x,
                placement.y,
                placement.width,
                placement.height,
                placement.padding,
            ] {
                write_u32(&mut file, value)?;
            }
        }
        file.sync_all().map_err(|error| ResourceError::Io {
            path: temporary.clone(),
            cause: error.to_string(),
        })?;
        fs::rename(&temporary, path).map_err(|error| ResourceError::Io {
            path: path.to_path_buf(),
            cause: error.to_string(),
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

type CachedAtlas = (Vec<AtlasPage>, Vec<Placement>);

fn read_cache(
    path: &Path,
    key: ContentHash,
    policy: AtlasPolicy,
    resolved: &[ResolvedTexture],
) -> Result<Option<CachedAtlas>, ResourceError> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ResourceError::Io {
                path: path.to_path_buf(),
                cause: error.to_string(),
            });
        }
    };
    let result = (|| -> Option<(Vec<AtlasPage>, Vec<Placement>)> {
        let mut magic = [0; 8];
        file.read_exact(&mut magic).ok()?;
        if &magic != CACHE_MAGIC {
            return None;
        }
        let mut stored_key = [0; 32];
        file.read_exact(&mut stored_key).ok()?;
        if stored_key != *key.as_bytes() {
            return None;
        }
        let width = read_u32(&mut file)?;
        let height = read_u32(&mut file)?;
        if width != policy.page_width || height != policy.page_height {
            return None;
        }
        let page_count = read_u32(&mut file)? as usize;
        if page_count == 0 || page_count > policy.max_pages as usize {
            return None;
        }
        let expected_page_bytes = usize::try_from(u64::from(width) * u64::from(height) * 4).ok()?;
        let mut pages = Vec::with_capacity(page_count);
        for _ in 0..page_count {
            let rgba = read_bytes(&mut file, expected_page_bytes)?;
            if rgba.len() != expected_page_bytes {
                return None;
            }
            pages.push(AtlasPage {
                width,
                height,
                rgba,
                sampler: policy.sampler,
            });
        }
        let count = read_u32(&mut file)? as usize;
        if count != resolved.len() {
            return None;
        }
        let mut placements = Vec::with_capacity(count);
        for resource in resolved {
            if read_string(&mut file)? != resource.texture.id.as_str() {
                return None;
            }
            let placement = Placement {
                page: read_u32(&mut file)?,
                x: read_u32(&mut file)?,
                y: read_u32(&mut file)?,
                width: read_u32(&mut file)?,
                height: read_u32(&mut file)?,
                padding: read_u32(&mut file)?,
            };
            if placement.page as usize >= pages.len()
                || placement.x.checked_add(placement.width)? > width
                || placement.y.checked_add(placement.height)? > height
            {
                return None;
            }
            placements.push(placement);
        }
        let mut trailing = [0];
        if file.read(&mut trailing).ok()? != 0 {
            return None;
        }
        Some((pages, placements))
    })();
    Ok(result)
}

fn write_u32(writer: &mut impl Write, value: u32) -> Result<(), ResourceError> {
    writer.write_all(&value.to_le_bytes()).map_err(cache_io)
}
fn write_bytes(writer: &mut impl Write, bytes: &[u8]) -> Result<(), ResourceError> {
    write_u32(writer, bytes.len() as u32)?;
    writer.write_all(bytes).map_err(cache_io)
}
fn write_string(writer: &mut impl Write, value: &str) -> Result<(), ResourceError> {
    write_bytes(writer, value.as_bytes())
}
fn cache_io(error: std::io::Error) -> ResourceError {
    ResourceError::Io {
        path: PathBuf::from("<resource-cache>"),
        cause: error.to_string(),
    }
}
fn read_u32(reader: &mut impl Read) -> Option<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes).ok()?;
    Some(u32::from_le_bytes(bytes))
}
fn read_bytes(reader: &mut impl Read, maximum: usize) -> Option<Vec<u8>> {
    let length = read_u32(reader)? as usize;
    if length > maximum {
        return None;
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).ok()?;
    Some(bytes)
}
fn read_string(reader: &mut impl Read) -> Option<String> {
    String::from_utf8(read_bytes(reader, 1024 * 1024)?).ok()
}

pub fn write_atlas_debug(
    resources: &CompiledResources,
    directory: &Path,
) -> Result<Vec<PathBuf>, ResourceError> {
    fs::create_dir_all(directory).map_err(|error| ResourceError::Io {
        path: directory.to_path_buf(),
        cause: error.to_string(),
    })?;
    let mut output = Vec::new();
    for (index, page) in resources.pages.iter().enumerate() {
        let path = directory.join(format!("atlas-{index}.png"));
        let file = fs::File::create(&path).map_err(|error| ResourceError::Io {
            path: path.clone(),
            cause: error.to_string(),
        })?;
        let mut encoder = png::Encoder::new(file, page.width, page.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| ResourceError::Io {
            path: path.clone(),
            cause: error.to_string(),
        })?;
        writer
            .write_image_data(&page.rgba)
            .map_err(|error| ResourceError::Io {
                path: path.clone(),
                cause: error.to_string(),
            })?;
        output.push(path);
    }
    let metadata = directory.join("atlas-map.txt");
    let mut text = String::new();
    for texture in &resources.textures {
        text.push_str(&format!(
            "{} provider={} page={} rect={},{},{},{} uv={:?}..{:?} padding={} hash={} source={}\n",
            texture.id,
            texture.provider,
            texture.page,
            texture.pixel_rect.x,
            texture.pixel_rect.y,
            texture.pixel_rect.width,
            texture.pixel_rect.height,
            texture.uv_min,
            texture.uv_max,
            texture.padding,
            texture.source_hash,
            texture.source_path.display()
        ));
    }
    fs::write(&metadata, text).map_err(|error| ResourceError::Io {
        path: metadata.clone(),
        cause: error.to_string(),
    })?;
    output.push(metadata);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(value: &str) -> PackageId {
        PackageId::parse(value).unwrap()
    }
    fn resource(value: &str) -> ResourceId {
        ResourceId::parse(value).unwrap()
    }

    fn write_png(path: &Path, width: u32, height: u32, color: [u8; 4]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&color.repeat((width * height) as usize))
            .unwrap();
    }

    fn temporary(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("rustcraft-resource-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn discovery_is_namespaced_sorted_and_rejects_traversal() {
        let root = temporary("discovery");
        write_png(
            &root.join("assets/test/textures/block/zeta.png"),
            2,
            2,
            [1, 2, 3, 255],
        );
        write_png(
            &root.join("assets/test/textures/block/alpha.png"),
            2,
            2,
            [4, 5, 6, 255],
        );
        fs::create_dir_all(root.join("assets/test/models/block")).unwrap();
        fs::write(
            root.join("assets/test/models/block/future.json"),
            br#"{"future":true}"#,
        )
        .unwrap();
        let discovered = discover_package(
            &root,
            package("test:package/native"),
            ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(
            discovered.textures()[0].id,
            resource("test:textures/block/alpha")
        );
        assert_eq!(
            discovered.textures()[1].id,
            resource("test:textures/block/zeta")
        );
        let mut authored = ResourcePackage::new(package("test:package/manual"), &root);
        assert!(
            authored
                .add_texture(
                    resource("test:textures/block/bad"),
                    "../escape.png",
                    None,
                    SamplerPolicy::Nearest
                )
                .is_err()
        );
    }

    #[test]
    fn aggregate_unique_source_decode_budget_is_checked_before_parallel_decode() {
        let root = temporary("aggregate-decode-limit");
        write_png(&root.join("first.png"), 4, 4, [1, 2, 3, 255]);
        write_png(&root.join("second.png"), 4, 4, [4, 5, 6, 255]);
        let mut package = ResourcePackage::new(package("test:package/resources"), &root);
        package
            .add_texture(
                resource("test:textures/first"),
                "first.png",
                None,
                SamplerPolicy::Nearest,
            )
            .unwrap();
        package
            .add_texture(
                resource("test:textures/second"),
                "second.png",
                None,
                SamplerPolicy::Nearest,
            )
            .unwrap();
        let error = compile_resources(
            &[package],
            AtlasPolicy {
                page_width: 16,
                page_height: 16,
                ..AtlasPolicy::default()
            },
            ResourceLimits {
                max_decoded_bytes: 100,
                ..ResourceLimits::default()
            },
            None,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("aggregate decoded allocations 128")
        );
    }

    #[test]
    fn adaptive_policy_selects_smallest_supported_page_and_can_be_device_bounded() {
        let root = temporary("adaptive-page");
        write_png(&root.join("value.png"), 16, 16, [1, 2, 3, 255]);
        let mut package = ResourcePackage::new(package("test:package/resources"), &root);
        package
            .add_texture(
                resource("test:textures/value"),
                "value.png",
                None,
                SamplerPolicy::Nearest,
            )
            .unwrap();
        let compiled = compile_resources(
            &[package],
            AtlasPolicy::default(),
            ResourceLimits::default(),
            None,
        )
        .unwrap();
        assert_eq!(
            (compiled.pages[0].width, compiled.pages[0].height),
            (128, 128)
        );
        let bounded = AtlasPolicy::default()
            .bounded_by_device_dimension(300)
            .unwrap();
        assert_eq!((bounded.page_width, bounded.page_height), (256, 256));
    }

    #[test]
    fn optional_cache_write_failure_is_reported_but_does_not_block_compilation() {
        let root = temporary("cache-write-failure");
        write_png(&root.join("value.png"), 2, 2, [1, 2, 3, 255]);
        let mut package = ResourcePackage::new(package("test:package/resources"), &root);
        package
            .add_texture(
                resource("test:textures/value"),
                "value.png",
                None,
                SamplerPolicy::Nearest,
            )
            .unwrap();
        let invalid_cache_directory = root.join("not-a-directory");
        fs::write(&invalid_cache_directory, b"file").unwrap();
        let compiled = compile_resources(
            &[package],
            AtlasPolicy {
                page_width: 16,
                page_height: 16,
                ..AtlasPolicy::default()
            },
            ResourceLimits::default(),
            Some(&invalid_cache_directory),
        )
        .unwrap();
        assert!(compiled.metrics.cache_write_attempted);
        assert!(!compiled.metrics.cache_write_succeeded);
        assert!(compiled.metrics.cache_write_error.is_some());
        assert!(compiled.report().contains("cache_write=failed"));
    }

    #[test]
    fn one_sampler_policy_is_preserved_per_physical_page() {
        let root = temporary("sampler-policy");
        write_png(&root.join("value.png"), 2, 2, [1, 2, 3, 255]);
        let mut nearest = ResourcePackage::new(package("test:package/nearest"), &root);
        nearest
            .add_texture(
                resource("test:textures/value"),
                "value.png",
                None,
                SamplerPolicy::Nearest,
            )
            .unwrap();
        let linear_policy = AtlasPolicy {
            page_width: 16,
            page_height: 16,
            sampler: SamplerPolicy::Linear,
            ..AtlasPolicy::default()
        };
        assert!(matches!(
            compile_resources(&[nearest], linear_policy, ResourceLimits::default(), None),
            Err(ResourceError::UnsupportedPolicy(_))
        ));

        let mut linear = ResourcePackage::new(package("test:package/linear"), &root);
        linear
            .add_texture(
                resource("test:textures/value"),
                "value.png",
                None,
                SamplerPolicy::Linear,
            )
            .unwrap();
        let compiled =
            compile_resources(&[linear], linear_policy, ResourceLimits::default(), None).unwrap();
        assert!(
            compiled
                .pages
                .iter()
                .all(|page| page.sampler == SamplerPolicy::Linear)
        );
    }

    #[test]
    fn concurrent_cache_writers_use_independent_temporary_files() {
        let root = temporary("cache-concurrency");
        write_png(&root.join("value.png"), 2, 2, [1, 2, 3, 255]);
        let mut package = ResourcePackage::new(package("test:package/resources"), &root);
        package
            .add_texture(
                resource("test:textures/value"),
                "value.png",
                None,
                SamplerPolicy::Nearest,
            )
            .unwrap();
        let cache = root.join("cache");
        let policy = AtlasPolicy {
            page_width: 16,
            page_height: 16,
            ..AtlasPolicy::default()
        };
        std::thread::scope(|scope| {
            let first_package = package.clone();
            let first_cache = cache.clone();
            let first = scope.spawn(move || {
                compile_resources(
                    &[first_package],
                    policy,
                    ResourceLimits::default(),
                    Some(&first_cache),
                )
                .unwrap()
            });
            let second = scope.spawn(|| {
                compile_resources(&[package], policy, ResourceLimits::default(), Some(&cache))
                    .unwrap()
            });
            assert_eq!(
                first.join().unwrap().cache_key,
                second.join().unwrap().cache_key
            );
        });
        assert!(fs::read_dir(&cache).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp-")
        }));
    }

    #[test]
    fn later_packages_override_and_cache_is_content_addressed() {
        let root = temporary("cache");
        write_png(&root.join("a.png"), 3, 2, [10, 20, 30, 255]);
        write_png(&root.join("b.png"), 3, 2, [40, 50, 60, 255]);
        let id = resource("test:textures/block/value");
        let mut a = ResourcePackage::new(package("a:package/resources"), &root);
        a.add_texture(id.clone(), "a.png", None, SamplerPolicy::Nearest)
            .unwrap();
        let mut b = ResourcePackage::new(package("b:package/resources"), &root);
        b.add_texture(id.clone(), "b.png", None, SamplerPolicy::Nearest)
            .unwrap();
        let policy = AtlasPolicy {
            page_width: 16,
            page_height: 16,
            ..AtlasPolicy::default()
        };
        let cache = root.join("cache");
        let cold = compile_resources(
            &[a.clone(), b.clone()],
            policy,
            ResourceLimits::default(),
            Some(&cache),
        )
        .unwrap();
        assert!(!cold.metrics.cache_hit);
        assert_eq!(cold.texture(&id).unwrap().provider, b.id);
        assert_eq!(cold.overrides.len(), 1);
        let warm = compile_resources(
            &[a.clone(), b.clone()],
            policy,
            ResourceLimits::default(),
            Some(&cache),
        )
        .unwrap();
        assert!(warm.metrics.cache_hit);
        assert_eq!(cold.cache_key, warm.cache_key);
        write_png(&root.join("b.png"), 3, 2, [99, 50, 60, 255]);
        let changed = compile_resources(
            &[a.clone(), b.clone()],
            policy,
            ResourceLimits::default(),
            Some(&cache),
        )
        .unwrap();
        assert!(!changed.metrics.cache_hit);
        assert_ne!(warm.cache_key, changed.cache_key);
        let changed_policy = AtlasPolicy {
            padding: 1,
            ..policy
        };
        assert!(
            !compile_resources(
                &[a, b],
                changed_policy,
                ResourceLimits::default(),
                Some(&cache)
            )
            .unwrap()
            .metrics
            .cache_hit
        );

        let mut replacement = ResourcePackage::new(package("replacement:package/resources"), &root);
        replacement
            .add_texture(id, "b.png", None, SamplerPolicy::Nearest)
            .unwrap();
        let provider_changed = compile_resources(
            &[replacement],
            policy,
            ResourceLimits::default(),
            Some(&cache),
        )
        .unwrap();
        assert!(!provider_changed.metrics.cache_hit);
        assert_ne!(changed.cache_key, provider_changed.cache_key);
    }

    #[test]
    fn packing_is_deterministic_multi_page_and_extrudes_edges() {
        let root = temporary("packing");
        let mut package = ResourcePackage::new(package("test:package/resources"), &root);
        for index in 0..6 {
            let name = format!("{index}.png");
            let color = [index as u8 + 1, 0, 0, 255];
            write_png(&root.join(&name), 4, 4, color);
            package
                .add_texture(
                    resource(&format!("test:textures/block/t{index}")),
                    name,
                    None,
                    SamplerPolicy::Nearest,
                )
                .unwrap();
        }
        let policy = AtlasPolicy {
            page_width: 16,
            page_height: 16,
            max_pages: 8,
            padding: 1,
            ..AtlasPolicy::default()
        };
        let first =
            compile_resources(&[package.clone()], policy, ResourceLimits::default(), None).unwrap();
        let second =
            compile_resources(&[package], policy, ResourceLimits::default(), None).unwrap();
        assert!(first.pages.len() > 1);
        assert_eq!(first.cache_key, second.cache_key);
        assert_eq!(
            first
                .textures
                .iter()
                .map(|entry| (entry.page, entry.pixel_rect))
                .collect::<Vec<_>>(),
            second
                .textures
                .iter()
                .map(|entry| (entry.page, entry.pixel_rect))
                .collect::<Vec<_>>()
        );
        let entry = &first.textures[0];
        let page = &first.pages[entry.page as usize];
        let interior = ((entry.pixel_rect.y * page.width + entry.pixel_rect.x) * 4) as usize;
        let padding =
            (((entry.pixel_rect.y - 1) * page.width + entry.pixel_rect.x - 1) * 4) as usize;
        assert_eq!(
            &page.rgba[interior..interior + 4],
            &page.rgba[padding..padding + 4]
        );

        let debug_directory = root.join("atlas-debug");
        let debug_outputs = write_atlas_debug(&first, &debug_directory).unwrap();
        assert_eq!(
            debug_outputs
                .iter()
                .filter(|path| path.extension().is_some_and(|extension| extension == "png"))
                .count(),
            first.pages.len()
        );
        assert!(debug_directory.join("atlas-map.txt").is_file());
    }

    #[test]
    fn corrupted_cache_rebuilds_safely() {
        let root = temporary("corrupt");
        write_png(&root.join("value.png"), 2, 2, [1, 2, 3, 255]);
        let mut package = ResourcePackage::new(package("test:package/resources"), &root);
        package
            .add_texture(
                resource("test:textures/value"),
                "value.png",
                None,
                SamplerPolicy::Nearest,
            )
            .unwrap();
        let cache = root.join("cache");
        let policy = AtlasPolicy {
            page_width: 16,
            page_height: 16,
            ..AtlasPolicy::default()
        };
        let first = compile_resources(
            &[package.clone()],
            policy,
            ResourceLimits::default(),
            Some(&cache),
        )
        .unwrap();
        fs::write(
            cache.join(format!("{}.rcatlas", first.cache_key)),
            b"broken",
        )
        .unwrap();
        let rebuilt =
            compile_resources(&[package], policy, ResourceLimits::default(), Some(&cache)).unwrap();
        assert!(!rebuilt.metrics.cache_hit);
        assert_eq!(rebuilt.textures.len(), 1);
    }

    #[test]
    fn compiler_version_is_part_of_the_cache_identity() {
        let root = temporary("version");
        write_png(&root.join("value.png"), 2, 2, [1, 2, 3, 255]);
        let texture = AuthoredTexture {
            id: resource("test:textures/value"),
            source: TextureSource {
                path: root.join("value.png"),
                crop: None,
            },
            sampler: SamplerPolicy::Nearest,
        };
        let bytes: Arc<[u8]> = Arc::from(fs::read(&texture.source.path).unwrap());
        let resolved = vec![ResolvedTexture {
            texture,
            provider: package("test:package/resources"),
            source_hash: ContentHash::from_content_bytes(&bytes),
        }];
        assert_ne!(
            cache_key_version(&resolved, AtlasPolicy::default(), 1),
            cache_key_version(&resolved, AtlasPolicy::default(), 2)
        );
    }
}
