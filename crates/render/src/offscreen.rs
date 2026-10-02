//! Surface-independent deterministic rendering. Scene vertices use clip coordinates.
//! The caller resolves semantic content; this module knows no first-party block IDs.
use super::*;
use wgpu::util::DeviceExt;

pub struct Scene {
    pub vertices: Vec<Vertex>,
    pub width: u32,
    pub height: u32,
    pub textured: bool,
    pub cull: bool,
}

/// Project-owned UV chart: U=red, V=green, asymmetric colored corners and a top arrow.
pub fn uv_chart() -> Vec<u8> {
    let mut data = vec![0; 64 * 64 * 4];
    for y in 0..64 {
        for x in 0usize..64 {
            let mut c = [x as u8 * 4, y as u8 * 4, 80, 255];
            if x < 10 && y < 10 {
                c = [255, 0, 0, 255];
            }
            if x >= 54 && y < 10 {
                c = [0, 255, 0, 255];
            }
            if x >= 54 && y >= 54 {
                c = [0, 0, 255, 255];
            }
            if x < 10 && y >= 54 {
                c = [255, 255, 0, 255];
            }
            if (30..34).contains(&x) && (12..50).contains(&y)
                || (8..20).contains(&y) && x.abs_diff(32) < y - 7
            {
                c = [255; 4];
            }
            data[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4].copy_from_slice(&c);
        }
    }
    data
}

/// Offscreen color/depth attachments are explicit, never a window/surface copy.
pub async fn render(scene: &Scene, atlas: Option<&Path>, output: &Path) -> Result<(), String> {
    render_input(scene, None, atlas.map(AtlasInput::Path), output).await
}

pub async fn render_rgba(
    scene: &Scene,
    width: u32,
    height: u32,
    rgba: &[u8],
    output: &Path,
) -> Result<(), String> {
    render_input(
        scene,
        None,
        Some(AtlasInput::Rgba {
            width,
            height,
            rgba,
        }),
        output,
    )
    .await
}

/// Multi-page offscreen path used by synthetic renderer contract tests.
pub async fn render_pages_rgba(
    scene: &Scene,
    geometry: &[PageVertices],
    atlas_pages: &[RgbaTexture],
    output: &Path,
) -> Result<(), String> {
    render_input(
        scene,
        Some(geometry),
        Some(AtlasInput::RgbaPages(atlas_pages)),
        output,
    )
    .await
}

enum AtlasInput<'a> {
    Path(&'a Path),
    Rgba {
        width: u32,
        height: u32,
        rgba: &'a [u8],
    },
    RgbaPages(&'a [RgbaTexture]),
}

async fn render_input(
    scene: &Scene,
    page_geometry: Option<&[PageVertices]>,
    atlas: Option<AtlasInput<'_>>,
    output: &Path,
) -> Result<(), String> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .map_err(|e| e.to_string())?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("render-test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: Default::default(),
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|e| e.to_string())?;
    eprintln!(
        "offscreen adapter: {} ({:?})",
        adapter.get_info().name,
        adapter.get_info().backend
    );
    let textures = match atlas {
        Some(AtlasInput::Path(path)) => vec![load_texture(&device, &queue, path)?],
        Some(AtlasInput::Rgba {
            width,
            height,
            rgba,
        }) => vec![upload_texture(&device, &queue, width, height, rgba)],
        Some(AtlasInput::RgbaPages(pages)) => pages
            .iter()
            .map(|page| {
                upload_texture_with_sampling(
                    &device,
                    &queue,
                    page.width,
                    page.height,
                    &page.rgba,
                    page.sampler,
                )
            })
            .collect(),
        None => vec![upload_texture(&device, &queue, 64, 64, &uv_chart())],
    };
    if textures.is_empty() {
        return Err("offscreen render requires at least one atlas page".into());
    }
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shared production shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });
    let pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label:Some("offscreen triangles"),layout:None,
        vertex:wgpu::VertexState{module:&shader,entry_point:Some("vs_hud"),buffers:&[wgpu::VertexBufferLayout{array_stride:36,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3]}],compilation_options:Default::default()},
        fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some(if scene.textured {"fs_inspect"}else{"fs_color"}),targets:&[Some(wgpu::ColorTargetState{format:wgpu::TextureFormat::Rgba8UnormSrgb,blend:None,write_mask:wgpu::ColorWrites::ALL})],compilation_options:Default::default()}),
        primitive:wgpu::PrimitiveState{cull_mode:scene.cull.then_some(wgpu::Face::Back),..Default::default()},
        depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:true,depth_compare:wgpu::CompareFunction::Less,stencil:Default::default(),bias:Default::default()}),multisample:Default::default(),multiview:None,cache:None,
    });
    let binds = scene.textured.then(|| {
        textures
            .iter()
            .map(|texture| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &pipeline.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&texture.sampler),
                        },
                    ],
                })
            })
            .collect::<Vec<_>>()
    });
    let size = wgpu::Extent3d {
        width: scene.width,
        height: scene.height,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("readable offscreen target"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let mut flattened = Vec::new();
    let mut ranges = Vec::new();
    if let Some(pages) = page_geometry {
        for page in pages {
            if page.texture.0 as usize >= textures.len() {
                return Err(format!(
                    "offscreen geometry references missing atlas page {}",
                    page.texture.0
                ));
            }
            let start = flattened.len() as u32;
            flattened.extend_from_slice(&page.vertices);
            ranges.push((page.texture, start..flattened.len() as u32));
        }
    } else {
        flattened.extend_from_slice(&scene.vertices);
        ranges.push((TextureHandle(0), 0..flattened.len() as u32));
    }
    let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&flattened),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let stride = (scene.width * 4).div_ceil(256) * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(stride) * u64::from(scene.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let view = target.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.12,
                        g: 0.14,
                        b: 0.18,
                        a: 1.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_vertex_buffer(0, vertices.slice(..));
        for (texture, range) in &ranges {
            if let Some(binds) = &binds {
                pass.set_bind_group(0, &binds[texture.0 as usize], &[]);
            }
            pass.draw(range.clone(), 0..1);
        }
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(scene.height),
            },
        },
        size,
    );
    queue.submit(Some(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device
        .poll(wgpu::PollType::Wait)
        .map_err(|e| e.to_string())?;
    rx.recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let mapped = readback.slice(..).get_mapped_range();
    let mut rgba = Vec::new();
    for row in mapped.chunks(stride as usize) {
        rgba.extend_from_slice(&row[..scene.width as usize * 4]);
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut png = png::Encoder::new(
        std::fs::File::create(output).map_err(|e| e.to_string())?,
        scene.width,
        scene.height,
    );
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&rgba)
        .map_err(|e| e.to_string())?;
    eprintln!("{}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MultiPage;
    impl BlockTextureResolver for MultiPage {
        fn texture(&self, block: BlockId, face: Face) -> Option<AtlasRegion> {
            (block.0 != 0).then(|| {
                let page = match face {
                    Face::Top => 0,
                    Face::East => 2,
                    _ => 1,
                };
                AtlasRegion::full(TextureHandle(page))
            })
        }
        fn opaque(&self, block: BlockId) -> bool {
            block.0 != 0
        }
    }

    fn model() -> inspection::BlockModel {
        inspection::BlockModel::resolve(BlockState::new(BlockId(1)), &MultiPage).unwrap()
    }

    fn append_transformed(
        output: &mut Vec<PageVertices>,
        input: &[PageVertices],
        transform: impl Fn([f32; 3]) -> [f32; 3],
    ) {
        for page in input {
            let out = page_vertices_mut(output, page.texture);
            out.extend(page.vertices.iter().copied().map(|mut vertex| {
                vertex.position = transform(vertex.position);
                vertex
            }));
        }
    }

    /// GPU-dependent by design; `just render-test-all` invokes this ignored offscreen contract.
    #[test]
    #[ignore = "requires an offscreen wgpu adapter"]
    fn chunk_dropped_and_gui_multi_page_paths_render_every_page() {
        let mut combined = Vec::new();

        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 0, y: 0, z: 0 }, BlockId(1));
        let extracted = RenderWorld::from_world(&world);
        let chunk = extracted.chunks().next().unwrap();
        let chunk_pages = build_chunk_mesh_pages(&extracted, chunk, &MultiPage)
            .into_iter()
            .map(|page| PageVertices {
                texture: page.texture,
                vertices: page
                    .mesh
                    .indices
                    .iter()
                    .map(|index| page.mesh.vertices[*index as usize])
                    .collect(),
            })
            .collect::<Vec<_>>();
        assert_eq!(chunk_pages.len(), 3);
        append_transformed(&mut combined, &chunk_pages, |p| {
            [
                -0.88 + p[0] * 0.24 + p[2] * 0.08,
                0.45 - p[1] * 0.5 + p[2] * 0.08,
                0.5,
            ]
        });

        let block_model = model();
        let mut dropped = Vec::new();
        append_dropped_item_pages(
            &mut dropped,
            &[ItemSprite {
                model: Some(block_model),
                position: Vec3::ZERO,
                top: block_model.texture(Face::Top),
                side: block_model.texture(Face::North),
                bottom: block_model.texture(Face::Bottom),
                tint: [1.; 3],
                age: 7.,
                count: 1,
                hover_start: 0.,
            }],
        );
        assert_eq!(dropped.len(), 3);
        append_transformed(&mut combined, &dropped, |p| {
            [p[0] * 1.2, p[1] * 1.2 - 0.1, 0.45 + p[2] * 0.1]
        });

        let mut gui = Vec::new();
        block_model.gui_page_vertices(&mut gui, [242., 34.], 3., [300., 100.]);
        assert_eq!(gui.len(), 3);
        append_transformed(&mut combined, &gui, |p| p);

        assert_eq!(
            combined.iter().map(|page| page.texture).collect::<Vec<_>>(),
            [TextureHandle(0), TextureHandle(1), TextureHandle(2)]
        );
        let atlas_pages =
            [[255, 32, 32, 255], [32, 255, 32, 255], [32, 32, 255, 255]].map(|color| RgbaTexture {
                width: 2,
                height: 2,
                rgba: color.repeat(4),
                sampler: TextureSampling::Nearest,
            });
        let output = std::env::temp_dir().join(format!(
            "rustcraft-multi-page-offscreen-{}.png",
            std::process::id()
        ));
        pollster::block_on(render_pages_rgba(
            &Scene {
                vertices: Vec::new(),
                width: 300,
                height: 100,
                textured: true,
                cull: false,
            },
            &combined,
            &atlas_pages,
            &output,
        ))
        .unwrap();
        let decoder = png::Decoder::new(std::fs::File::open(&output).unwrap());
        let mut reader = decoder.read_info().unwrap();
        let mut bytes = vec![0; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut bytes).unwrap();
        let pixels = bytes[..frame.buffer_size()].as_chunks::<4>().0.iter();
        for channel in 0..3 {
            assert!(pixels.clone().any(|pixel| {
                pixel[channel] > 200
                    && pixel[(channel + 1) % 3] < 80
                    && pixel[(channel + 2) % 3] < 80
            }));
        }
        std::fs::remove_file(output).unwrap();
    }
}
