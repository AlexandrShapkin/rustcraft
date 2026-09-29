//! Presentation extraction, CPU voxel meshing and the wgpu presentation backend.
//! Simulation never depends on this crate.

use rustcraft_engine_core::{
    BlockId, BlockPos, BlockState, CHUNK_SIZE, ChunkPos, Vec3, World, block_index,
};
use std::{collections::HashMap, io::BufReader, path::Path, sync::Arc};

pub mod diagnostic;
pub mod geometry;
pub mod hud;
pub mod inspection;
pub mod offscreen;
mod timing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    North,
    South,
    East,
    West,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureTile {
    pub x: u8,
    pub y: u8,
}

pub trait BlockTextureResolver {
    /// Independent hooks: whole-model orientation and state-dependent material selection.
    fn model_rotation(
        &self,
        _state: BlockState,
    ) -> rustcraft_engine_core::orientation::ModelRotation {
        rustcraft_engine_core::orientation::ModelRotation::IDENTITY
    }
    fn state_texture(&self, state: BlockState, face: Face) -> Option<TextureTile> {
        self.texture(state.block, face)
    }

    fn texture(&self, block: BlockId, face: Face) -> Option<TextureTile>;
    fn opaque(&self, block: BlockId) -> bool;
    fn visible(&self, block: BlockId) -> bool {
        self.opaque(block)
    }
    /// Linear RGB material tint, independent of face brightness/lighting.
    fn tint(&self, _block: BlockId, _face: Face) -> [f32; 3] {
        [1.0; 3]
    }
}

#[derive(Debug, Clone)]
pub struct RenderChunk {
    pub position: ChunkPos,
    pub section_y: i32,
    pub blocks: Vec<BlockState>,
}

#[derive(Debug, Default)]
pub struct RenderWorld {
    chunks: HashMap<(ChunkPos, i32), RenderChunk>,
    lights: HashMap<BlockPos, rustcraft_engine_core::VoxelLight>,
}

impl RenderWorld {
    pub fn sync_sections(
        &mut self,
        world: &World,
        dirty: impl IntoIterator<Item = rustcraft_engine_core::SectionPos>,
    ) {
        for (pos, y) in dirty {
            self.copy_chunk(world, pos, y);
        }
    }
    #[must_use]
    pub fn from_world(world: &World) -> Self {
        let mut presentation = Self::default();
        for (position, section_y) in world.section_positions() {
            presentation.copy_chunk(world, position, section_y);
        }
        presentation
    }
    pub fn sync_dirty(&mut self, world: &World, dirty: impl IntoIterator<Item = ChunkPos>) {
        for position in dirty {
            let sections = world
                .section_positions()
                .filter_map(|(candidate, section_y)| (candidate == position).then_some(section_y))
                .collect::<Vec<_>>();
            for section_y in sections {
                self.copy_chunk(world, position, section_y);
            }
        }
    }
    fn copy_chunk(&mut self, world: &World, position: ChunkPos, section_y: i32) {
        if let Some(chunk) = world.section(position, section_y) {
            for y in -1..=16 {
                for z in -1..=16 {
                    for x in -1..=16 {
                        let p = BlockPos {
                            x: position.x * 16 + x,
                            y: section_y * 16 + y,
                            z: position.z * 16 + z,
                        };
                        self.lights.insert(p, world.light(p));
                    }
                }
            }
            let mut blocks = Vec::with_capacity(4096);
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        blocks.push(chunk.state((x, y, z)));
                    }
                }
            }
            self.chunks.insert(
                (position, section_y),
                RenderChunk {
                    position,
                    section_y,
                    blocks,
                },
            );
        } else {
            self.chunks.remove(&(position, section_y));
        }
    }
    pub fn chunks(&self) -> impl Iterator<Item = &RenderChunk> {
        self.chunks.values()
    }
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
    #[must_use]
    pub fn block(&self, position: BlockPos) -> BlockId {
        let cx = position.x.div_euclid(CHUNK_SIZE);
        let cz = position.z.div_euclid(CHUNK_SIZE);
        let key = ChunkPos { x: cx, z: cz };
        let section_y = position.y.div_euclid(16);
        self.chunks
            .get(&(key, section_y))
            .map_or(BlockId(0), |chunk| {
                chunk.blocks[block_index((
                    position.x.rem_euclid(16) as u8,
                    position.y.rem_euclid(16) as u8,
                    position.z.rem_euclid(16) as u8,
                ))]
                .block
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub aspect: f32,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    #[must_use]
    pub fn view_projection(self) -> [[f32; 4]; 4] {
        // CPU helpers use rows; upload columns once. WGSL: P * V * I * position.
        transpose(matrix_mul(projection(self), view(self)))
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        if width != 0 && height != 0 {
            self.aspect = width as f32 / height as f32;
        }
    }
    #[must_use]
    pub fn forward(self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            -self.pitch.sin(),
            self.yaw.cos() * self.pitch.cos(),
        )
    }
    #[must_use]
    pub fn basis(self) -> (Vec3, Vec3, Vec3) {
        let forward = self.forward();
        let right = normalize(cross(forward, Vec3::new(0.0, 1.0, 0.0)));
        let up = cross(right, forward);
        (right, up, forward)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub shade: f32,
    pub color: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct ItemSprite {
    pub model: Option<inspection::BlockModel>,
    pub position: Vec3,
    pub top: TextureTile,
    pub side: TextureTile,
    pub bottom: TextureTile,
    pub tint: [f32; 3],
    pub age: f32,
    pub count: u16,
    pub hover_start: f32,
}

#[must_use]
pub fn beta_item_bob(age_ticks: f32, hover_start: f32) -> f32 {
    (age_ticks / 10.0 + hover_start).sin() * 0.1 + 0.1
}

#[must_use]
pub fn beta_item_rotation_degrees(age_ticks: f32, hover_start: f32) -> f32 {
    (age_ticks / 20.0 + hover_start) * 57.295776
}

/// Shared production/inspector world-space item extraction; animation never mutates simulation.
pub fn append_dropped_items(vertices: &mut Vec<Vertex>, sprites: &[ItemSprite]) {
    for sprite in sprites {
        let p = sprite.position;
        // Beta 1.7.3 RenderItem.doRenderItem bob/rotation formulas.
        let bob = beta_item_bob(sprite.age, sprite.hover_start);
        let angle = beta_item_rotation_degrees(sprite.age, sprite.hover_start).to_radians();
        let (sin, cos) = angle.sin_cos();
        let scale = 0.25_f32;
        let h = scale * 0.5;
        let copies = if sprite.count > 20 {
            4
        } else if sprite.count > 5 {
            3
        } else if sprite.count > 1 {
            2
        } else {
            1
        };
        for copy in 0..copies {
            let mut offset = [0.0; 3];
            if copy > 0 {
                let mut rng = 187_u32.wrapping_add(copy as u32 * 747796405);
                let next = |r: &mut u32| {
                    *r = r.wrapping_mul(1664525).wrapping_add(1013904223);
                    ((*r >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0
                };
                offset = [
                    next(&mut rng) * 0.2 / scale,
                    next(&mut rng) * 0.2 / scale,
                    next(&mut rng) * 0.2 / scale,
                ];
            }
            let transform = |v: [f32; 3]| {
                let x = v[0] + offset[0];
                let z = v[2] + offset[2];
                [
                    p.x + x * cos - z * sin,
                    p.y + bob + v[1] + offset[1],
                    p.z + x * sin + z * cos,
                ]
            };
            for definition in geometry::FACES {
                let direction = definition.direction;
                let shade = match direction {
                    Face::Top => 1.,
                    Face::Bottom => 0.55,
                    Face::East | Face::West => 0.72,
                    _ => 0.82,
                };
                let face = definition.positions.map(|p| {
                    let p = p.map(|v| (v - 0.5) * 2. * h);
                    sprite.model.map_or(p, |m| m.rotation.transform(p))
                });
                let uvs = definition.uv_corners;
                let tile = sprite.model.map_or(
                    match direction {
                        Face::Top => sprite.top,
                        Face::Bottom => sprite.bottom,
                        _ => sprite.side,
                    },
                    |m| m.texture(direction),
                );
                let tint = sprite
                    .model
                    .map_or(sprite.tint, |m| m.tints[direction as usize]);
                let uv = uvs.map(|uv| tile_uv(tile, uv));
                for (i, u) in [
                    (0, uv[0]),
                    (1, uv[1]),
                    (2, uv[2]),
                    (0, uv[0]),
                    (2, uv[2]),
                    (3, uv[3]),
                ] {
                    vertices.push(Vertex {
                        position: transform(face[i]),
                        uv: u,
                        shade,
                        color: tint,
                    });
                }
            }
        }
    }
}

/// Centered adapter over the canonical mesh, in historical inventory face order.
#[must_use]
#[allow(clippy::type_complexity)]
pub fn beta_item_cube_faces(h: f32) -> [([[f32; 3]; 4], [[f32; 2]; 4], f32); 6] {
    [
        (Face::Bottom, 0.55),
        (Face::Top, 1.),
        (Face::South, 0.82),
        (Face::North, 0.82),
        (Face::West, 0.72),
        (Face::East, 0.72),
    ]
    .map(|(direction, shade)| {
        let f = geometry::definition(direction);
        (
            f.positions.map(|p| p.map(|v| (v - 0.5) * 2. * h)),
            f.uv_corners,
            shade,
        )
    })
}

#[derive(Debug, Default, Clone)]
pub struct CpuMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl CpuMesh {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

pub fn build_chunk_mesh(
    world: &RenderWorld,
    chunk: &RenderChunk,
    resolver: &impl BlockTextureResolver,
) -> CpuMesh {
    let mut mesh = CpuMesh::default();
    let origin = section_origin(chunk.position, chunk.section_y);
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let state = chunk.blocks[block_index((x, y, z))];
                let block = state.block;
                let rotation = resolver.model_rotation(state);
                if !resolver.visible(block) {
                    continue;
                }
                let position = BlockPos {
                    x: origin.x + i32::from(x),
                    y: origin.y + i32::from(y),
                    z: origin.z + i32::from(z),
                };
                for model_face in geometry::FACES {
                    let normal = rotation.transform(model_face.normal);
                    let face = geometry::FACES
                        .iter()
                        .find(|f| f.normal == normal)
                        .expect("orthogonal model rotation")
                        .direction;
                    let (_, neighbor, shade) = faces(position)[face as usize];
                    if resolver.opaque(world.block(neighbor))
                        || (!resolver.opaque(block) && world.block(neighbor) == block)
                    {
                        continue;
                    }
                    if let Some(tile) = resolver.state_texture(state, model_face.direction) {
                        let start = mesh.vertices.len();
                        let light = world.lights.get(&neighbor).map_or(1.0, |l| {
                            let own = world.lights.get(&position).map_or(0, |v| v.block());
                            0.05 + 0.95 * f32::from(l.sky().max(l.block()).max(own)) / 15.0
                        });
                        append_face(
                            &mut mesh,
                            position,
                            model_face.direction,
                            tile,
                            shade * light,
                        );
                        for v in &mut mesh.vertices[start..] {
                            let local = [
                                v.position[0] - position.x as f32,
                                v.position[1] - position.y as f32,
                                v.position[2] - position.z as f32,
                            ];
                            let p = rotation.point(local);
                            v.position = [
                                p[0] + position.x as f32,
                                p[1] + position.y as f32,
                                p[2] + position.z as f32,
                            ];
                        }
                        for vertex in &mut mesh.vertices[start..] {
                            vertex.color = resolver.tint(block, model_face.direction);
                        }
                    }
                }
            }
        }
    }
    mesh
}

#[must_use]
pub fn section_origin(position: ChunkPos, section_y: i32) -> BlockPos {
    BlockPos {
        x: position.x * CHUNK_SIZE,
        y: section_y * CHUNK_SIZE,
        z: position.z * CHUNK_SIZE,
    }
}

// PNG and texture coordinates both start at top-left; no V flip.
fn tile_uv(tile: TextureTile, uv: [f32; 2]) -> [f32; 2] {
    assert!(tile.x < 16 && tile.y < 16);
    [
        (f32::from(tile.x) + uv[0]) / 16.0,
        (f32::from(tile.y) + uv[1]) / 16.0,
    ]
}

fn faces(position: BlockPos) -> [(Face, BlockPos, f32); 6] {
    [
        (
            Face::North,
            BlockPos {
                z: position.z + 1,
                ..position
            },
            0.82,
        ),
        (
            Face::South,
            BlockPos {
                z: position.z - 1,
                ..position
            },
            0.82,
        ),
        (
            Face::East,
            BlockPos {
                x: position.x + 1,
                ..position
            },
            0.72,
        ),
        (
            Face::West,
            BlockPos {
                x: position.x - 1,
                ..position
            },
            0.72,
        ),
        (
            Face::Top,
            BlockPos {
                y: position.y + 1,
                ..position
            },
            1.0,
        ),
        (
            Face::Bottom,
            BlockPos {
                y: position.y - 1,
                ..position
            },
            0.55,
        ),
    ]
}

fn append_face(mesh: &mut CpuMesh, position: BlockPos, face: Face, tile: TextureTile, shade: f32) {
    let x = position.x as f32;
    let y = position.y as f32;
    let z = position.z as f32;
    let n = mesh.vertices.len() as u32;
    let definition = geometry::definition(face);
    let corners = definition.positions.map(|p| [x + p[0], y + p[1], z + p[2]]);
    let uv = definition.uv_corners;
    for i in 0..4 {
        mesh.vertices.push(Vertex {
            position: corners[i],
            uv: tile_uv(tile, uv[i]),
            shade,
            color: [1.0; 3],
        });
    }
    mesh.indices
        .extend_from_slice(&[n, n + 1, n + 2, n, n + 2, n + 3]);
}

#[derive(Debug)]
pub struct Texture {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}
#[derive(Debug)]
pub struct GpuChunk {
    pub vertex: wgpu::Buffer,
    pub index: wgpu::Buffer,
    pub index_count: u32,
    vertex_count: usize,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    hud_pipeline: wgpu::RenderPipeline,
    gui_item_pipeline: wgpu::RenderPipeline,
    gui_pipeline: wgpu::RenderPipeline,
    crack_pipeline: wgpu::RenderPipeline,
    hud_geometry: hud::HudGeometry,
    hud_buffer: wgpu::Buffer,
    gui_bind: wgpu::BindGroup,
    hotbar_bind: wgpu::BindGroup,
    player_bind: wgpu::BindGroup,
    item_buffer: wgpu::Buffer,
    item_vertices: usize,
    crack_buffer: wgpu::Buffer,
    crack_vertices: usize,
    pub adapter_info: wgpu::AdapterInfo,
    timing: Option<timing::GpuTiming>,
    pub telemetry_enabled: bool,
    camera_buffer: wgpu::Buffer,
    camera_bind: wgpu::BindGroup,
    _texture: Texture,
    _gui_texture: Texture,
    _hotbar_texture: Texture,
    _player_texture: Texture,
    depth_view: wgpu::TextureView,
    chunks: HashMap<(ChunkPos, i32), GpuChunk>,
    pub mesh_rebuilds: u64,
    pub vertices: usize,
    pub indices: usize,
    diagnostic: Option<diagnostic::Stage>,
    projection_logged: bool,
}

impl Renderer {
    pub async fn new(
        window: Arc<winit::window::Window>,
        texture_path: &Path,
    ) -> Result<Self, String> {
        Self::new_with_stage(window, texture_path, None).await
    }

    pub async fn new_diagnostic(
        window: Arc<winit::window::Window>,
        texture_path: &Path,
        stage: diagnostic::Stage,
    ) -> Result<Self, String> {
        Self::new_with_stage(window, texture_path, Some(stage)).await
    }

    async fn new_with_stage(
        window: Arc<winit::window::Window>,
        texture_path: &Path,
        diagnostic: Option<diagnostic::Stage>,
    ) -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("create surface: {e}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("request adapter: {e}"))?;
        let info = adapter.get_info();
        eprintln!("graphics adapter={} backend={:?}", info.name, info.backend);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("rustcraft-device"),
                required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| format!("request device: {e}"))?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | (caps.usages & wgpu::TextureUsages::COPY_SRC),
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: caps
                .present_modes
                .iter()
                .copied()
                .find(|m| *m == wgpu::PresentMode::Fifo)
                .unwrap_or(caps.present_modes[0]),
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let texture = if diagnostic.is_some_and(|s| !s.uses_atlas()) {
            diagnostic::checker(&device, &queue)
        } else {
            load_texture(&device, &queue, texture_path)?
        };
        let gui_path = texture_path
            .parent()
            .unwrap_or_else(|| Path::new("reference/assets"))
            .join("gui/inventory.png");
        let gui_texture = load_texture(&device, &queue, &gui_path)?;
        let hotbar_path = texture_path
            .parent()
            .unwrap_or_else(|| Path::new("reference/assets"))
            .join("gui/gui.png");
        let hotbar_texture = load_texture(&device, &queue, &hotbar_path)?;
        let player_path = texture_path
            .parent()
            .unwrap_or_else(|| Path::new("reference/assets"))
            .join("mob/char.png");
        let player_texture = load_texture(&device, &queue, &player_path)?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxel-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let camera_buffer = wgpu::util::DeviceExt::create_buffer_init(
            &device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("camera"),
                contents: bytemuck::bytes_of(&[[0.0_f32; 4]; 4]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        );
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera-texture-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let camera_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-texture-bind"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
        });
        let gui_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gui-texture-bind"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&gui_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&gui_texture.sampler),
                },
            ],
        });
        let hotbar_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hotbar-texture-bind"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&hotbar_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&hotbar_texture.sampler),
                },
            ],
        });
        let player_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("player-preview-texture-bind"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&player_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&player_texture.sampler),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxel-pipeline-layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxel-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(if diagnostic.is_some_and(|s| !s.textured()) { "fs_color" } else if diagnostic.is_some_and(|s| s != diagnostic::Stage::NormalLit) { "fs_unlit" } else { "fs_main" }),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: if diagnostic == Some(diagnostic::Stage::CubeNoCull) { None } else { Some(wgpu::Face::Back) },
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let depth_view = create_depth_view(&device, config.width, config.height);
        let hud_pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:Some("pixel-hud"),layout:Some(&pipeline_layout),
            vertex:wgpu::VertexState{module:&shader,entry_point:Some("vs_hud"),buffers:&[wgpu::VertexBufferLayout{array_stride:std::mem::size_of::<Vertex>() as u64,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3]}],compilation_options:Default::default()},
            fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some("fs_hud"),targets:&[Some(wgpu::ColorTargetState{format,blend:Some(wgpu::BlendState::ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})],compilation_options:Default::default()}),primitive:Default::default(),depth_stencil:None,multisample:Default::default(),multiview:None,cache:None});
        let hud_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bounded-hud"),
            size: 8 * 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let gui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("inventory-gui"), layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_hud"), buffers: &[wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3] }], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_hud"), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview: None, cache: None,
        });
        let gui_item_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gui-block-items"), layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_hud"), buffers: &[wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3] }], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_main"), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState{cull_mode:Some(wgpu::Face::Back),..Default::default()}, depth_stencil: Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth24Plus,depth_write_enabled:true,depth_compare:wgpu::CompareFunction::Less,stencil:Default::default(),bias:Default::default()}), multisample: Default::default(), multiview: None, cache: None,
        });
        let crack_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("block-crack-overlay"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32,3=>Float32x3] }], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_crack"), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState { color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Dst, dst_factor: wgpu::BlendFactor::Src, operation: wgpu::BlendOperation::Add }, alpha: wgpu::BlendComponent::REPLACE }), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: wgpu::TextureFormat::Depth24Plus, depth_write_enabled: false, depth_compare: wgpu::CompareFunction::LessEqual, stencil: Default::default(), bias: wgpu::DepthBiasState { constant: -3, slope_scale: -3.0, clamp: 0.0 } }),
            multisample: Default::default(), multiview: None, cache: None,
        });
        let item_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("world-item-sprites"),
            size: 512 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let crack_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("block-crack-overlay-buffer"),
            size: 256 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let adapter_info = adapter.get_info();
        let timing = timing::GpuTiming::new(&device);
        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            hud_pipeline,
            gui_item_pipeline,
            gui_pipeline,
            crack_pipeline,
            hud_geometry: hud::HudGeometry::default(),
            hud_buffer,
            gui_bind,
            hotbar_bind,
            player_bind,
            item_buffer,
            item_vertices: 0,
            crack_buffer,
            crack_vertices: 0,
            adapter_info,
            timing,
            telemetry_enabled: false,
            camera_buffer,
            camera_bind,
            _texture: texture,
            _gui_texture: gui_texture,
            _hotbar_texture: hotbar_texture,
            _player_texture: player_texture,
            depth_view,
            chunks: HashMap::new(),
            mesh_rebuilds: 0,
            vertices: 0,
            indices: 0,
            diagnostic,
            projection_logged: false,
        })
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = create_depth_view(&self.device, width, height);
        }
    }
    #[must_use]
    pub fn width(&self) -> u32 {
        self.config.width
    }
    pub fn set_hud(&mut self, snapshot: &hud::HudSnapshot, camera: Camera) {
        self.hud_geometry
            .build(snapshot, self.width(), self.height(), camera);
        self.hud_geometry.build_gui_background(
            self.width(),
            self.height(),
            snapshot.inventory_open,
            snapshot.selected,
        );
        let gui_bytes = bytemuck::cast_slice(&self.hud_geometry.gui_vertices);
        assert!(
            gui_bytes.len() <= 256 * 1024,
            "GUI geometry capacity exceeded"
        );
        if !gui_bytes.is_empty() {
            self.queue
                .write_buffer(&self.hud_buffer, 512 * 1024, gui_bytes);
        }
        let hotbar_bytes = bytemuck::cast_slice(&self.hud_geometry.hotbar_vertices);
        if !hotbar_bytes.is_empty() {
            self.queue
                .write_buffer(&self.hud_buffer, 768 * 1024, hotbar_bytes);
        }
        let player_bytes = bytemuck::cast_slice(&self.hud_geometry.player_vertices);
        if !player_bytes.is_empty() {
            self.queue
                .write_buffer(&self.hud_buffer, 896 * 1024, player_bytes);
        }
        let item_bytes = bytemuck::cast_slice(&self.hud_geometry.item_vertices);
        if !item_bytes.is_empty() {
            self.queue
                .write_buffer(&self.hud_buffer, 1024 * 1024, item_bytes);
        }
        let bytes = bytemuck::cast_slice(&self.hud_geometry.vertices);
        assert!(bytes.len() <= 256 * 1024, "HUD geometry capacity exceeded");
        self.queue.write_buffer(&self.hud_buffer, 0, bytes);
        if self.hud_geometry.debug_changed {
            let debug_bytes = bytemuck::cast_slice(&self.hud_geometry.debug_vertices);
            assert!(
                debug_bytes.len() <= (8 * 1024 - 256) * 1024,
                "debug text capacity exceeded"
            );
            if !debug_bytes.is_empty() {
                self.queue
                    .write_buffer(&self.hud_buffer, 256 * 1024, debug_bytes);
            }
        }
    }
    pub fn set_item_sprites(&mut self, sprites: &[ItemSprite]) {
        let mut vertices = Vec::with_capacity(sprites.len() * 36);
        append_dropped_items(&mut vertices, sprites);
        self.item_vertices = vertices.len();
        if !vertices.is_empty() {
            self.queue
                .write_buffer(&self.item_buffer, 0, bytemuck::cast_slice(&vertices));
        }
    }
    pub fn item_render_count(&self) -> usize {
        self.item_vertices / 6
    }
    pub fn set_crack_overlay(&mut self, target: Option<BlockPos>, progress: Option<f32>) {
        let mut vertices = Vec::new();
        let Some(target) = target else {
            self.crack_vertices = 0;
            return;
        };
        let Some(progress) = progress else {
            self.crack_vertices = 0;
            return;
        };
        // Beta 1.7.3 RenderGlobal: terrain tiles 240..249 and block geometry override.
        let stage = hud::destroy_stage(progress);
        let crack_tile = TextureTile { x: stage, y: 15 };
        let o = [target.x as f32, target.y as f32, target.z as f32];
        for face in geometry::FACES {
            let quad = face.positions;
            let shade = match face.direction {
                Face::Top => 1.,
                Face::Bottom => 0.5,
                Face::East | Face::West => 0.7,
                _ => 0.8,
            };
            // Accepted destroy material orientation retained; it is independent of block-item UVs.
            for (i, uv) in [
                (0, [0., 0.]),
                (1, [1., 0.]),
                (2, [1., 1.]),
                (0, [0., 0.]),
                (2, [1., 1.]),
                (3, [0., 1.]),
            ] {
                vertices.push(Vertex {
                    position: [o[0] + quad[i][0], o[1] + quad[i][1], o[2] + quad[i][2]],
                    uv: crate::tile_uv(crack_tile, uv),
                    shade,
                    color: [1.; 3],
                });
            }
        }
        self.crack_vertices = vertices.len();
        self.queue
            .write_buffer(&self.crack_buffer, 0, bytemuck::cast_slice(&vertices));
    }
    /// No frustum culling yet: every nonempty resident mesh is submitted.
    pub fn rendered_sections(&self) -> usize {
        self.chunks.values().filter(|c| c.index_count > 0).count()
    }
    pub fn draw_calls(&self) -> usize {
        self.rendered_sections()
            + usize::from(!self.hud_geometry.vertices.is_empty())
            + usize::from(!self.hud_geometry.debug_vertices.is_empty())
    }
    pub fn mesh_count(&self) -> usize {
        self.chunks.len()
    }
    pub fn gpu_ms(&self) -> Option<f64> {
        self.timing.as_ref().and_then(|t| t.milliseconds)
    }
    pub fn surface_description(&self) -> String {
        format!(
            "{}X{} {:?} {:?}",
            self.width(),
            self.height(),
            self.config.present_mode,
            self.config.format
        )
    }
    pub fn config_present_mode(&self) -> wgpu::PresentMode {
        self.config.present_mode
    }
    #[must_use]
    pub fn height(&self) -> u32 {
        self.config.height
    }
    pub fn upload_chunk(&mut self, position: ChunkPos, section_y: i32, mesh: &CpuMesh) {
        use wgpu::util::DeviceExt;
        let vertex = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("chunk-vertices"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("chunk-indices"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        if let Some(old) = self.chunks.remove(&(position, section_y)) {
            self.vertices = self.vertices.saturating_sub(old.vertex_count);
            self.indices = self.indices.saturating_sub(old.index_count as usize);
        }
        self.vertices += mesh.vertices.len();
        self.indices += mesh.indices.len();
        self.chunks.insert(
            (position, section_y),
            GpuChunk {
                vertex,
                index,
                index_count: mesh.indices.len() as u32,
                vertex_count: mesh.vertices.len(),
            },
        );
        self.mesh_rebuilds += 1;
    }
    pub fn remove_chunk(&mut self, position: ChunkPos) {
        let positions = self
            .chunks
            .keys()
            .filter_map(|(candidate, section_y)| {
                (*candidate == position).then_some((*candidate, *section_y))
            })
            .collect::<Vec<_>>();
        for key in positions {
            let Some(old) = self.chunks.remove(&key) else {
                continue;
            };
            self.vertices = self.vertices.saturating_sub(old.vertex_count);
            self.indices = self.indices.saturating_sub(old.index_count as usize);
        }
    }
    pub fn render(&mut self, camera: Camera) -> Result<(), wgpu::SurfaceError> {
        self.render_capture(camera, None)
    }

    /// Optional one-frame development capture of the actual surface render.
    pub fn render_capture(
        &mut self,
        camera: Camera,
        capture: Option<&Path>,
    ) -> Result<(), wgpu::SurfaceError> {
        if !self.projection_logged {
            eprintln!(
                "projection: window={}x{} aspect={} fov_radians={} near={} far={}",
                self.width(),
                self.height(),
                camera.aspect,
                camera.fov_y,
                camera.near,
                camera.far
            );
            self.projection_logged = true;
        }
        let matrix = self
            .diagnostic
            .filter(|s| !s.normal_world())
            .map_or_else(|| camera.view_projection(), |s| s.matrix(camera.aspect));
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&matrix));
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-encoder"),
            });
        if self.telemetry_enabled
            && let Some(t) = &mut self.timing
        {
            t.poll(&self.device, self.queue.get_timestamp_period());
        }
        let timed = self.telemetry_enabled && self.timing.as_ref().is_some_and(|t| t.ready());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("voxel-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.52,
                            g: 0.70,
                            b: 0.92,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: if timed {
                    self.timing
                        .as_ref()
                        .map(|t| wgpu::RenderPassTimestampWrites {
                            query_set: &t.queries,
                            beginning_of_pass_write_index: Some(0),
                            end_of_pass_write_index: self
                                .hud_geometry
                                .vertices
                                .is_empty()
                                .then_some(1),
                        })
                } else {
                    None
                },
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind, &[]);
            for chunk in self.chunks.values() {
                if chunk.index_count > 0 {
                    pass.set_vertex_buffer(0, chunk.vertex.slice(..));
                    pass.set_index_buffer(chunk.index.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..chunk.index_count, 0, 0..1);
                }
            }
            if self.item_vertices > 0 {
                pass.set_vertex_buffer(0, self.item_buffer.slice(..));
                pass.draw(0..self.item_vertices as u32, 0..1);
            }
        }
        if self.crack_vertices > 0 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("block-crack-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: None,
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.crack_pipeline);
            pass.set_bind_group(0, &self.camera_bind, &[]);
            pass.set_vertex_buffer(0, self.crack_buffer.slice(..));
            pass.draw(0..self.crack_vertices as u32, 0..1);
        }
        if !self.hud_geometry.gui_vertices.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("inventory-gui-background"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.gui_pipeline);
            pass.set_bind_group(0, &self.gui_bind, &[]);
            pass.set_vertex_buffer(0, self.hud_buffer.slice(512 * 1024..));
            pass.draw(0..self.hud_geometry.gui_vertices.len() as u32, 0..1);
        }
        if !self.hud_geometry.hotbar_vertices.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hotbar-background"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.gui_pipeline);
            pass.set_bind_group(0, &self.hotbar_bind, &[]);
            pass.set_vertex_buffer(0, self.hud_buffer.slice(768 * 1024..));
            pass.draw(0..self.hud_geometry.hotbar_vertices.len() as u32, 0..1);
        }
        if !self.hud_geometry.player_vertices.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("inventory-player-preview"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.gui_pipeline);
            pass.set_bind_group(0, &self.player_bind, &[]);
            pass.set_vertex_buffer(0, self.hud_buffer.slice(896 * 1024..));
            pass.draw(0..self.hud_geometry.player_vertices.len() as u32, 0..1);
        }
        if !self.hud_geometry.item_vertices.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gui-block-depth-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.gui_item_pipeline);
            pass.set_bind_group(0, &self.camera_bind, &[]);
            pass.set_vertex_buffer(0, self.hud_buffer.slice(1024 * 1024..));
            pass.draw(0..self.hud_geometry.item_vertices.len() as u32, 0..1);
        }
        if !self.hud_geometry.vertices.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hud-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: if timed {
                    self.timing
                        .as_ref()
                        .map(|t| wgpu::RenderPassTimestampWrites {
                            query_set: &t.queries,
                            beginning_of_pass_write_index: None,
                            end_of_pass_write_index: Some(1),
                        })
                } else {
                    None
                },
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.hud_pipeline);
            pass.set_bind_group(0, &self.camera_bind, &[]);
            pass.set_vertex_buffer(0, self.hud_buffer.slice(..));
            pass.draw(0..self.hud_geometry.vertices.len() as u32, 0..1);
            if !self.hud_geometry.debug_vertices.is_empty() {
                pass.set_vertex_buffer(0, self.hud_buffer.slice(256 * 1024..));
                pass.draw(0..self.hud_geometry.debug_vertices.len() as u32, 0..1);
            }
        }
        if timed {
            self.timing.as_ref().unwrap().resolve(&mut encoder);
        }
        let readback = capture.map(|_| {
            diagnostic::copy_surface(&self.device, &mut encoder, &output.texture, &self.config)
        });
        self.queue.submit(Some(encoder.finish()));
        if timed {
            self.timing.as_mut().unwrap().map();
        }
        if let (Some(path), Some(buffer)) = (capture, readback) {
            diagnostic::save_capture(&self.device, buffer, &self.config, path);
        }
        output.present();
        Ok(())
    }
}

fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth-buffer"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn load_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &Path,
) -> Result<Texture, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("unable to open texture {}: {e}; set RUSTCRAFT_TERRAIN_TEXTURE to a local 256x256 terrain atlas", path.display()))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("decode texture {}: {e}", path.display()))?;
    let mut data = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut data)
        .map_err(|e| format!("read texture {}: {e}", path.display()))?;
    if info.width == 0 || info.height == 0 {
        return Err(format!(
            "texture {} has invalid dimensions {}x{}",
            path.display(),
            info.width,
            info.height
        ));
    }
    let rgba = match info.color_type {
        png::ColorType::Rgba => data[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => data[..info.buffer_size()]
            .chunks(3)
            .filter(|pixel| pixel.len() == 3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        _ => {
            return Err(format!(
                "terrain texture {} must decode to RGB or RGBA PNG",
                path.display()
            ));
        }
    };
    eprintln!(
        "texture: path={} dimensions={}x{} origin=top-left sampler=nearest format=Rgba8UnormSrgb",
        path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .display(),
        info.width,
        info.height
    );
    Ok(upload_texture(
        device,
        queue,
        info.width,
        info.height,
        &rgba,
    ))
}

fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("terrain-atlas"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    Texture { view, sampler }
}

fn projection(c: Camera) -> [[f32; 4]; 4] {
    assert!(c.aspect.is_finite() && c.aspect > 0.0);
    assert!(c.fov_y.is_finite() && c.fov_y > 0.0 && c.fov_y < std::f32::consts::PI);
    assert!(c.near > 0.0 && c.far.is_finite() && c.far > c.near);
    let f = 1.0 / (c.fov_y * 0.5).tan();
    let nf = 1.0 / (c.near - c.far);
    [
        [f / c.aspect, 0., 0., 0.],
        [0., f, 0., 0.],
        [0., 0., c.far * nf, c.far * c.near * nf],
        [0., 0., -1., 0.],
    ]
}
fn view(c: Camera) -> [[f32; 4]; 4] {
    let (right, up, forward) = c.basis();
    view_from_basis(c.position, right, up, forward)
}

// Right-handed view: camera looks down -Z. These are ROWS, not GPU columns.
fn view_from_basis(eye: Vec3, right: Vec3, up: Vec3, forward: Vec3) -> [[f32; 4]; 4] {
    [
        [right.x, right.y, right.z, -dot(right, eye)],
        [up.x, up.y, up.z, -dot(up, eye)],
        [-forward.x, -forward.y, -forward.z, dot(forward, eye)],
        [0., 0., 0., 1.],
    ]
}
fn normalize(v: Vec3) -> Vec3 {
    let length = dot(v, v).sqrt();
    assert!(length > 0.0);
    Vec3::new(v.x / length, v.y / length, v.z / length)
}

fn look_at(eye: Vec3, target: Vec3) -> [[f32; 4]; 4] {
    let forward = normalize(Vec3::new(
        target.x - eye.x,
        target.y - eye.y,
        target.z - eye.z,
    ));
    let right = normalize(cross(forward, Vec3::new(0.0, 1.0, 0.0)));
    view_from_basis(eye, right, cross(right, forward), forward)
}
fn dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}
fn matrix_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut o = [[0.; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            for i in 0..4 {
                o[r][c] += a[r][i] * b[i][c];
            }
        }
    }
    o
}
fn transpose(matrix: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0; 4]; 4];
    for row in 0..4 {
        for column in 0..4 {
            result[column][row] = matrix[row][column];
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // Interpret the uploaded bytes exactly as WGSL does: outer array = columns.
    fn shader_transform(matrix: [[f32; 4]; 4], p: [f32; 4]) -> [f32; 4] {
        std::array::from_fn(|r| (0..4).map(|c| matrix[c][r] * p[c]).sum())
    }

    #[test]
    fn translated_camera_centers_target_and_has_unit_basis() {
        let camera = Camera {
            position: Vec3::new(0.0, 1.6, 4.0),
            yaw: std::f32::consts::PI,
            pitch: (1.1_f32 / 4.0).atan(),
            aspect: 16.0 / 9.0,
            fov_y: 70.0_f32.to_radians(),
            near: 0.05,
            far: 256.0,
        };
        let clip = shader_transform(camera.view_projection(), [0.0, 0.5, 0.0, 1.0]);
        assert!(
            clip[0].abs() < 1e-5 && clip[1].abs() < 1e-5,
            "target clip={clip:?}"
        );
        assert!(clip[3] > 0.0 && clip[2] > 0.0 && clip[2] < clip[3]);
        let (right, up, forward) = camera.basis();
        for axis in [right, up, forward] {
            assert!((dot(axis, axis) - 1.0).abs() < 1e-5);
        }
        assert!(dot(cross(right, up), forward) < -0.9999);
    }

    #[test]
    fn look_at_maps_eye_to_origin_and_target_to_negative_z() {
        let eye = Vec3::new(0.0, 1.6, 4.0);
        let matrix = transpose(look_at(eye, Vec3::new(0.0, 0.5, 0.0)));
        let origin = shader_transform(matrix, [eye.x, eye.y, eye.z, 1.0]);
        assert_eq!(origin, [0.0, 0.0, 0.0, 1.0]);
        let target = shader_transform(matrix, [0.0, 0.5, 0.0, 1.0]);
        assert!(target[0].abs() < 1e-6 && target[1].abs() < 1e-6);
        assert!((target[2] + (4.0_f32 * 4.0 + 1.1 * 1.1).sqrt()).abs() < 1e-6);
        let right = shader_transform(matrix, [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(right, [1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn projection_depth_fov_aspect_and_resize_are_wgpu_compatible() {
        let mut camera = diagnostic::Stage::Triangle.camera(16.0 / 9.0);
        camera.resize(800, 600);
        camera.resize(0, 600);
        camera.resize(800, 0);
        assert_eq!(camera.aspect, 800.0 / 600.0);
        assert!((camera.fov_y - 1.2217305).abs() < 1e-6);
        let p = transpose(projection(camera));
        let near = shader_transform(p, [0.0, 0.0, -camera.near, 1.0]);
        let far = shader_transform(p, [0.0, 0.0, -camera.far, 1.0]);
        assert!((near[2] / near[3]).abs() < 1e-6);
        assert!((far[2] / far[3] - 1.0).abs() < 1e-6);
        let height = (camera.fov_y * 0.5).tan();
        let edge = shader_transform(p, [height * camera.aspect, height, -1.0, 1.0]);
        assert!((edge[0] - edge[3]).abs() < 1e-6);
        assert!((edge[1] - edge[3]).abs() < 1e-6);
    }

    #[test]
    fn extreme_pitch_stays_finite_and_roll_free() {
        for pitch in [-1.5, -1.0, 0.0, 1.0, 1.5] {
            let camera = Camera {
                position: Vec3::new(2.0, 3.0, -4.0),
                yaw: 1.2,
                pitch,
                aspect: 0.1,
                fov_y: 70.0_f32.to_radians(),
                near: 0.05,
                far: 256.0,
            };
            let (right, up, forward) = camera.basis();
            for axis in [right, up, forward] {
                assert!(axis.x.is_finite() && axis.y.is_finite() && axis.z.is_finite());
            }
            assert!(
                dot(right, up).abs() < 1e-5
                    && dot(right, forward).abs() < 1e-5
                    && dot(up, forward).abs() < 1e-5
            );
            assert!((dot(right, right) - 1.0).abs() < 1e-5);
            assert!(
                camera
                    .view_projection()
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
            );
        }
    }

    #[test]
    fn composed_matrix_matches_separate_view_then_projection() {
        let camera = diagnostic::Stage::Triangle.camera(16.0 / 9.0);
        let v = look_at(camera.position, Vec3::new(0.0, 0.5, 0.0));
        let point = [0.3, 0.8, -0.4, 1.0];
        let separate = shader_transform(
            transpose(projection(camera)),
            shader_transform(transpose(v), point),
        );
        let combined = shader_transform(diagnostic::Stage::Triangle.matrix(camera.aspect), point);
        for i in 0..4 {
            assert!((separate[i] - combined[i]).abs() < 1e-6);
        }
        // A world-horizontal line remains screen-horizontal: no roll.
        let left = shader_transform(
            diagnostic::Stage::Triangle.matrix(camera.aspect),
            [-10., 0., -30., 1.],
        );
        let right = shader_transform(
            diagnostic::Stage::Triangle.matrix(camera.aspect),
            [10., 0., -30., 1.],
        );
        assert!((left[1] / left[3] - right[1] / right[3]).abs() < 1e-6);
    }

    #[test]
    fn vertex_layout_matches_shader_attributes() {
        assert_eq!(std::mem::size_of::<Vertex>(), 36);
        assert_eq!(std::mem::offset_of!(Vertex, position), 0);
        assert_eq!(std::mem::offset_of!(Vertex, uv), 12);
        assert_eq!(std::mem::offset_of!(Vertex, shade), 20);
        assert_eq!(std::mem::offset_of!(Vertex, color), 24);
        assert_eq!(std::mem::size_of::<[[f32; 4]; 4]>(), 64);
    }

    #[test]
    fn quad_indices_uvs_and_both_triangle_windings() {
        let mut mesh = CpuMesh::default();
        let normals = [
            (Face::North, Vec3::new(0., 0., 1.)),
            (Face::South, Vec3::new(0., 0., -1.)),
            (Face::East, Vec3::new(1., 0., 0.)),
            (Face::West, Vec3::new(-1., 0., 0.)),
            (Face::Top, Vec3::new(0., 1., 0.)),
            (Face::Bottom, Vec3::new(0., -1., 0.)),
        ];
        for (face_number, (face, expected_normal)) in normals.into_iter().enumerate() {
            append_face(
                &mut mesh,
                BlockPos { x: 0, y: 0, z: 0 },
                face,
                TextureTile { x: 1, y: 0 },
                1.0,
            );
            let base = (face_number * 4) as u32;
            assert_eq!(
                &mesh.indices[face_number * 6..],
                &[base, base + 1, base + 2, base, base + 2, base + 3]
            );
            for (vertex, expected_uv) in mesh.vertices[face_number * 4..].iter().zip([
                [0., 1.],
                [1., 1.],
                [1., 0.],
                [0., 0.],
            ]) {
                assert_eq!(vertex.uv, tile_uv(TextureTile { x: 1, y: 0 }, expected_uv));
            }
            for triangle in mesh.indices[face_number * 6..].as_chunks::<3>().0 {
                let p: [[f32; 3]; 3] =
                    std::array::from_fn(|i| mesh.vertices[triangle[i] as usize].position);
                let ab = Vec3::new(p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]);
                let ac = Vec3::new(p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]);
                assert_eq!(cross(ab, ac), expected_normal);
            }
        }
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
    }

    #[test]
    fn atlas_uv_bounds_use_top_left_origin() {
        assert_eq!(
            tile_uv(TextureTile { x: 1, y: 0 }, [0., 0.]),
            [16.0 / 256.0, 0.]
        );
        assert_eq!(
            tile_uv(TextureTile { x: 1, y: 0 }, [1., 1.]),
            [32.0 / 256.0, 16.0 / 256.0]
        );
        assert_eq!(tile_uv(TextureTile { x: 15, y: 15 }, [1., 1.]), [1., 1.]);
    }

    #[test]
    fn signed_section_y_changes_only_world_y() {
        for section_y in [-1, 0, 1] {
            assert_eq!(
                section_origin(ChunkPos { x: 2, z: -3 }, section_y),
                BlockPos {
                    x: 32,
                    y: section_y * 16,
                    z: -48
                }
            );
            let stage = match section_y {
                -1 => diagnostic::Stage::SectionNegative,
                0 => diagnostic::Stage::SectionZero,
                _ => diagnostic::Stage::SectionPositive,
            };
            let mesh = stage.mesh();
            assert_eq!(mesh.indices.len(), 30 * 6); // 9 top + 9 bottom + 12 perimeter
            for vertex in mesh.vertices {
                assert!((0.0..=3.0).contains(&vertex.position[0]));
                assert!((0.0..=3.0).contains(&vertex.position[2]));
                assert!(
                    ((section_y * 16 + 1) as f32..=(section_y * 16 + 2) as f32)
                        .contains(&vertex.position[1])
                );
            }
        }
        let position = BlockPos { x: 0, y: -1, z: 0 };
        assert_eq!(position.y.div_euclid(CHUNK_SIZE), -1);
        assert_eq!(position.y.rem_euclid(CHUNK_SIZE), 15);
    }
    struct Textures;
    impl BlockTextureResolver for Textures {
        fn texture(&self, block: BlockId, _face: Face) -> Option<TextureTile> {
            (block.0 != 0).then_some(TextureTile { x: 1, y: 0 })
        }
        fn opaque(&self, block: BlockId) -> bool {
            block.0 != 0
        }
    }
    #[test]
    fn adjacent_opaque_blocks_share_no_internal_face() {
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 0, y: 0, z: 0 }, BlockId(1));
        world.set(BlockPos { x: 1, y: 0, z: 0 }, BlockId(1));
        let presentation = RenderWorld::from_world(&world);
        let mesh = build_chunk_mesh(
            &presentation,
            presentation.chunks().next().unwrap(),
            &Textures,
        );
        assert_eq!(mesh.indices.len(), 10 * 6);
    }
    #[test]
    fn empty_chunk_has_no_geometry() {
        let world = World::new(BlockId(0));
        let presentation = RenderWorld::from_world(&world);
        assert_eq!(presentation.chunk_count(), 0);
    }

    #[test]
    fn chunk_edge_neighbor_culls_boundary_face() {
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 15, y: 0, z: 0 }, BlockId(1));
        world.set(BlockPos { x: 16, y: 0, z: 0 }, BlockId(1));
        let presentation = RenderWorld::from_world(&world);
        let left = presentation
            .chunks()
            .find(|chunk| chunk.position.x == 0)
            .unwrap();
        let mesh = build_chunk_mesh(&presentation, left, &Textures);
        assert_eq!(mesh.indices.len(), 5 * 6);
    }

    #[test]
    fn camera_basis_is_right_handed_and_has_no_roll() {
        let camera = Camera {
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            aspect: 1.0,
            fov_y: 1.0,
            near: 0.1,
            far: 100.0,
        };
        let (right, up, forward) = camera.basis();
        assert_eq!(forward, Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(right, Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(up, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(dot(right, up), 0.0);
        assert_eq!(dot(right, forward), 0.0);
        assert_eq!(dot(up, forward), 0.0);
    }

    #[test]
    fn camera_projection_puts_forward_point_in_front_of_camera() {
        let camera = Camera {
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            aspect: 1.0,
            fov_y: 1.0,
            near: 0.1,
            far: 100.0,
        };
        let matrix = camera.view_projection();
        let point = [0.0, 0.0, 1.0, 1.0];
        let clip = [
            matrix[0][0] * point[0]
                + matrix[1][0] * point[1]
                + matrix[2][0] * point[2]
                + matrix[3][0],
            matrix[0][1] * point[0]
                + matrix[1][1] * point[1]
                + matrix[2][1] * point[2]
                + matrix[3][1],
            matrix[0][2] * point[0]
                + matrix[1][2] * point[1]
                + matrix[2][2] * point[2]
                + matrix[3][2],
            matrix[0][3] * point[0]
                + matrix[1][3] * point[1]
                + matrix[2][3] * point[2]
                + matrix[3][3],
        ];
        assert!(clip[3] > 0.0);
        assert!(clip[2] / clip[3] > 0.0 && clip[2] / clip[3] < 1.0);
        assert_eq!(clip[0], 0.0);
        assert_eq!(clip[1], 0.0);
    }

    #[test]
    fn cube_faces_have_outward_winding_and_uvs_stay_in_one_tile() {
        let expected = [
            (Face::North, Vec3::new(0.0, 0.0, 1.0)),
            (Face::South, Vec3::new(0.0, 0.0, -1.0)),
            (Face::East, Vec3::new(1.0, 0.0, 0.0)),
            (Face::West, Vec3::new(-1.0, 0.0, 0.0)),
            (Face::Top, Vec3::new(0.0, 1.0, 0.0)),
            (Face::Bottom, Vec3::new(0.0, -1.0, 0.0)),
        ];
        for (face, normal) in expected {
            let mut mesh = CpuMesh::default();
            append_face(
                &mut mesh,
                BlockPos { x: 0, y: 0, z: 0 },
                face,
                TextureTile { x: 4, y: 7 },
                1.0,
            );
            let a = mesh.vertices[0].position;
            let b = mesh.vertices[1].position;
            let c = mesh.vertices[2].position;
            let actual = cross(
                Vec3::new(b[0] - a[0], b[1] - a[1], b[2] - a[2]),
                Vec3::new(c[0] - a[0], c[1] - a[1], c[2] - a[2]),
            );
            assert_eq!(actual, normal);
            for vertex in &mesh.vertices {
                assert!(vertex.uv[0] >= 4.0 / 16.0 && vertex.uv[0] <= 5.0 / 16.0);
                assert!(vertex.uv[1] >= 7.0 / 16.0 && vertex.uv[1] <= 8.0 / 16.0);
            }
        }
    }
}

#[cfg(test)]
mod item_fidelity_tests {
    use super::*;

    #[test]
    fn beta_item_animation_changes_without_mutating_world_position() {
        let phase = 0.37;
        let b0 = beta_item_bob(0.0, phase);
        let b10 = beta_item_bob(10.0, phase);
        assert_ne!(b0, b10);
        assert_ne!(
            beta_item_rotation_degrees(0.0, phase),
            beta_item_rotation_degrees(20.0, phase)
        );
    }

    #[test]
    fn beta_item_cube_has_six_oriented_faces() {
        let faces = beta_item_cube_faces(0.5);
        assert_eq!(faces.len(), 6);
        assert!(faces.iter().all(|(_, uv, _)| {
            uv.iter()
                .all(|p| p[0] >= 0.0 && p[0] <= 1.0 && p[1] >= 0.0 && p[1] <= 1.0)
        }));
        assert!(faces[1].2 > faces[0].2); // top brighter than bottom
        for (positions, uv, _) in faces {
            for (axis, _) in uv[0].iter().enumerate() {
                assert_eq!(uv[0][axis] + uv[2][axis], uv[1][axis] + uv[3][axis]);
            }
            assert!(positions.iter().flatten().all(|v| v.abs() == 0.5));
        }
    }
}

#[cfg(test)]
mod m2_tests {
    use super::*;
    struct Materials;
    impl BlockTextureResolver for Materials {
        fn texture(&self, _: BlockId, _: Face) -> Option<TextureTile> {
            Some(TextureTile { x: 0, y: 0 })
        }
        fn opaque(&self, b: BlockId) -> bool {
            b.0 == 1
        }
        fn visible(&self, b: BlockId) -> bool {
            b.0 != 0
        }
    }
    fn mesh(w: &World) -> CpuMesh {
        let r = RenderWorld::from_world(w);
        build_chunk_mesh(&r, r.chunks().next().unwrap(), &Materials)
    }
    #[test]
    fn nonopaque_faces_preserve_opaque_neighbor_geometry() {
        let mut w = World::new(BlockId(0));
        w.set(BlockPos { x: 2, y: 2, z: 2 }, BlockId(1));
        w.set(BlockPos { x: 3, y: 2, z: 2 }, BlockId(2));
        assert_eq!(mesh(&w).indices.len(), 11 * 6); // opaque face behind glass retained
        w.set(BlockPos { x: 2, y: 2, z: 2 }, BlockId(2));
        assert_eq!(mesh(&w).indices.len(), 10 * 6); // same transparent material shared face culled
        w.set(BlockPos { x: 2, y: 2, z: 2 }, BlockId(3));
        assert_eq!(mesh(&w).indices.len(), 12 * 6); // different nonopaque materials preserve both
    }
    #[test]
    fn stored_light_changes_vertices_after_dirty_extraction() {
        let mut w = World::new(BlockId(0));
        let p = BlockPos { x: 15, y: 15, z: 3 };
        w.set(p, BlockId(1));
        let mut r = RenderWorld::from_world(&w);
        let key = (ChunkPos { x: 0, z: 0 }, 0);
        let dark = build_chunk_mesh(&r, r.chunks().next().unwrap(), &Materials);
        w.set_light(
            BlockPos { y: 16, ..p },
            rustcraft_engine_core::VoxelLight::new(15, 0),
        );
        r.sync_sections(&w, [key]);
        let lit = build_chunk_mesh(&r, r.chunks().next().unwrap(), &Materials);
        assert!(
            lit.vertices
                .iter()
                .zip(&dark.vertices)
                .any(|(l, d)| l.shade > d.shade)
        );
        assert_eq!(dark.indices, lit.indices);
    }
}
