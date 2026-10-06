//! Load-time bridge from semantic Game API definitions to indexed renderer data.

use rustcraft_content::{ResourceId, resources::CompiledResources};
use rustcraft_engine_core::{BlockId, BlockState, orientation::ModelRotation};
use rustcraft_game_api::{CompiledGameProfile, MaterialClass, TextureKey};
use rustcraft_render::{
    AtlasRegion, BlockTextureResolver, Face, TextureHandle, inspection::BlockModel,
};
use std::{collections::HashMap, fmt};

#[derive(Debug, Clone)]
pub struct CompiledTextureEntry {
    pub key: TextureKey,
    pub provider: rustcraft_content::PackageId,
    pub region: AtlasRegion,
    pub pixel_rect: rustcraft_content::resources::PixelRect,
    pub source_hash: rustcraft_content::ContentHash,
}

#[derive(Debug, Clone)]
pub struct CompiledTextureRegistry {
    entries: Vec<CompiledTextureEntry>,
    by_key: HashMap<TextureKey, usize>,
}

impl CompiledTextureRegistry {
    pub fn from_resources(resources: &CompiledResources) -> Result<Self, CompileRenderError> {
        let mut entries = Vec::with_capacity(resources.textures.len());
        let mut by_key = HashMap::with_capacity(resources.textures.len());
        for texture in &resources.textures {
            let key = TextureKey::parse(texture.id.as_str())
                .map_err(|_| CompileRenderError::InvalidTextureIdentity(texture.id.clone()))?;
            let index = entries.len();
            by_key.insert(key.clone(), index);
            entries.push(CompiledTextureEntry {
                key,
                provider: texture.provider.clone(),
                region: AtlasRegion {
                    texture: TextureHandle(texture.page),
                    uv_min: texture.uv_min,
                    uv_max: texture.uv_max,
                },
                pixel_rect: texture.pixel_rect,
                source_hash: texture.source_hash,
            });
        }
        Ok(Self { entries, by_key })
    }

    #[must_use]
    pub fn get(&self, key: &TextureKey) -> Option<&CompiledTextureEntry> {
        self.by_key.get(key).map(|index| &self.entries[*index])
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct CompiledVoxelRender {
    pub geometry: Option<rustcraft_game_api::CompiledGeometryBinding>,
    pub visible: bool,
    pub opaque: bool,
    pub translucent: bool,
    pub selectable: bool,
    pub liquid_surface_height: Option<f32>,
    pub material: MaterialClass,
    pub faces: [AtlasRegion; 6],
    pub tints: [[f32; 3]; 6],
    pub base_rotation: ModelRotation,
    pub state_schema: rustcraft_game_api::state_schema::CompiledStateSchema,
}

#[derive(Debug, Clone)]
pub struct CompiledVoxelRenderRegistry {
    geometry: std::sync::Arc<rustcraft_game_api::GeometryCatalog>,
    blocks: Vec<CompiledVoxelRender>,
}

impl CompiledVoxelRenderRegistry {
    pub fn compile(
        profile: &CompiledGameProfile,
        textures: &CompiledTextureRegistry,
    ) -> Result<Self, CompileRenderError> {
        let mut blocks = Vec::with_capacity(profile.blocks().len());
        for definition in profile.blocks() {
            let mut faces = [AtlasRegion::full(TextureHandle(0)); 6];
            if definition.material != MaterialClass::Invisible {
                for (face, region) in faces.iter_mut().enumerate() {
                    let key = definition.textures.face(face);
                    *region = textures.get(key).map(|entry| entry.region).ok_or_else(|| {
                        CompileRenderError::MissingTexture {
                            block: definition.id,
                            texture: key.clone(),
                        }
                    })?;
                }
            }
            blocks.push(CompiledVoxelRender {
                geometry: definition.geometry.clone(),
                visible: definition.material != MaterialClass::Invisible,
                opaque: definition.material == MaterialClass::Opaque,
                translucent: matches!(
                    definition.material,
                    MaterialClass::Translucent | MaterialClass::Liquid
                ),
                selectable: definition.targetable,
                liquid_surface_height: (definition.material == MaterialClass::Liquid)
                    .then_some(0.875),
                material: definition.material,
                faces,
                tints: definition
                    .face_tints
                    .map(|color| color.map(|channel| f32::from(channel) / f32::from(u16::MAX))),
                base_rotation: definition.base_rotation,
                state_schema: definition.state_schema.clone(),
            });
        }
        Ok(Self {
            blocks,
            geometry: profile.geometry().clone(),
        })
    }

    #[must_use]
    pub fn block(&self, id: BlockId) -> Option<&CompiledVoxelRender> {
        self.blocks.get(id.0 as usize)
    }

    #[must_use]
    pub fn block_model(&self, state: BlockState) -> Option<BlockModel> {
        let block = self.block(state.block)?;
        block.state_schema.validate(state.variant).ok()?;
        let rotation = self.model_rotation(state);
        block.visible.then(|| BlockModel {
            state,
            triangles: self.model_triangles(state).cloned(),
            textures: block.faces,
            tints: block.tints,
            rotation,
        })
    }
}

impl BlockTextureResolver for CompiledVoxelRenderRegistry {
    fn covers_triangle(&self, state: BlockState, face: Face, positions: [[f32; 3]; 3]) -> bool {
        if self.covers_face(state, face) {
            return true;
        }
        let Some(block) = self.block(state.block).filter(|b| b.opaque) else {
            return false;
        };
        let Some(binding) = &block.geometry else {
            return false;
        };
        let Ok(g) = binding.resolve(state.variant) else {
            return false;
        };
        let inverse = block.base_rotation.compose(g.transform).inverse();
        let local = inverse.transform(rustcraft_render::geometry::FACES[face as usize].normal);
        let f = rustcraft_render::geometry::FACES
            .iter()
            .position(|f| f.normal == local)
            .unwrap();
        let axis = [2, 2, 0, 0, 1, 1][f];
        let uv = match axis {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        };
        let positions = positions.map(|p| inverse.point(p));
        let min: [f32; 2] =
            std::array::from_fn(|i| positions.iter().map(|p| p[uv[i]]).fold(1., f32::min));
        let max: [f32; 2] =
            std::array::from_fn(|i| positions.iter().map(|p| p[uv[i]]).fold(0., f32::max));
        let mut required = 0u16;
        for v in 0..4 {
            for u in 0..4 {
                if min[0] < (u + 1) as f32 / 4.
                    && max[0] > u as f32 / 4.
                    && min[1] < (v + 1) as f32 / 4.
                    && max[1] > v as f32 / 4.
                {
                    required |= 1 << (v * 4 + u);
                }
            }
        }
        let covered = self.geometry.models[g.model.0 as usize].boundary_masks[f];
        required != 0 && required & !covered == 0
    }

    fn valid_state(&self, state: BlockState) -> bool {
        self.block(state.block)
            .is_some_and(|b| b.state_schema.validate(state.variant).is_ok())
    }

    fn selection_edges(&self, state: BlockState) -> Option<&std::sync::Arc<[[[f32; 3]; 2]]>> {
        let g = self
            .block(state.block)?
            .geometry
            .as_ref()?
            .resolve(state.variant)
            .ok()?;
        self.geometry.shapes[g.selection.0 as usize].edges.as_ref()
    }

    fn model_triangles(
        &self,
        state: BlockState,
    ) -> Option<&std::sync::Arc<[rustcraft_engine_core::shape::Triangle]>> {
        let g = self
            .block(state.block)?
            .geometry
            .as_ref()?
            .resolve(state.variant)
            .ok()?;
        self.geometry
            .models
            .get(g.model.0 as usize)?
            .triangles
            .as_ref()
    }
    fn covers_face(&self, state: BlockState, face: Face) -> bool {
        let Some(block) = self.block(state.block).filter(|b| b.opaque) else {
            return false;
        };
        let Some(binding) = &block.geometry else {
            return true;
        };
        let Ok(g) = binding.resolve(state.variant) else {
            return false;
        };
        let rotation = block.base_rotation.compose(g.transform);
        let local = rotation
            .inverse()
            .transform(rustcraft_render::geometry::FACES[face as usize].normal);
        let f = rustcraft_render::geometry::FACES
            .iter()
            .position(|f| f.normal == local)
            .unwrap();
        self.geometry.models[g.model.0 as usize].coverage[f] == rustcraft_game_api::Coverage::Full
    }

    fn model_rotation(&self, state: BlockState) -> ModelRotation {
        self.block(state.block)
            .map_or(ModelRotation::IDENTITY, |block| {
                if let Some(binding) = &block.geometry {
                    return binding
                        .resolve(state.variant)
                        .map_or(ModelRotation::IDENTITY, |g| {
                            block.base_rotation.compose(g.transform)
                        });
                }
                block.base_rotation.compose(
                    block
                        .state_schema
                        .rotation(state.variant)
                        .unwrap_or(ModelRotation::IDENTITY),
                )
            })
    }

    fn texture(&self, block: BlockId, face: Face) -> Option<AtlasRegion> {
        self.block(block)
            .filter(|block| block.visible)
            .map(|block| block.faces[face as usize])
    }

    fn opaque(&self, block: BlockId) -> bool {
        self.block(block).is_some_and(|block| block.opaque)
    }
    fn visible(&self, block: BlockId) -> bool {
        self.block(block).is_some_and(|block| block.visible)
    }
    fn translucent(&self, block: BlockId) -> bool {
        self.block(block).is_some_and(|block| block.translucent)
    }
    fn liquid_surface_height(&self, state: BlockState) -> Option<f32> {
        self.block(state.block)
            .and_then(|block| block.liquid_surface_height)
    }
    fn tint(&self, block: BlockId, face: Face) -> [f32; 3] {
        self.block(block)
            .map_or([1.0; 3], |block| block.tints[face as usize])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileRenderError {
    InvalidTextureIdentity(ResourceId),
    MissingTexture { block: BlockId, texture: TextureKey },
}

impl fmt::Display for CompileRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTextureIdentity(id) => {
                write!(formatter, "resource {id} cannot be used as a texture key")
            }
            Self::MissingTexture { block, texture } => write!(
                formatter,
                "compiled block {:?} references missing texture {}",
                block,
                texture.as_str()
            ),
        }
    }
}

impl std::error::Error for CompileRenderError {}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_content::{
        PackageId,
        resources::{
            AtlasPolicy, ResourceLimits, ResourcePackage, SamplerPolicy, compile_resources,
        },
    };
    use rustcraft_engine_core::orientation::OrientationProperty;
    use rustcraft_game_api::{
        BlockKey, CollisionDescriptor, FaceResources, GameProfile, GameRegistry, LightDescriptor,
        MaterialClass, VoxelDefinition,
    };
    use std::{fs, path::Path};

    fn png(path: &Path) {
        let file = fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[255; 16])
            .unwrap();
    }

    #[test]
    fn compiles_direct_block_id_lookup_without_semantic_hot_path() {
        let root =
            std::env::temp_dir().join(format!("rustcraft-render-profile-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        png(&root.join("block.png"));
        let package = PackageId::parse("test:package/resources").unwrap();
        let texture = TextureKey::parse("test:textures/block/value").unwrap();
        let mut resources = ResourcePackage::new(package.clone(), &root);
        resources
            .add_texture(
                ResourceId::parse(texture.as_str()).unwrap(),
                "block.png",
                None,
                SamplerPolicy::Nearest,
            )
            .unwrap();
        let compiled_resources = compile_resources(
            &[resources],
            AtlasPolicy {
                page_width: 16,
                page_height: 16,
                ..AtlasPolicy::default()
            },
            ResourceLimits::default(),
            None,
        )
        .unwrap();
        let textures = CompiledTextureRegistry::from_resources(&compiled_resources).unwrap();
        let mut registry = GameRegistry::default();
        registry.register_package(package.clone()).unwrap();
        let block_key = BlockKey::parse("test:block/value").unwrap();
        registry
            .register_block(VoxelDefinition {
                geometry: None,
                common: rustcraft_game_api::ContentDefinition::new(
                    block_key.clone().as_id().clone(),
                )
                .with_capabilities(vec![]),
                state_schema: None,
                collision: CollisionDescriptor::FullCube,
                targetable: true,
                material: MaterialClass::Opaque,
                textures: FaceResources::All(texture),
                light: LightDescriptor::default(),
                base_rotation: ModelRotation::IDENTITY,
                orientation: OrientationProperty::None,
                face_tints: [[u16::MAX; 3]; 6],
            })
            .unwrap();
        let water_key = BlockKey::parse("test:block/water").unwrap();
        registry
            .register_block(VoxelDefinition {
                geometry: None,
                common: rustcraft_game_api::ContentDefinition::new(
                    water_key.clone().as_id().clone(),
                )
                .with_capabilities(vec![]),
                state_schema: None,
                collision: CollisionDescriptor::Empty,
                targetable: false,
                material: MaterialClass::Liquid,
                textures: FaceResources::All(
                    TextureKey::parse("test:textures/block/value").unwrap(),
                ),
                light: LightDescriptor {
                    sky_opacity: 1,
                    block_opacity: 1,
                    ..LightDescriptor::default()
                },
                base_rotation: ModelRotation::IDENTITY,
                orientation: OrientationProperty::None,
                face_tints: [[u16::MAX; 3]; 6],
            })
            .unwrap();
        let profile = registry
            .compile(&GameProfile {
                id: rustcraft_game_api::ContentId::parse("test:profile/default").unwrap(),
                packages: vec![package],
                resources: vec![],
                systems: vec![],
                manifest: Default::default(),
                default_block: block_key,
            })
            .unwrap();
        let render = CompiledVoxelRenderRegistry::compile(&profile, &textures).unwrap();
        assert!(render.visible(profile.default_state().block));
        assert_eq!(
            render
                .texture(profile.default_state().block, Face::Top)
                .unwrap()
                .texture,
            TextureHandle(0)
        );
        let water = render.block(profile.block_id(&water_key).unwrap()).unwrap();
        assert!(!water.opaque);
        assert!(water.translucent);
        assert!(!water.selectable);
        assert_eq!(water.liquid_surface_height, Some(0.875));
    }
}
