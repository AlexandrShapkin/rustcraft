//! Synthetic semantic presentation proofs; no historical pixels are needed.
use super::*;
use rustcraft_content::{PackageId, ResourceId, resources::*};
use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
fn write(path: &Path, width: u32, height: u32, pixels: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(pixels)
        .unwrap();
}
fn read(path: &Path) -> (u32, u32, Vec<u8>) {
    let mut reader = png::Decoder::new(std::fs::File::open(path).unwrap())
        .read_info()
        .unwrap();
    let mut bytes = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut bytes).unwrap();
    bytes.truncate(info.buffer_size());
    (info.width, info.height, bytes)
}
fn pixels(compiled: &CompiledResources, id: &ResourceId) -> Vec<u8> {
    let resource = compiled.texture(id).unwrap();
    let page = &compiled.pages[resource.page as usize];
    let rect = resource.pixel_rect;
    (rect.y..rect.y + rect.height)
        .flat_map(|y| {
            let start = ((y * page.width + rect.x) * 4) as usize;
            page.rgba[start..start + rect.width as usize * 4]
                .iter()
                .copied()
        })
        .collect()
}
struct Fixture {
    root: PathBuf,
    legacy: ResourcePackage,
    rearranged: ResourcePackage,
    direct: ResourcePackage,
    ids: Vec<ResourceId>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "rustcraft-r2-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let original = root.join("legacy");
        for (file, w, h) in [
            ("terrain.png", 256, 256),
            ("gui/gui.png", 256, 256),
            ("gui/inventory.png", 256, 256),
            ("mob/char.png", 64, 32),
        ] {
            let pixels = (0..w * h)
                .flat_map(|i| [(i % w) as u8, (i / w) as u8, (i * 13) as u8, 255])
                .collect::<Vec<_>>();
            write(&original.join(file), w, h, &pixels);
        }
        let imported =
            rustcraft_minecraft_b173::legacy_resource_package(&original.join("terrain.png"))
                .unwrap();
        let ids = std::iter::once(rustcraft_minecraft_b173::resource_keys::inventory())
            .chain([
                rustcraft_minecraft_b173::resource_keys::hotbar(),
                rustcraft_minecraft_b173::resource_keys::selector(),
            ])
            .chain((0..6).map(rustcraft_minecraft_b173::resource_keys::player_part))
            .collect::<Vec<_>>();
        let mut legacy = ResourcePackage::new(imported.id.clone(), &original);
        let moved_root = root.join("rearranged");
        let mut rearranged =
            ResourcePackage::new(PackageId::parse("fixture:rearranged").unwrap(), &moved_root);
        for (index, id) in ids.iter().enumerate() {
            let texture = imported
                .textures()
                .iter()
                .find(|texture| &texture.id == id)
                .unwrap();
            let rect = texture.source.crop.unwrap();
            legacy
                .add_texture(
                    id.clone(),
                    texture.source.path.strip_prefix(&original).unwrap(),
                    Some(rect),
                    SamplerPolicy::Nearest,
                )
                .unwrap();
            let (width, _, source) = read(&original.join(&texture.source.path));
            let pixels = (rect.y..rect.y + rect.height)
                .flat_map(|y| {
                    let start = ((y * width + rect.x) * 4) as usize;
                    source[start..start + rect.width as usize * 4]
                        .iter()
                        .copied()
                })
                .collect::<Vec<_>>();
            // Different physical sheet dimensions, offsets and filename for each semantic region.
            let w = rect.width + 47;
            let h = rect.height + 59;
            let mut moved = vec![0; (w * h * 4) as usize];
            for y in 0..rect.height {
                let dest = (((y + 23) * w + 19) * 4) as usize;
                let src = (y * rect.width * 4) as usize;
                moved[dest..dest + rect.width as usize * 4]
                    .copy_from_slice(&pixels[src..src + rect.width as usize * 4]);
            }
            let filename = format!("moved-{index}.png");
            write(&moved_root.join(&filename), w, h, &moved);
            rearranged
                .add_texture(
                    id.clone(),
                    &filename,
                    Some(PixelRect {
                        x: 19,
                        y: 23,
                        width: rect.width,
                        height: rect.height,
                    }),
                    SamplerPolicy::Nearest,
                )
                .unwrap();
            let (namespace, semantic) = id.as_str().split_once(':').unwrap();
            write(
                &root
                    .join("direct/assets")
                    .join(namespace)
                    .join(format!("{semantic}.png")),
                rect.width,
                rect.height,
                &pixels,
            );
        }
        let direct = discover_package(
            root.join("direct"),
            PackageId::parse("fixture:direct").unwrap(),
            ResourceLimits::default(),
        )
        .unwrap();
        Self {
            root,
            legacy,
            rearranged,
            direct,
            ids,
        }
    }
    fn compile(&self, packages: &[ResourcePackage], cache: bool) -> CompiledResources {
        compile_resources(
            packages,
            AtlasPolicy {
                page_width: 256,
                page_height: 256,
                ..Default::default()
            },
            ResourceLimits::default(),
            cache.then(|| self.root.join("cache")).as_deref(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn r2_semantic_rearranged_direct_override_and_cache() {
    let f = Fixture::new();
    let baseline = f.compile(std::slice::from_ref(&f.legacy), false);
    let moved = f.compile(std::slice::from_ref(&f.rearranged), true);
    let direct = f.compile(std::slice::from_ref(&f.direct), false);
    for id in &f.ids {
        assert_eq!(pixels(&baseline, id), pixels(&moved, id));
        assert_eq!(pixels(&baseline, id), pixels(&direct, id));
    }
    assert!(!moved.metrics.cache_hit);
    assert!(
        f.compile(std::slice::from_ref(&f.rearranged), true)
            .metrics
            .cache_hit
    );
    let override_id = rustcraft_minecraft_b173::resource_keys::selector();
    let mut override_package = ResourcePackage::new(
        PackageId::parse("fixture:override").unwrap(),
        f.root.join("override"),
    );
    write(
        &f.root.join("override/selector.png"),
        24,
        22,
        &[27, 83, 191, 255].repeat(24 * 22),
    );
    override_package
        .add_texture(
            override_id.clone(),
            "selector.png",
            None,
            SamplerPolicy::Nearest,
        )
        .unwrap();
    let overridden = f.compile(&[f.legacy.clone(), override_package], false);
    assert_eq!(
        pixels(&overridden, &override_id),
        [27, 83, 191, 255].repeat(24 * 22)
    );
    assert_eq!(overridden.overrides.len(), 1);
    for id in f.ids.iter().filter(|id| **id != override_id) {
        assert_eq!(pixels(&overridden, id), pixels(&baseline, id));
    }
    let path = f.root.join("rearranged/moved-2.png");
    let (w, h, mut data) = read(&path);
    let offset = ((23 * w + 19) * 4) as usize;
    data[offset] ^= 0xff;
    write(&path, w, h, &data);
    let changed = f.compile(std::slice::from_ref(&f.rearranged), true);
    assert!(!changed.metrics.cache_hit);
    assert_ne!(
        pixels(&changed, &override_id),
        pixels(&baseline, &override_id)
    );
}

fn render_fixture(
    f: &Fixture,
    compiled: &CompiledResources,
    open: bool,
    separate_pages: bool,
    output: &Path,
) {
    use rustcraft_render::{
        PageVertices, TextureHandle,
        hud::{HudGeometry, HudSnapshot},
        offscreen,
    };
    let mut pages = compiled
        .pages
        .iter()
        .map(|page| RgbaTexture {
            width: page.width,
            height: page.height,
            rgba: page.rgba.clone(),
            sampler: TextureSampling::Nearest,
        })
        .collect::<Vec<_>>();
    let mut regions = f
        .ids
        .iter()
        .map(|id| {
            let texture = compiled.texture(id).unwrap();
            AtlasRegion {
                texture: TextureHandle(texture.page),
                uv_min: texture.uv_min,
                uv_max: texture.uv_max,
            }
        })
        .collect::<Vec<_>>();
    if separate_pages {
        pages.clear();
        for (index, id) in f.ids.iter().enumerate() {
            let rect = compiled.texture(id).unwrap().pixel_rect;
            pages.push(RgbaTexture {
                width: rect.width,
                height: rect.height,
                rgba: pixels(compiled, id),
                sampler: TextureSampling::Nearest,
            });
            regions[index] = AtlasRegion::full(TextureHandle(index as u32));
        }
    }
    let mut geometry = HudGeometry::default();
    let camera = Camera {
        position: Vec3::ZERO,
        yaw: 0.,
        pitch: 0.,
        aspect: 4. / 3.,
        fov_y: 1.,
        near: 0.05,
        far: 100.,
    };
    geometry.build(
        &HudSnapshot {
            inventory_open: open,
            ..Default::default()
        },
        800,
        600,
        camera,
    );
    geometry.build_gui_background(800, 600, open, 3);
    let mut draws = Vec::new();
    if open {
        rustcraft_render::resolve_presentation_quads(&mut geometry.gui_vertices, &regions[..1]);
        draws.push(PageVertices {
            texture: regions[0].texture,
            vertices: geometry.gui_vertices,
        });
        for (texture, range) in rustcraft_render::resolve_presentation_quads(
            &mut geometry.player_vertices,
            &regions[3..],
        ) {
            draws.push(PageVertices {
                texture,
                vertices: geometry.player_vertices[range.start as usize..range.end as usize]
                    .to_vec(),
            });
        }
        assert_eq!(draws.len(), 7);
    } else {
        for (texture, range) in rustcraft_render::resolve_presentation_quads(
            &mut geometry.hotbar_vertices,
            &regions[1..3],
        ) {
            draws.push(PageVertices {
                texture,
                vertices: geometry.hotbar_vertices[range.start as usize..range.end as usize]
                    .to_vec(),
            });
        }
        assert_eq!(draws.len(), 2);
    }
    // The reusable offscreen inspector has a depth test. Give painter-ordered GUI quads
    // descending depth so overlays are visible, matching the production GUI pass without depth.
    for (index, draw) in draws.iter_mut().enumerate() {
        for vertex in &mut draw.vertices {
            vertex.position[2] = 0.8 - index as f32 * 0.01;
        }
    }
    let scene = offscreen::Scene {
        vertices: vec![],
        width: 800,
        height: 600,
        textured: true,
        cull: false,
    };
    pollster::block_on(offscreen::render_pages_rgba(&scene, &draws, &pages, output)).unwrap();
}

#[test]
#[ignore = "requires a wgpu adapter; explicit R2 offscreen acceptance"]
fn r2_offscreen_semantic_layout_and_page_equivalence() {
    let f = Fixture::new();
    let original = f.compile(std::slice::from_ref(&f.legacy), false);
    let rearranged = f.compile(std::slice::from_ref(&f.rearranged), false);
    let direct = f.compile(std::slice::from_ref(&f.direct), false);
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/r2/offscreen");
    std::fs::create_dir_all(&output).unwrap();
    for open in [false, true] {
        let scene = if open { "inventory" } else { "hotbar" };
        let base = output.join(format!("{scene}-legacy.png"));
        render_fixture(&f, &original, open, false, &base);
        let (_, _, expected) = read(&base);
        let (x, y) = if open {
            (336usize, 186usize)
        } else {
            (360usize, 576usize)
        };
        let sample = &expected[(y * 800 + x) * 4..(y * 800 + x) * 4 + 4];
        if open {
            assert!(
                sample[0] < 20 && sample[1] < 20,
                "preview head must actually be visible: {sample:?}"
            );
        } else {
            assert!(
                sample[0] < 24 && sample[1] >= 22,
                "selector must actually overlay hotbar: {sample:?}"
            );
        }
        for (name, compiled, separate) in [
            ("rearranged", &rearranged, false),
            ("direct", &direct, false),
            ("nine-pages", &direct, true),
        ] {
            let image = output.join(format!("{scene}-{name}.png"));
            render_fixture(&f, compiled, open, separate, &image);
            assert_eq!(read(&image).2, expected, "{scene} differs for {name}");
        }
    }
}
