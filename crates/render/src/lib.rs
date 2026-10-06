//! Presentation extraction, CPU voxel meshing and the wgpu presentation backend.
//! Simulation never depends on this crate.

use rustcraft_engine_core::{BlockId, BlockPos, BlockState, CHUNK_SIZE, ChunkPos, Vec3, World};
use std::{
    collections::{BTreeMap, HashMap},
    io::BufReader,
    path::Path,
    sync::Arc,
};

const SNAPSHOT_EDGE: usize = CHUNK_SIZE as usize + 2;
const SNAPSHOT_VOLUME: usize = SNAPSHOT_EDGE * SNAPSHOT_EDGE * SNAPSHOT_EDGE;

fn snapshot_index(x: i32, y: i32, z: i32) -> usize {
    debug_assert!((-1..=CHUNK_SIZE).contains(&x));
    debug_assert!((-1..=CHUNK_SIZE).contains(&y));
    debug_assert!((-1..=CHUNK_SIZE).contains(&z));
    let x = (x + 1) as usize;
    let y = (y + 1) as usize;
    let z = (z + 1) as usize;
    (y * SNAPSHOT_EDGE + z) * SNAPSHOT_EDGE + x
}

pub mod diagnostic;
pub mod geometry;
pub mod hud;
pub mod inspection;
pub mod meshing;
pub mod offscreen;
pub mod text;
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

/// Profile-local handle for a resolved physical texture page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextureHandle(pub u32);

/// A normalized rectangle in a resolved texture, independent of atlas cell size/layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtlasRegion {
    pub texture: TextureHandle,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
}

impl AtlasRegion {
    pub const fn full(texture: TextureHandle) -> Self {
        Self {
            texture,
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
        }
    }

    pub fn from_pixels(
        texture: TextureHandle,
        texture_size: [u32; 2],
        rectangle: [u32; 4],
    ) -> Option<Self> {
        let [width, height] = texture_size;
        let [x, y, region_width, region_height] = rectangle;
        (width > 0
            && height > 0
            && region_width > 0
            && region_height > 0
            && x.checked_add(region_width)? <= width
            && y.checked_add(region_height)? <= height)
            .then_some(Self {
                texture,
                uv_min: [x as f32 / width as f32, y as f32 / height as f32],
                uv_max: [
                    (x + region_width) as f32 / width as f32,
                    (y + region_height) as f32 / height as f32,
                ],
            })
    }

    pub fn grid_cell(
        texture: TextureHandle,
        columns: u32,
        rows: u32,
        x: u32,
        y: u32,
    ) -> Option<Self> {
        Self::from_pixels(texture, [columns, rows], [x, y, 1, 1])
    }
}

pub trait BlockTextureResolver {
    /// Triangle positions are in the neighbor cell's local coordinates.
    fn covers_triangle(&self, state: BlockState, face: Face, _positions: [[f32; 3]; 3]) -> bool {
        self.covers_face(state, face)
    }

    fn valid_state(&self, _state: BlockState) -> bool {
        true
    }

    fn selection_edges(&self, _state: BlockState) -> Option<&std::sync::Arc<[[[f32; 3]; 2]]>> {
        None
    }

    /// Shared compiled triangles; None selects the specialized canonical cube path.
    fn model_triangles(
        &self,
        _state: BlockState,
    ) -> Option<&std::sync::Arc<[rustcraft_engine_core::shape::Triangle]>> {
        None
    }
    fn covers_face(&self, state: BlockState, _face: Face) -> bool {
        self.opaque(state.block)
    }

    /// Independent hooks: whole-model orientation and state-dependent material selection.
    fn model_rotation(
        &self,
        _state: BlockState,
    ) -> rustcraft_engine_core::orientation::ModelRotation {
        rustcraft_engine_core::orientation::ModelRotation::IDENTITY
    }
    fn state_texture(&self, state: BlockState, face: Face) -> Option<AtlasRegion> {
        self.texture(state.block, face)
    }

    fn texture(&self, block: BlockId, face: Face) -> Option<AtlasRegion>;
    fn opaque(&self, block: BlockId) -> bool;
    /// Materials submitted with alpha blending after opaque/cutout geometry.
    fn translucent(&self, _block: BlockId) -> bool {
        false
    }
    /// Optional top height for static liquid geometry in block-local units.
    fn liquid_surface_height(&self, _state: BlockState) -> Option<f32> {
        None
    }
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
    voxels: Vec<BlockState>,
    lights: Vec<rustcraft_engine_core::VoxelLight>,
}

impl RenderChunk {
    #[must_use]
    pub fn state(&self, local: (i32, i32, i32)) -> BlockState {
        self.voxels[snapshot_index(local.0, local.1, local.2)]
    }

    #[must_use]
    pub fn light(&self, local: (i32, i32, i32)) -> rustcraft_engine_core::VoxelLight {
        self.lights[snapshot_index(local.0, local.1, local.2)]
    }

    #[must_use]
    pub fn snapshot_bytes(&self) -> usize {
        self.voxels.len() * std::mem::size_of::<BlockState>()
            + self.lights.len() * std::mem::size_of::<rustcraft_engine_core::VoxelLight>()
    }
}

#[derive(Debug)]
pub struct RenderWorld {
    chunks: HashMap<(ChunkPos, i32), RenderChunk>,
    default_state: BlockState,
}

impl Default for RenderWorld {
    fn default() -> Self {
        Self {
            chunks: HashMap::new(),
            // Standalone diagnostics explicitly use zero as their local empty sentinel.
            default_state: BlockState::new(BlockId(0)),
        }
    }
}

impl RenderWorld {
    #[must_use]
    pub fn section(&self, position: ChunkPos, section_y: i32) -> Option<&RenderChunk> {
        self.chunks.get(&(position, section_y))
    }

    pub fn sync_sections(
        &mut self,
        world: &World,
        dirty: impl IntoIterator<Item = rustcraft_engine_core::SectionPos>,
    ) {
        self.default_state = world.default_state();
        for (pos, y) in dirty {
            self.copy_chunk(world, pos, y);
        }
    }
    #[must_use]
    pub fn from_world(world: &World) -> Self {
        let mut presentation = Self {
            default_state: world.default_state(),
            ..Self::default()
        };
        for (position, section_y) in world.section_positions() {
            presentation.copy_chunk(world, position, section_y);
        }
        presentation
    }
    pub fn sync_dirty(&mut self, world: &World, dirty: impl IntoIterator<Item = ChunkPos>) {
        self.default_state = world.default_state();
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
        if let Some(section) = world.section(position, section_y) {
            let origin = section_origin(position, section_y);
            let mut voxels = vec![world.default_state(); SNAPSHOT_VOLUME];
            let mut lights = vec![rustcraft_engine_core::VoxelLight::default(); SNAPSHOT_VOLUME];
            let center_lights = world.section_lights(position, section_y);
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        let source_index = y * 256 + z * 16 + x;
                        let destination_index = snapshot_index(x as i32, y as i32, z as i32);
                        voxels[destination_index] = section.states()[source_index];
                        if let Some(section_lights) = center_lights {
                            lights[destination_index] = section_lights[source_index];
                        }
                    }
                }
            }
            for y in -1..=16 {
                for z in -1..=16 {
                    for x in -1..=16 {
                        if (0..16).contains(&x) && (0..16).contains(&y) && (0..16).contains(&z) {
                            continue;
                        }
                        let p = BlockPos {
                            x: origin.x + x,
                            y: origin.y + y,
                            z: origin.z + z,
                        };
                        let destination_index = snapshot_index(x, y, z);
                        voxels[destination_index] = world.state(p);
                        lights[destination_index] = world.light(p);
                    }
                }
            }
            self.chunks.insert(
                (position, section_y),
                RenderChunk {
                    position,
                    section_y,
                    voxels,
                    lights,
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
    pub fn snapshot_bytes(&self) -> usize {
        self.chunks.values().map(RenderChunk::snapshot_bytes).sum()
    }
    #[must_use]
    pub fn block(&self, position: BlockPos) -> BlockId {
        let cx = position.x.div_euclid(CHUNK_SIZE);
        let cz = position.z.div_euclid(CHUNK_SIZE);
        let key = ChunkPos { x: cx, z: cz };
        let section_y = position.y.div_euclid(16);
        self.chunks
            .get(&(key, section_y))
            .map_or(self.default_state.block, |chunk| {
                chunk
                    .state((
                        position.x.rem_euclid(16),
                        position.y.rem_euclid(16),
                        position.z.rem_euclid(16),
                    ))
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FogPresentation {
    pub color: [f32; 3],
    pub start: f32,
    pub end: f32,
}
impl FogPresentation {
    #[must_use]
    pub fn linear_amount(self, distance: f32) -> f32 {
        ((distance - self.start) / (self.end - self.start).max(0.001)).clamp(0.0, 1.0)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    matrix: [[f32; 4]; 4],
    position: [f32; 4],
    fog_color: [f32; 4],
    fog_range: [f32; 4],
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

/// Conservative camera-frustum test used only for draw submission. Resident meshes are retained.
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    camera: Camera,
    right: Vec3,
    up: Vec3,
    forward: Vec3,
    tan_y: f32,
    tan_x: f32,
}

impl Frustum {
    #[must_use]
    pub fn from_camera(camera: Camera) -> Self {
        let (right, up, forward) = camera.basis();
        let tan_y = (camera.fov_y * 0.5).tan();
        Self {
            camera,
            right,
            up,
            forward,
            tan_y,
            tan_x: tan_y * camera.aspect,
        }
    }

    #[must_use]
    pub fn intersects_aabb(self, bounds: rustcraft_engine_core::Aabb) -> bool {
        let mut outside = [true; 6];
        for x in [bounds.min.x, bounds.max.x] {
            for y in [bounds.min.y, bounds.max.y] {
                for z in [bounds.min.z, bounds.max.z] {
                    let relative = Vec3::new(
                        x - self.camera.position.x,
                        y - self.camera.position.y,
                        z - self.camera.position.z,
                    );
                    let view_x = dot(relative, self.right);
                    let view_y = dot(relative, self.up);
                    let view_z = dot(relative, self.forward);
                    let tests = [
                        view_z - self.camera.near,
                        self.camera.far - view_z,
                        view_x + view_z * self.tan_x,
                        -view_x + view_z * self.tan_x,
                        view_y + view_z * self.tan_y,
                        -view_y + view_z * self.tan_y,
                    ];
                    for (still_outside, distance) in outside.iter_mut().zip(tests) {
                        *still_outside &= distance < 0.0;
                    }
                }
            }
        }
        !outside.into_iter().any(|value| value)
    }

    #[must_use]
    pub fn intersects_section(self, position: ChunkPos, section_y: i32) -> bool {
        let origin = section_origin(position, section_y);
        self.intersects_aabb(rustcraft_engine_core::Aabb::new(
            Vec3::new(origin.x as f32, origin.y as f32, origin.z as f32),
            Vec3::new(
                (origin.x + CHUNK_SIZE) as f32,
                (origin.y + CHUNK_SIZE) as f32,
                (origin.z + CHUNK_SIZE) as f32,
            ),
        ))
    }
}

pub fn visible_section_positions(
    sections: impl IntoIterator<Item = (ChunkPos, i32)>,
    frustum: Frustum,
) -> Vec<(ChunkPos, i32)> {
    let mut visible = sections
        .into_iter()
        .filter(|(position, section_y)| frustum.intersects_section(*position, *section_y))
        .collect::<Vec<_>>();
    visible.sort_by_key(|(position, section_y)| (position.x, position.z, *section_y));
    visible
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub shade: f32,
    pub color: [f32; 3],
}

/// CPU geometry that must be submitted with one compiled atlas page bound.
#[derive(Debug, Clone)]
pub struct PageVertices {
    pub texture: TextureHandle,
    pub vertices: Vec<Vertex>,
}

pub(crate) fn page_vertices_mut(
    pages: &mut Vec<PageVertices>,
    texture: TextureHandle,
) -> &mut Vec<Vertex> {
    match pages.binary_search_by_key(&texture, |page| page.texture) {
        Ok(index) => &mut pages[index].vertices,
        Err(index) => {
            pages.insert(
                index,
                PageVertices {
                    texture,
                    vertices: Vec::new(),
                },
            );
            &mut pages[index].vertices
        }
    }
}

#[derive(Debug, Clone)]
pub struct ItemSprite {
    pub model: Option<inspection::BlockModel>,
    pub position: Vec3,
    pub top: AtlasRegion,
    pub side: AtlasRegion,
    pub bottom: AtlasRegion,
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
pub fn append_dropped_item_pages(pages: &mut Vec<PageVertices>, sprites: &[ItemSprite]) {
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
            if let Some(model) = sprite.model.as_ref().filter(|m| m.triangles.is_some()) {
                let mut model_pages = Vec::new();
                model.world_page_vertices(&mut model_pages);
                for page in model_pages {
                    let out = page_vertices_mut(pages, page.texture);
                    for mut vertex in page.vertices {
                        vertex.position = transform(vertex.position.map(|v| (v - 0.5) * scale));
                        out.push(vertex);
                    }
                }
                continue;
            }
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
                    sprite.model.as_ref().map_or(p, |m| m.rotation.transform(p))
                });
                let uvs = definition.uv_corners;
                let tile = sprite.model.as_ref().map_or(
                    match direction {
                        Face::Top => sprite.top,
                        Face::Bottom => sprite.bottom,
                        _ => sprite.side,
                    },
                    |m| m.texture(direction),
                );
                let tint = sprite
                    .model
                    .as_ref()
                    .map_or(sprite.tint, |m| m.tints[direction as usize]);
                let uv = uvs.map(|uv| region_uv(tile, uv));
                let vertices = page_vertices_mut(pages, tile.texture);
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

/// Compatibility helper for single-page offscreen diagnostics.
///
/// Production rendering uses [`append_dropped_item_pages`] so the texture page is never lost.
pub fn append_dropped_items(vertices: &mut Vec<Vertex>, sprites: &[ItemSprite]) {
    let mut pages = Vec::new();
    append_dropped_item_pages(&mut pages, sprites);
    assert!(
        pages.len() <= 1,
        "single-page dropped-item helper received a multi-page model"
    );
    if let Some(page) = pages.pop() {
        vertices.extend(page.vertices);
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

#[derive(Debug, Clone)]
pub struct PageMesh {
    pub texture: TextureHandle,
    pub translucent: bool,
    pub mesh: CpuMesh,
}

impl CpuMesh {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

pub fn build_chunk_mesh(
    _world: &RenderWorld,
    chunk: &RenderChunk,
    resolver: &impl BlockTextureResolver,
) -> CpuMesh {
    let pages = build_section_mesh_pages(chunk, resolver);
    let mut merged = CpuMesh::default();
    for page in pages {
        let base = merged.vertices.len() as u32;
        merged.vertices.extend(page.mesh.vertices);
        merged
            .indices
            .extend(page.mesh.indices.into_iter().map(|index| base + index));
    }
    merged
}

pub fn build_chunk_mesh_pages(
    _world: &RenderWorld,
    chunk: &RenderChunk,
    resolver: &impl BlockTextureResolver,
) -> Vec<PageMesh> {
    build_section_mesh_pages(chunk, resolver)
}

/// Builds one immutable section snapshot without consulting shared world state.
pub fn build_section_mesh_pages(
    chunk: &RenderChunk,
    resolver: &impl BlockTextureResolver,
) -> Vec<PageMesh> {
    let mut pages = BTreeMap::<(bool, u32), CpuMesh>::new();
    let origin = section_origin(chunk.position, chunk.section_y);
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                let state = chunk.state((x, y, z));
                let block = state.block;
                let rotation = resolver.model_rotation(state);
                let translucent = resolver.translucent(block);
                let liquid_height = resolver.liquid_surface_height(state);
                if !resolver.visible(block) || !resolver.valid_state(state) {
                    continue;
                }
                let position = BlockPos {
                    x: origin.x + x,
                    y: origin.y + y,
                    z: origin.z + z,
                };
                if let Some(triangles) = resolver.model_triangles(state) {
                    for triangle in triangles.iter() {
                        let transformed = triangle.positions.map(|p| rotation.point(p));
                        let normal = rotation.transform(triangle.normal());
                        let axis = (0..3)
                            .max_by(|a, b| {
                                normal[*a]
                                    .abs()
                                    .total_cmp(&normal[*b].abs())
                                    .then_with(|| b.cmp(a))
                            })
                            .unwrap();
                        let mut n = [0; 3];
                        n[axis] = if normal[axis] > 0. { 1 } else { -1 };
                        let face = geometry::FACES
                            .iter()
                            .find(|f| f.normal == n.map(|v| v as f32))
                            .unwrap()
                            .direction;
                        let neighbor_local = (x + n[0], y + n[1], z + n[2]);
                        let neighbor = chunk.state(neighbor_local);
                        let boundary = transformed
                            .iter()
                            .all(|p| p[axis] == if n[axis] > 0 { 1. } else { 0. });
                        let opposite = geometry::FACES
                            .iter()
                            .find(|f| f.normal == n.map(|v| -v as f32))
                            .unwrap()
                            .direction;
                        let neighbor_positions =
                            transformed.map(|p| std::array::from_fn(|a| p[a] - n[a] as f32));
                        if boundary
                            && resolver.covers_triangle(neighbor, opposite, neighbor_positions)
                        {
                            continue;
                        }
                        let model_face = geometry::FACES[triangle.face as usize].direction;
                        if let Some(tile) = resolver.state_texture(state, model_face) {
                            let mesh = pages.entry((translucent, tile.texture.0)).or_default();
                            let light = chunk.light(neighbor_local);
                            let strength = light
                                .sky()
                                .max(light.block())
                                .max(chunk.light((x, y, z)).block());
                            let shade = faces(position)[face as usize].2
                                * (0.05 + 0.95 * f32::from(strength) / 15.);
                            let base = mesh.vertices.len() as u32;
                            for (p, uv) in transformed.into_iter().zip(triangle.uv) {
                                mesh.vertices.push(Vertex {
                                    position: [
                                        p[0] + position.x as f32,
                                        p[1] + position.y as f32,
                                        p[2] + position.z as f32,
                                    ],
                                    uv: region_uv(tile, uv),
                                    shade,
                                    color: resolver.tint(block, model_face),
                                });
                            }
                            mesh.indices.extend([base, base + 1, base + 2]);
                        }
                    }
                    continue;
                }
                for model_face in geometry::FACES {
                    let normal = rotation.transform(model_face.normal);
                    let face = geometry::FACES
                        .iter()
                        .find(|f| f.normal == normal)
                        .expect("orthogonal model rotation")
                        .direction;
                    let (_, _neighbor, shade) = faces(position)[face as usize];
                    let neighbor_local = (
                        x + normal[0] as i32,
                        y + normal[1] as i32,
                        z + normal[2] as i32,
                    );
                    let neighbor_state = chunk.state(neighbor_local);
                    let neighbor_block = neighbor_state.block;
                    let opposite = geometry::FACES[(face as usize) ^ 1].direction;
                    if resolver.covers_face(neighbor_state, opposite)
                        || ((!resolver.opaque(block) || translucent)
                            && neighbor_block == block
                            && resolver.model_triangles(neighbor_state).is_none())
                    {
                        continue;
                    }
                    if let Some(tile) = resolver.state_texture(state, model_face.direction) {
                        let mesh = pages.entry((translucent, tile.texture.0)).or_default();
                        let start = mesh.vertices.len();
                        let neighbor_light = chunk.light(neighbor_local);
                        let own = chunk.light((x, y, z)).block();
                        let light = 0.05
                            + 0.95
                                * f32::from(
                                    neighbor_light.sky().max(neighbor_light.block()).max(own),
                                )
                                / 15.0;
                        append_face(
                            mesh,
                            position,
                            model_face.direction,
                            tile,
                            shade * light,
                            liquid_height,
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
    pages
        .into_iter()
        .map(|((translucent, page), mesh)| PageMesh {
            texture: TextureHandle(page),
            translucent,
            mesh,
        })
        .collect()
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
fn region_uv(region: AtlasRegion, uv: [f32; 2]) -> [f32; 2] {
    [
        region.uv_min[0] + uv[0] * (region.uv_max[0] - region.uv_min[0]),
        region.uv_min[1] + uv[1] * (region.uv_max[1] - region.uv_min[1]),
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

fn append_face(
    mesh: &mut CpuMesh,
    position: BlockPos,
    face: Face,
    tile: AtlasRegion,
    shade: f32,
    liquid_surface_height: Option<f32>,
) {
    let x = position.x as f32;
    let y = position.y as f32;
    let z = position.z as f32;
    let n = mesh.vertices.len() as u32;
    let definition = geometry::definition(face);
    let corners = definition.positions.map(|mut p| {
        if p[1] >= 1.0
            && let Some(height) = liquid_surface_height
        {
            p[1] = height.clamp(0.5, 1.0);
        }
        [x + p[0], y + p[1], z + p[2]]
    });
    let uv = definition.uv_corners;
    for i in 0..4 {
        mesh.vertices.push(Vertex {
            position: corners[i],
            uv: region_uv(tile, uv[i]),
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

#[derive(Debug, Clone)]
pub struct RgbaTexture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub sampler: TextureSampling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureSampling {
    Nearest,
    Linear,
}

/// Compiled physical pages plus game-selected semantic presentation roles.
#[derive(Debug, Clone)]
pub struct RendererResources {
    pub fonts: rustcraft_content::fonts::FontResources,
    pub atlas_pages: Vec<RgbaTexture>,
    pub container_background: AtlasRegion,
    pub hud: [AtlasRegion; 2],
    pub player_preview: [AtlasRegion; 6],
    pub occupancy: f32,
    pub cache_hit: bool,
}
#[derive(Debug)]
pub struct GpuPageMesh {
    texture: TextureHandle,
    translucent: bool,
    pub vertex: wgpu::Buffer,
    pub index: wgpu::Buffer,
    vertex_capacity_bytes: u64,
    index_capacity_bytes: u64,
    pub index_count: u32,
    vertex_count: usize,
}

#[derive(Debug)]
pub struct GpuChunk {
    pages: Vec<GpuPageMesh>,
}

fn buffer_capacity(logical_bytes: usize) -> u64 {
    logical_bytes
        .max(256)
        .checked_next_power_of_two()
        .expect("mesh buffer capacity overflow") as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BufferCapacityAction {
    Reuse,
    Reallocate(u64),
}

fn buffer_capacity_action(existing_capacity: u64, logical_bytes: usize) -> BufferCapacityAction {
    let logical = logical_bytes as u64;
    let should_shrink = logical.saturating_mul(4) < existing_capacity;
    if logical <= existing_capacity && !should_shrink {
        BufferCapacityAction::Reuse
    } else {
        BufferCapacityAction::Reallocate(buffer_capacity(logical_bytes))
    }
}

fn create_mesh_buffer(
    device: &wgpu::Device,
    capacity: u64,
    usage: wgpu::BufferUsages,
    label: &str,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: capacity,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn reuse_or_allocate_buffer(
    device: &wgpu::Device,
    existing: wgpu::Buffer,
    existing_capacity: u64,
    logical_bytes: usize,
    usage: wgpu::BufferUsages,
    label: &str,
) -> (wgpu::Buffer, u64, bool) {
    match buffer_capacity_action(existing_capacity, logical_bytes) {
        BufferCapacityAction::Reuse => (existing, existing_capacity, false),
        BufferCapacityAction::Reallocate(capacity) => (
            create_mesh_buffer(device, capacity, usage, label),
            capacity,
            true,
        ),
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderSubmissionStats {
    pub resident_sections: usize,
    pub meshed_sections: usize,
    pub visible_sections: usize,
    pub culled_sections: usize,
    pub drawn_section_page_batches: usize,
    pub texture_page_bind_switches: usize,
    pub pipeline_switches: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RendererColumnState {
    pub resident_section_count: usize,
    pub drawable_section_count: usize,
    pub submitted_section_count: usize,
    pub frustum_culled_section_count: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MeshUploadStats {
    pub logical_bytes: usize,
    pub allocations: u64,
    pub reallocations: u64,
    pub reuses: u64,
    pub submit_ms: f64,
}

pub struct CapturedFrame {
    pub path: std::path::PathBuf,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
struct PendingCapture {
    buffer: wgpu::Buffer,
    rx: std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
    config: wgpu::SurfaceConfiguration,
    path: std::path::PathBuf,
    started: std::time::Instant,
}
#[derive(Default, Clone, Copy)]
pub struct ApplicationFrameTiming {
    pub acquire_ms: f64,
    pub prepare_ms: f64,
    pub total_ms: f64,
    pub submit_at: Option<std::time::Instant>,
    pub present_at: Option<std::time::Instant>,
}
pub struct Renderer {
    pub frame_timing_enabled: bool,
    pub frame_timing: ApplicationFrameTiming,
    pub supported_present_modes: Vec<wgpu::PresentMode>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    translucent_pipeline: wgpu::RenderPipeline,
    hud_pipeline: wgpu::RenderPipeline,
    gui_item_pipeline: wgpu::RenderPipeline,
    gui_pipeline: wgpu::RenderPipeline,
    crack_pipeline: wgpu::RenderPipeline,
    selection_pipeline: wgpu::RenderPipeline,
    hud_geometry: hud::HudGeometry,
    text_surface: TextSurface,
    text_scale: f32,
    hud_buffer: wgpu::Buffer,
    selection_buffer: wgpu::Buffer,
    debug_boxes: Vec<(rustcraft_engine_core::Aabb, [f32; 3])>,
    page_binds: Vec<wgpu::BindGroup>,
    container_background: AtlasRegion,
    hud_regions: [AtlasRegion; 2],
    player_regions: [AtlasRegion; 6],
    hotbar_ranges: Vec<(TextureHandle, std::ops::Range<u32>)>,
    player_ranges: Vec<(TextureHandle, std::ops::Range<u32>)>,
    item_buffer: wgpu::Buffer,
    item_vertices: usize,
    item_page_ranges: Vec<(TextureHandle, std::ops::Range<u32>)>,
    crack_buffer: wgpu::Buffer,
    crack_vertices: usize,
    crack_texture: Option<TextureHandle>,
    pub adapter_info: wgpu::AdapterInfo,
    timing: Option<timing::GpuTiming>,
    pub telemetry_enabled: bool,
    camera_buffer: wgpu::Buffer,
    fog: Option<FogPresentation>,
    _textures: Vec<Texture>,
    depth_view: wgpu::TextureView,
    chunks: HashMap<(ChunkPos, i32), GpuChunk>,
    pub mesh_rebuilds: u64,
    pub vertices: usize,
    pub indices: usize,
    mesh_buffer_allocations: u64,
    mesh_buffer_reallocations: u64,
    mesh_buffer_reuses: u64,
    resident_section_count: usize,
    diagnostic: Option<diagnostic::Stage>,
    projection_logged: bool,
    async_capture: bool,
    pending_capture: Option<PendingCapture>,
    texture_bytes: usize,
    atlas_occupancy: f32,
    resource_cache_hit: bool,
    atlas_upload_submit_ms: f64,
    submission: RenderSubmissionStats,
}

impl Renderer {
    pub fn set_camera_fog(&mut self, fog: Option<FogPresentation>) {
        self.fog = fog;
    }
    pub async fn new(
        window: Arc<winit::window::Window>,
        resources: &RendererResources,
    ) -> Result<Self, String> {
        Self::new_with_stage(window, resources, None).await
    }

    pub async fn new_diagnostic(
        window: Arc<winit::window::Window>,
        resources: &RendererResources,
        stage: diagnostic::Stage,
    ) -> Result<Self, String> {
        Self::new_with_stage(window, resources, Some(stage)).await
    }

    async fn new_with_stage(
        window: Arc<winit::window::Window>,
        resources: &RendererResources,
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
        let upload_started = std::time::Instant::now();
        let max_texture_dimension = adapter.limits().max_texture_dimension_2d;
        let textures = if diagnostic.is_some_and(|s| !s.uses_atlas()) {
            vec![diagnostic::checker(&device, &queue)]
        } else {
            if resources.atlas_pages.is_empty() {
                return Err("compiled renderer resources contain no atlas pages".into());
            }
            resources
                .atlas_pages
                .iter()
                .enumerate()
                .map(|(index, page)| {
                    if page.width == 0
                        || page.height == 0
                        || page.width > max_texture_dimension
                        || page.height > max_texture_dimension
                        || page.rgba.len() != page.width as usize * page.height as usize * 4
                    {
                        return Err(format!(
                            "invalid atlas page {index}: {}x{} with {} bytes",
                            page.width,
                            page.height,
                            page.rgba.len()
                        ));
                    }
                    Ok(upload_texture_with_sampling(
                        &device,
                        &queue,
                        page.width,
                        page.height,
                        &page.rgba,
                        page.sampler,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?
        };
        let atlas_upload_submit_ms = upload_started.elapsed().as_secs_f64() * 1000.0;
        let texture_bytes = resources
            .atlas_pages
            .iter()
            .map(|page| page.rgba.len())
            .sum();
        eprintln!(
            "atlas upload submission: pages={} bytes={} device_max={} submit_ms={atlas_upload_submit_ms:.3}",
            textures.len(),
            texture_bytes,
            max_texture_dimension
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxel-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let camera_buffer = wgpu::util::DeviceExt::create_buffer_init(
            &device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("camera"),
                contents: bytemuck::bytes_of(&CameraUniform {
                    matrix: [[0.0; 4]; 4],
                    position: [0.0; 4],
                    fog_color: [0.0; 4],
                    fog_range: [0.0; 4],
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        );
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera-texture-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
        let page_binds = textures
            .iter()
            .enumerate()
            .map(|(index, texture)| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("atlas-page-{index}-bind")),
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
                })
            })
            .collect::<Vec<_>>();
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
                entry_point: Some(if diagnostic.is_some_and(|s| !s.textured()) { "fs_color" } else if diagnostic.is_some_and(|s| s != diagnostic::Stage::NormalLit) { "fs_unlit" } else { "fs_world" }),
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
        let translucent_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxel-translucent-pipeline"),
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
                entry_point: Some("fs_translucent"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
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
        let selection_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("depth-tested-block-selection-outline"),
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
                entry_point: Some("fs_selection"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: selection_primitive_state(),
            depth_stencil: Some(selection_depth_stencil_state()),
            multisample: Default::default(),
            multiview: None,
            cache: None,
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
        let selection_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("world-space-selection-outline"),
            size: 65536,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let adapter_info = adapter.get_info();
        let timing = timing::GpuTiming::new(&device);
        let text_surface =
            TextSurface::new(&device, &layout, &camera_buffer, resources.fonts.clone())?;
        Ok(Self {
            text_surface,
            text_scale: 1.,
            surface,
            device,
            queue,
            config,
            pipeline,
            translucent_pipeline,
            hud_pipeline,
            gui_item_pipeline,
            gui_pipeline,
            crack_pipeline,
            selection_pipeline,
            hud_geometry: hud::HudGeometry::default(),
            hud_buffer,
            selection_buffer,
            debug_boxes: Vec::new(),
            page_binds,
            container_background: resources.container_background,
            hud_regions: resources.hud,
            player_regions: resources.player_preview,
            hotbar_ranges: Vec::new(),
            player_ranges: Vec::new(),
            item_buffer,
            item_vertices: 0,
            item_page_ranges: Vec::new(),
            crack_buffer,
            crack_vertices: 0,
            crack_texture: None,
            adapter_info,
            timing,
            telemetry_enabled: false,
            frame_timing_enabled: false,
            frame_timing: ApplicationFrameTiming::default(),
            supported_present_modes: caps.present_modes.clone(),
            camera_buffer,
            fog: None,
            _textures: textures,
            depth_view,
            chunks: HashMap::new(),
            mesh_rebuilds: 0,
            vertices: 0,
            indices: 0,
            mesh_buffer_allocations: 0,
            mesh_buffer_reallocations: 0,
            mesh_buffer_reuses: 0,
            resident_section_count: 0,
            diagnostic,
            projection_logged: false,
            async_capture: false,
            pending_capture: None,
            texture_bytes,
            atlas_occupancy: resources.occupancy,
            resource_cache_hit: resources.cache_hit,
            atlas_upload_submit_ms,
            submission: RenderSubmissionStats::default(),
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
    /// Transient bounded debug geometry, independent of world meshes and persistence.
    pub fn set_debug_boxes(&mut self, boxes: &[rustcraft_engine_core::Aabb]) {
        self.set_debug_boxes_colored(boxes, &[]);
    }
    pub fn set_debug_boxes_colored(
        &mut self,
        boxes: &[rustcraft_engine_core::Aabb],
        colors: &[[f32; 3]],
    ) {
        self.debug_boxes.clear();
        self.debug_boxes.extend(
            boxes
                .iter()
                .take(64)
                .enumerate()
                .map(|(i, b)| (*b, colors.get(i).copied().unwrap_or([0.1, 0.8, 0.4]))),
        );
    }
    pub fn set_text_scale(&mut self, scale: f32) {
        self.text_scale = scale;
        self.hud_geometry.set_text_scale(scale);
    }
    pub fn text_metrics(&self) -> &text::TextMetrics {
        &self.text_surface.system.metrics
    }
    pub fn set_hud(&mut self, snapshot: &hud::HudSnapshot, camera: Camera) {
        self.hud_geometry
            .build(snapshot, self.width(), self.height(), camera);
        let runs = self
            .hud_geometry
            .debug_runs
            .iter()
            .chain(&self.hud_geometry.text_runs)
            .cloned()
            .map(|mut r| {
                r.pixels *= self.text_scale;
                r
            })
            .collect::<Vec<_>>();
        self.text_surface.update(
            &self.device,
            &self.queue,
            &runs,
            self.width(),
            self.height(),
            snapshot.caret,
        );
        for (bounds, color) in &self.debug_boxes {
            let point = |i: usize| {
                [
                    if i & 1 == 0 {
                        bounds.min.x
                    } else {
                        bounds.max.x
                    },
                    if i & 2 == 0 {
                        bounds.min.y
                    } else {
                        bounds.max.y
                    },
                    if i & 4 == 0 {
                        bounds.min.z
                    } else {
                        bounds.max.z
                    },
                ]
            };
            for corner in 0..8 {
                for bit in [1, 2, 4] {
                    if corner & bit == 0 {
                        for position in [point(corner), point(corner | bit)] {
                            self.hud_geometry.selection_vertices.push(Vertex {
                                position,
                                uv: [0.; 2],
                                shade: 1.,
                                color: *color,
                            });
                        }
                    }
                }
            }
        }
        self.player_ranges = resolve_presentation_quads(
            &mut self.hud_geometry.player_vertices,
            &self.player_regions,
        );
        self.hud_geometry.build_gui_background(
            self.width(),
            self.height(),
            snapshot.inventory_open,
            snapshot.selected,
        );
        for vertex in &mut self.hud_geometry.gui_vertices {
            vertex.uv = region_uv(self.container_background, vertex.uv);
        }
        self.hotbar_ranges =
            resolve_presentation_quads(&mut self.hud_geometry.hotbar_vertices, &self.hud_regions);
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
        for page in &self.hud_geometry.item_pages {
            assert!(
                (page.texture.0 as usize) < self.page_binds.len(),
                "GUI item references missing atlas page {}",
                page.texture.0
            );
        }
        assert!(
            item_bytes.len() <= 7 * 1024 * 1024,
            "GUI item geometry capacity exceeded"
        );
        if !item_bytes.is_empty() {
            self.queue
                .write_buffer(&self.hud_buffer, 1024 * 1024, item_bytes);
        }
        let bytes = bytemuck::cast_slice(&self.hud_geometry.vertices);
        assert!(bytes.len() <= 256 * 1024, "HUD geometry capacity exceeded");
        self.queue.write_buffer(&self.hud_buffer, 0, bytes);
        let selection_bytes = bytemuck::cast_slice(&self.hud_geometry.selection_vertices);
        assert!(
            selection_bytes.len() <= 65536,
            "selection geometry capacity exceeded"
        );
        if !selection_bytes.is_empty() {
            self.queue
                .write_buffer(&self.selection_buffer, 0, selection_bytes);
        }
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
        let mut pages = Vec::new();
        append_dropped_item_pages(&mut pages, sprites);
        let mut vertices = Vec::with_capacity(sprites.len() * 36);
        self.item_page_ranges.clear();
        for page in pages {
            assert!(
                (page.texture.0 as usize) < self.page_binds.len(),
                "dropped item references missing atlas page {}",
                page.texture.0
            );
            let start = vertices.len() as u32;
            vertices.extend(page.vertices);
            self.item_page_ranges
                .push((page.texture, start..vertices.len() as u32));
        }
        self.item_vertices = vertices.len();
        let bytes = bytemuck::cast_slice(&vertices);
        assert!(
            bytes.len() <= 512 * 1024,
            "dropped item geometry capacity exceeded"
        );
        if !vertices.is_empty() {
            self.queue.write_buffer(&self.item_buffer, 0, bytes);
        }
    }
    pub fn item_render_count(&self) -> usize {
        self.item_vertices / 6
    }
    pub fn set_crack_overlay(&mut self, target: Option<BlockPos>, region: Option<AtlasRegion>) {
        self.set_crack_overlay_model(target, region, None);
    }
    pub fn set_crack_overlay_model(
        &mut self,
        target: Option<BlockPos>,
        region: Option<AtlasRegion>,
        model: Option<&inspection::BlockModel>,
    ) {
        let mut vertices = Vec::new();
        let Some(target) = target else {
            self.crack_vertices = 0;
            self.crack_texture = None;
            return;
        };
        let Some(crack_region) = region else {
            self.crack_vertices = 0;
            self.crack_texture = None;
            return;
        };
        let o = [target.x as f32, target.y as f32, target.z as f32];
        if let Some(model) = model.filter(|m| m.triangles.is_some()) {
            let mut model = model.clone();
            model.textures = [crack_region; 6];
            model.tints = [[1.; 3]; 6];
            model.world_vertices(&mut vertices);
            for vertex in &mut vertices {
                for (a, offset) in o.into_iter().enumerate() {
                    vertex.position[a] += offset;
                }
            }
        } else {
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
                        uv: crate::region_uv(crack_region, uv),
                        shade,
                        color: [1.; 3],
                    });
                }
            }
        }
        self.crack_vertices = vertices.len();
        self.crack_texture = Some(crack_region.texture);
        self.queue
            .write_buffer(&self.crack_buffer, 0, bytemuck::cast_slice(&vertices));
    }
    pub fn rendered_sections(&self) -> usize {
        self.submission.visible_sections
    }
    pub fn submission_stats(&self) -> RenderSubmissionStats {
        self.submission
    }
    pub fn draw_calls(&self) -> usize {
        self.submission.drawn_section_page_batches
            + self.item_page_ranges.len()
            + usize::from(!self.hud_geometry.selection_vertices.is_empty())
            + usize::from(self.crack_vertices > 0)
            + usize::from(!self.hud_geometry.gui_vertices.is_empty())
            + self.hotbar_ranges.len()
            + self.player_ranges.len()
            + self.hud_geometry.item_pages.len()
            + usize::from(!self.hud_geometry.vertices.is_empty())
            + usize::from(!self.hud_geometry.debug_vertices.is_empty())
    }
    pub fn mesh_count(&self) -> usize {
        self.chunks.len()
    }
    /// Read-only selected-section presence/count, without exposing local GPU handles.
    pub fn section_mesh_pages(&self, position: ChunkPos, section_y: i32) -> Option<usize> {
        self.chunks
            .get(&(position, section_y))
            .map(|m| m.pages.len())
    }
    #[must_use]
    pub fn column_state(&self, position: ChunkPos, camera: Camera) -> RendererColumnState {
        let frustum = Frustum::from_camera(camera);
        let mut state = RendererColumnState::default();
        for ((column, section_y), chunk) in &self.chunks {
            if *column != position {
                continue;
            }
            state.resident_section_count += 1;
            if chunk.pages.iter().any(|page| page.index_count > 0) {
                state.drawable_section_count += 1;
                if frustum.intersects_section(*column, *section_y) {
                    state.submitted_section_count += 1;
                } else {
                    state.frustum_culled_section_count += 1;
                }
            }
        }
        state
    }
    /// RustCraft-owned objects; driver allocations are deliberately not inferred.
    pub fn mesh_lifetime_counts(&self) -> (usize, usize, usize, u64) {
        let pages = self.chunks.values().map(|c| c.pages.len()).sum::<usize>();
        let created = self.mesh_buffer_allocations + self.mesh_buffer_reallocations;
        (
            self.chunks.len(),
            pages,
            self.chunks.capacity(),
            created.saturating_sub((pages * 2) as u64),
        )
    }

    pub fn gpu_mesh_byte_components(&self) -> [u64; 4] {
        self.chunks
            .values()
            .flat_map(|chunk| &chunk.pages)
            .fold([0; 4], |mut totals, p| {
                totals[0] += (p.vertex_count * std::mem::size_of::<Vertex>()) as u64;
                totals[1] += u64::from(p.index_count) * 4;
                totals[2] += p.vertex_capacity_bytes;
                totals[3] += p.index_capacity_bytes;
                totals
            })
    }
    pub fn gpu_mesh_logical_bytes(&self) -> usize {
        self.chunks
            .values()
            .flat_map(|chunk| &chunk.pages)
            .map(|page| {
                page.vertex_count * std::mem::size_of::<Vertex>()
                    + page.index_count as usize * std::mem::size_of::<u32>()
            })
            .sum()
    }
    pub fn gpu_mesh_allocated_bytes(&self) -> u64 {
        self.chunks
            .values()
            .flat_map(|chunk| &chunk.pages)
            .map(|page| page.vertex_capacity_bytes + page.index_capacity_bytes)
            .sum()
    }
    pub fn mesh_buffer_allocations(&self) -> u64 {
        self.mesh_buffer_allocations
    }
    pub fn mesh_buffer_reallocations(&self) -> u64 {
        self.mesh_buffer_reallocations
    }
    pub fn mesh_buffer_reuses(&self) -> u64 {
        self.mesh_buffer_reuses
    }
    pub fn set_resident_section_count(&mut self, sections: usize) {
        self.resident_section_count = sections;
    }
    pub fn gpu_ms(&self) -> Option<f64> {
        self.timing.as_ref().and_then(|t| t.milliseconds)
    }
    pub fn atlas_page_count(&self) -> usize {
        self._textures.len()
    }
    pub fn texture_bytes(&self) -> usize {
        self.texture_bytes
    }
    pub fn atlas_occupancy(&self) -> f32 {
        self.atlas_occupancy
    }
    pub fn resource_cache_hit(&self) -> bool {
        self.resource_cache_hit
    }
    pub fn atlas_upload_submit_ms(&self) -> f64 {
        self.atlas_upload_submit_ms
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
    /// Uploads geometry under the explicit single-page compatibility contract.
    ///
    /// Generic compiled-resource meshing must use [`Self::upload_chunk_pages`]
    /// so every page referenced by the block model is preserved.
    pub fn upload_chunk(&mut self, position: ChunkPos, section_y: i32, mesh: &CpuMesh) {
        self.upload_chunk_pages(
            position,
            section_y,
            &[PageMesh {
                texture: TextureHandle(0),
                translucent: false,
                mesh: mesh.clone(),
            }],
        );
    }

    pub fn upload_chunk_pages(
        &mut self,
        position: ChunkPos,
        section_y: i32,
        meshes: &[PageMesh],
    ) -> MeshUploadStats {
        let started = std::time::Instant::now();
        let mut reusable = HashMap::new();
        if let Some(old) = self.chunks.remove(&(position, section_y)) {
            for page in old.pages {
                self.vertices = self.vertices.saturating_sub(page.vertex_count);
                self.indices = self.indices.saturating_sub(page.index_count as usize);
                reusable.insert((page.texture, page.translucent), page);
            }
        }
        let mut upload = MeshUploadStats::default();
        let pages = meshes
            .iter()
            .filter(|page| !page.mesh.is_empty())
            .map(|page| {
                assert!(
                    (page.texture.0 as usize) < self.page_binds.len(),
                    "mesh references missing atlas page {}",
                    page.texture.0
                );
                let vertex_bytes = bytemuck::cast_slice(&page.mesh.vertices);
                let index_bytes = bytemuck::cast_slice(&page.mesh.indices);
                upload.logical_bytes += vertex_bytes.len() + index_bytes.len();
                let old = reusable.remove(&(page.texture, page.translucent));
                let (vertex, vertex_capacity_bytes, index, index_capacity_bytes) =
                    if let Some(old) = old {
                        let (vertex, vertex_capacity_bytes, vertex_reallocated) =
                            reuse_or_allocate_buffer(
                                &self.device,
                                old.vertex,
                                old.vertex_capacity_bytes,
                                vertex_bytes.len(),
                                wgpu::BufferUsages::VERTEX,
                                "chunk-vertices",
                            );
                        let (index, index_capacity_bytes, index_reallocated) =
                            reuse_or_allocate_buffer(
                                &self.device,
                                old.index,
                                old.index_capacity_bytes,
                                index_bytes.len(),
                                wgpu::BufferUsages::INDEX,
                                "chunk-indices",
                            );
                        upload.reallocations +=
                            u64::from(vertex_reallocated) + u64::from(index_reallocated);
                        upload.reuses +=
                            u64::from(!vertex_reallocated) + u64::from(!index_reallocated);
                        (vertex, vertex_capacity_bytes, index, index_capacity_bytes)
                    } else {
                        upload.allocations += 2;
                        let vertex_capacity_bytes = buffer_capacity(vertex_bytes.len());
                        let index_capacity_bytes = buffer_capacity(index_bytes.len());
                        (
                            create_mesh_buffer(
                                &self.device,
                                vertex_capacity_bytes,
                                wgpu::BufferUsages::VERTEX,
                                "chunk-vertices",
                            ),
                            vertex_capacity_bytes,
                            create_mesh_buffer(
                                &self.device,
                                index_capacity_bytes,
                                wgpu::BufferUsages::INDEX,
                                "chunk-indices",
                            ),
                            index_capacity_bytes,
                        )
                    };
                self.queue.write_buffer(&vertex, 0, vertex_bytes);
                self.queue.write_buffer(&index, 0, index_bytes);
                self.vertices += page.mesh.vertices.len();
                self.indices += page.mesh.indices.len();
                GpuPageMesh {
                    texture: page.texture,
                    translucent: page.translucent,
                    vertex,
                    index,
                    vertex_capacity_bytes,
                    index_capacity_bytes,
                    index_count: page.mesh.indices.len() as u32,
                    vertex_count: page.mesh.vertices.len(),
                }
            })
            .collect();
        self.chunks
            .insert((position, section_y), GpuChunk { pages });
        self.mesh_rebuilds += 1;
        self.mesh_buffer_allocations += upload.allocations;
        self.mesh_buffer_reallocations += upload.reallocations;
        self.mesh_buffer_reuses += upload.reuses;
        upload.submit_ms = started.elapsed().as_secs_f64() * 1000.0;
        upload
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
            for page in old.pages {
                self.vertices = self.vertices.saturating_sub(page.vertex_count);
                self.indices = self.indices.saturating_sub(page.index_count as usize);
            }
        }
    }

    pub fn remove_section(&mut self, position: ChunkPos, section_y: i32) {
        if let Some(old) = self.chunks.remove(&(position, section_y)) {
            for page in old.pages {
                self.vertices = self.vertices.saturating_sub(page.vertex_count);
                self.indices = self.indices.saturating_sub(page.index_count as usize);
            }
        }
    }
    pub fn render(&mut self, camera: Camera) -> Result<(), wgpu::SurfaceError> {
        self.render_capture(camera, None)
    }

    /// DX capture coordinates mapping on the renderer thread, without a GPU Wait or blocking recv.
    pub fn render_capture_async(
        &mut self,
        camera: Camera,
        capture: Option<&Path>,
    ) -> Result<(), wgpu::SurfaceError> {
        self.async_capture = true;
        let result = self.render_capture(
            camera,
            if self.pending_capture.is_none() {
                capture
            } else {
                None
            },
        );
        self.async_capture = false;
        result
    }
    pub fn capture_pending(&self) -> bool {
        self.pending_capture.is_some()
    }
    pub fn poll_capture(&mut self) -> Option<Result<CapturedFrame, String>> {
        let pending = self.pending_capture.as_ref()?;
        let _ = self.device.poll(wgpu::PollType::Poll);
        match pending.rx.try_recv() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                self.pending_capture.take();
                return Some(Err(error.to_string()));
            }
            Err(std::sync::mpsc::TryRecvError::Empty)
                if pending.started.elapsed().as_secs() < 30 =>
            {
                return None;
            }
            _ => {
                self.pending_capture.take();
                return Some(Err("capture mapping timeout/disconnected".into()));
            }
        }
        let pending = self.pending_capture.take().unwrap();
        let data = pending.buffer.slice(..).get_mapped_range();
        let stride = (pending.config.width * 4).div_ceil(256) * 256;
        let mut rgba =
            Vec::with_capacity((pending.config.width * pending.config.height * 4) as usize);
        for row in data.chunks(stride as usize) {
            rgba.extend_from_slice(&row[..pending.config.width as usize * 4]);
        }
        if matches!(
            pending.config.format,
            wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm
        ) {
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
        }
        drop(data);
        pending.buffer.unmap();
        Some(Ok(CapturedFrame {
            path: pending.path,
            width: pending.config.width,
            height: pending.config.height,
            rgba,
        }))
    }
    /// Optional one-frame development capture of the actual surface render.
    pub fn render_capture(
        &mut self,
        camera: Camera,
        capture: Option<&Path>,
    ) -> Result<(), wgpu::SurfaceError> {
        self.frame_timing = ApplicationFrameTiming::default();
        let frame_started = self.frame_timing_enabled.then(std::time::Instant::now);
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
        let fog = self.fog.unwrap_or(FogPresentation {
            color: [0.0; 3],
            start: 0.0,
            end: 0.0,
        });
        let uniform = CameraUniform {
            matrix,
            position: [camera.position.x, camera.position.y, camera.position.z, 1.0],
            fog_color: [fog.color[0], fog.color[1], fog.color[2], 1.0],
            fog_range: [
                fog.start,
                fog.end,
                if self.fog.is_some() { 1.0 } else { 0.0 },
                0.0,
            ],
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        let frustum = Frustum::from_camera(camera);
        let visible = visible_section_positions(
            self.chunks.iter().filter_map(|(key, chunk)| {
                chunk
                    .pages
                    .iter()
                    .any(|page| page.index_count > 0)
                    .then_some(*key)
            }),
            frustum,
        );
        let mut world_draws = visible
            .iter()
            .flat_map(|key| {
                self.chunks[key]
                    .pages
                    .iter()
                    .enumerate()
                    .filter(|(_, page)| page.index_count > 0)
                    .map(move |(page_index, page)| {
                        (page.translucent, page.texture, *key, page_index)
                    })
            })
            .collect::<Vec<_>>();
        let draw_distance = |key: &(ChunkPos, i32)| {
            let center = Vec3::new(
                key.0.x as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 * 0.5,
                key.1 as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 * 0.5,
                key.0.z as f32 * CHUNK_SIZE as f32 + CHUNK_SIZE as f32 * 0.5,
            );
            let delta = Vec3::new(
                center.x - camera.position.x,
                center.y - camera.position.y,
                center.z - camera.position.z,
            );
            delta.x * delta.x + delta.y * delta.y + delta.z * delta.z
        };
        world_draws.sort_by(|a, b| match (a.0, b.0) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            (false, false) => {
                (a.1.0, a.2.0.x, a.2.0.z, a.2.1).cmp(&(b.1.0, b.2.0.x, b.2.0.z, b.2.1))
            }
            (true, true) => draw_distance(&b.2)
                .total_cmp(&draw_distance(&a.2))
                .then_with(|| {
                    (a.1.0, a.2.0.x, a.2.0.z, a.2.1).cmp(&(b.1.0, b.2.0.x, b.2.0.z, b.2.1))
                }),
        });
        let meshed_sections = self
            .chunks
            .values()
            .filter(|chunk| chunk.pages.iter().any(|page| page.index_count > 0))
            .count();
        let texture_page_bind_switches = world_draws
            .iter()
            .map(|draw| draw.1)
            .fold((None, 0), |(previous, switches), texture| {
                (
                    Some(texture),
                    switches + usize::from(previous.is_some() && previous != Some(texture)),
                )
            })
            .1;
        let mut active_pipelines = Vec::new();
        if world_draws.iter().any(|draw| !draw.0) || self.item_vertices > 0 {
            active_pipelines.push(0_u8);
        }
        if world_draws.iter().any(|draw| draw.0) {
            active_pipelines.push(1_u8);
        }
        if !self.hud_geometry.selection_vertices.is_empty() {
            active_pipelines.push(2_u8);
        }
        if self.crack_vertices > 0 {
            active_pipelines.push(3);
        }
        if !self.hud_geometry.gui_vertices.is_empty()
            || !self.hud_geometry.hotbar_vertices.is_empty()
            || !self.hud_geometry.player_vertices.is_empty()
        {
            active_pipelines.push(4);
        }
        if !self.hud_geometry.item_vertices.is_empty() {
            active_pipelines.push(5);
        }
        if !self.hud_geometry.vertices.is_empty() || !self.hud_geometry.debug_vertices.is_empty() {
            active_pipelines.push(6);
        }
        let pipeline_switches = active_pipelines
            .windows(2)
            .filter(|pair| pair[0] != pair[1])
            .count();
        self.submission = RenderSubmissionStats {
            resident_sections: self.resident_section_count.max(self.chunks.len()),
            meshed_sections,
            visible_sections: visible.len(),
            culled_sections: meshed_sections.saturating_sub(visible.len()),
            drawn_section_page_batches: world_draws.len(),
            texture_page_bind_switches,
            pipeline_switches,
        };
        let acquire_started = self.frame_timing_enabled.then(std::time::Instant::now);
        let output = self.surface.get_current_texture()?;
        if let Some(start) = acquire_started {
            self.frame_timing.acquire_ms = start.elapsed().as_secs_f64() * 1000.;
            self.frame_timing.prepare_ms =
                start.duration_since(frame_started.unwrap()).as_secs_f64() * 1000.;
        }
        // Some GL surfaces cannot be copied. Use the same render passes/readback on a
        // copyable attachment for that capture, then present the identical state normally.
        let capture_attachment = (capture.is_some()
            && !self.config.usage.contains(wgpu::TextureUsages::COPY_SRC))
        .then(|| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("surface-capture-attachment"),
                size: wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        });
        let view = capture_attachment
            .as_ref()
            .unwrap_or(&output.texture)
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
            for (translucent, texture, key, page_index) in &world_draws {
                if *translucent {
                    continue;
                }
                let page = &self.chunks[key].pages[*page_index];
                pass.set_bind_group(0, &self.page_binds[texture.0 as usize], &[]);
                pass.set_vertex_buffer(0, page.vertex.slice(..));
                pass.set_index_buffer(page.index.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..page.index_count, 0, 0..1);
            }
            if self.item_vertices > 0 {
                pass.set_vertex_buffer(0, self.item_buffer.slice(..));
                for (texture, range) in &self.item_page_ranges {
                    pass.set_bind_group(0, &self.page_binds[texture.0 as usize], &[]);
                    pass.draw(range.clone(), 0..1);
                }
            }
            if world_draws.iter().any(|draw| draw.0) {
                pass.set_pipeline(&self.translucent_pipeline);
                for (_, texture, key, page_index) in world_draws.iter().filter(|draw| draw.0) {
                    let page = &self.chunks[key].pages[*page_index];
                    pass.set_bind_group(0, &self.page_binds[texture.0 as usize], &[]);
                    pass.set_vertex_buffer(0, page.vertex.slice(..));
                    pass.set_index_buffer(page.index.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..page.index_count, 0, 0..1);
                }
            }
        }
        if !self.hud_geometry.selection_vertices.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("depth-tested-selection-outline"),
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
            pass.set_pipeline(&self.selection_pipeline);
            // The selection shader only consumes the camera uniform from this shared bind group;
            // the atlas bindings are not sampled.
            pass.set_bind_group(0, &self.page_binds[0], &[]);
            pass.set_vertex_buffer(0, self.selection_buffer.slice(..));
            pass.draw(0..self.hud_geometry.selection_vertices.len() as u32, 0..1);
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
            let texture = self
                .crack_texture
                .expect("visible crack geometry always retains its texture page");
            pass.set_bind_group(0, &self.page_binds[texture.0 as usize], &[]);
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
            pass.set_bind_group(
                0,
                &self.page_binds[self.container_background.texture.0 as usize],
                &[],
            );
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
            pass.set_vertex_buffer(0, self.hud_buffer.slice(768 * 1024..));
            for (texture, range) in &self.hotbar_ranges {
                pass.set_bind_group(0, &self.page_binds[texture.0 as usize], &[]);
                pass.draw(range.clone(), 0..1);
            }
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
            pass.set_vertex_buffer(0, self.hud_buffer.slice(896 * 1024..));
            for (texture, range) in &self.player_ranges {
                pass.set_bind_group(0, &self.page_binds[texture.0 as usize], &[]);
                pass.draw(range.clone(), 0..1);
            }
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
            pass.set_vertex_buffer(0, self.hud_buffer.slice(1024 * 1024..));
            let mut start = 0_u32;
            for page in &self.hud_geometry.item_pages {
                pass.set_bind_group(0, &self.page_binds[page.texture.0 as usize], &[]);
                let end = start + page.vertices.len() as u32;
                pass.draw(start..end, 0..1);
                start = end;
            }
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
            // All textured HUD items are submitted through item_pages above. This pass contains
            // only procedural color geometry whose shader branch does not sample the bound page.
            pass.set_bind_group(0, &self.page_binds[0], &[]);
            pass.set_vertex_buffer(0, self.hud_buffer.slice(..));
            pass.draw(0..self.hud_geometry.vertices.len() as u32, 0..1);
            if !self.hud_geometry.debug_vertices.is_empty() {
                pass.set_vertex_buffer(0, self.hud_buffer.slice(256 * 1024..));
                pass.draw(0..self.hud_geometry.debug_vertices.len() as u32, 0..1);
            }
        }
        if !self.text_surface.system.rgba.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("unicode-text"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.hud_pipeline);
            pass.set_bind_group(0, &self.text_surface.bind, &[]);
            pass.set_vertex_buffer(0, self.text_surface.vertices.slice(..));
            pass.draw(0..6, 0..1);
        }
        if timed {
            self.timing.as_ref().unwrap().resolve(&mut encoder);
        }
        let readback = capture.map(|_| {
            let mut config = self.config.clone();
            config.usage |= wgpu::TextureUsages::COPY_SRC;
            diagnostic::copy_surface(
                &self.device,
                &mut encoder,
                capture_attachment.as_ref().unwrap_or(&output.texture),
                &config,
            )
        });
        if self.frame_timing_enabled {
            self.frame_timing.submit_at = Some(std::time::Instant::now());
        }
        self.queue.submit(Some(encoder.finish()));
        if timed {
            self.timing.as_mut().unwrap().map();
        }
        if let (Some(path), Some(buffer)) = (capture, readback) {
            if self.async_capture {
                let (tx, rx) = std::sync::mpsc::channel();
                buffer
                    .slice(..)
                    .map_async(wgpu::MapMode::Read, move |result| {
                        let _ = tx.send(result);
                    });
                self.pending_capture = Some(PendingCapture {
                    buffer,
                    rx,
                    config: self.config.clone(),
                    path: path.to_owned(),
                    started: std::time::Instant::now(),
                });
            } else {
                diagnostic::save_capture(&self.device, buffer, &self.config, path);
            }
        }
        if self.frame_timing_enabled {
            self.frame_timing.present_at = Some(std::time::Instant::now());
        }
        output.present();
        if let Some(start) = frame_started {
            self.frame_timing.total_ms = start.elapsed().as_secs_f64() * 1000.;
        }
        if capture_attachment.is_some() {
            self.render_capture(camera, None)?;
        }
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
    upload_texture_with_sampling(device, queue, width, height, rgba, TextureSampling::Nearest)
}

fn upload_texture_with_sampling(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    rgba: &[u8],
    sampling: TextureSampling,
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
    let filter = match sampling {
        TextureSampling::Nearest => wgpu::FilterMode::Nearest,
        TextureSampling::Linear => wgpu::FilterMode::Linear,
    };
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: wgpu::FilterMode::Nearest,
        anisotropy_clamp: 1,
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

fn selection_primitive_state() -> wgpu::PrimitiveState {
    wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::LineList,
        cull_mode: None,
        ..Default::default()
    }
}

fn selection_depth_stencil_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth24Plus,
        depth_write_enabled: false,
        depth_compare: wgpu::CompareFunction::LessEqual,
        stencil: Default::default(),
        bias: wgpu::DepthBiasState {
            constant: -1,
            slope_scale: -1.0,
            clamp: 0.0,
        },
    }
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

    #[test]
    fn linear_underwater_fog_keeps_nearby_geometry_and_attenuates_distance() {
        let fog = FogPresentation {
            color: [0.2, 0.4, 0.62],
            start: 2.0,
            end: 18.0,
        };
        assert_eq!(fog.linear_amount(1.0), 0.0);
        assert_eq!(fog.linear_amount(2.0), 0.0);
        assert!((fog.linear_amount(10.0) - 0.5).abs() < 0.001);
        assert_eq!(fog.linear_amount(24.0), 1.0);
    }

    #[test]
    fn selection_pipeline_depth_tests_without_writing_depth() {
        let depth = selection_depth_stencil_state();
        assert_eq!(depth.format, wgpu::TextureFormat::Depth24Plus);
        assert!(!depth.depth_write_enabled);
        assert_eq!(depth.depth_compare, wgpu::CompareFunction::LessEqual);
        assert_eq!(
            selection_primitive_state().topology,
            wgpu::PrimitiveTopology::LineList
        );
    }

    #[test]
    fn mesh_buffer_capacity_reuses_grows_and_shrinks_after_large_reduction() {
        assert_eq!(buffer_capacity(0), 256);
        assert_eq!(buffer_capacity(1), 256);
        assert_eq!(buffer_capacity(257), 512);
        assert_eq!(
            buffer_capacity_action(1024, 1024),
            BufferCapacityAction::Reuse
        );
        assert_eq!(
            buffer_capacity_action(1024, 1100),
            BufferCapacityAction::Reallocate(2048)
        );
        assert_eq!(
            buffer_capacity_action(1024, 768),
            BufferCapacityAction::Reuse
        );
        assert_eq!(
            buffer_capacity_action(1024, 0),
            BufferCapacityAction::Reallocate(256)
        );
        assert_eq!(
            buffer_capacity_action(4096, 512),
            BufferCapacityAction::Reallocate(512)
        );
        assert_eq!(
            buffer_capacity_action(4096, 1024),
            BufferCapacityAction::Reuse
        );
        assert_eq!(
            buffer_capacity_action(4096, 512 - 1),
            BufferCapacityAction::Reallocate(512)
        );
    }

    fn test_camera() -> Camera {
        Camera {
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            aspect: 1.0,
            fov_y: 90.0_f32.to_radians(),
            near: 1.0,
            far: 10.0,
        }
    }

    #[test]
    fn frustum_accepts_inside_intersecting_and_near_far_edges() {
        let frustum = Frustum::from_camera(test_camera());
        let aabb = |min, max| rustcraft_engine_core::Aabb::new(min, max);
        assert!(
            frustum.intersects_aabb(aabb(Vec3::new(-1.0, -1.0, 4.0), Vec3::new(1.0, 1.0, 6.0)))
        );
        assert!(
            !frustum.intersects_aabb(aabb(Vec3::new(-1.0, -1.0, -3.0), Vec3::new(1.0, 1.0, -1.0)))
        );
        assert!(frustum.intersects_aabb(aabb(Vec3::new(3.5, -1.0, 4.0), Vec3::new(5.5, 1.0, 6.0))));
        assert!(
            frustum.intersects_aabb(aabb(Vec3::new(-0.5, -0.5, 0.5), Vec3::new(0.5, 0.5, 1.5)))
        );
        assert!(
            frustum.intersects_aabb(aabb(Vec3::new(-0.5, -0.5, 9.5), Vec3::new(0.5, 0.5, 10.5)))
        );
        assert!(
            !frustum.intersects_aabb(aabb(Vec3::new(-0.5, -0.5, 11.0), Vec3::new(0.5, 0.5, 12.0)))
        );
    }

    #[test]
    fn frustum_handles_camera_rotation_and_negative_sections() {
        let mut camera = test_camera();
        camera.yaw = std::f32::consts::FRAC_PI_2;
        camera.far = 64.0;
        let frustum = Frustum::from_camera(camera);
        assert!(frustum.intersects_section(ChunkPos { x: 1, z: 0 }, 0));
        assert!(!frustum.intersects_section(ChunkPos { x: -3, z: 0 }, 0));

        camera.position = Vec3::new(-24.0, 8.0, -48.0);
        camera.yaw = 0.0;
        let frustum = Frustum::from_camera(camera);
        assert!(frustum.intersects_section(ChunkPos { x: -2, z: -2 }, 0));
    }

    #[test]
    fn camera_visibility_changes_submission_set_without_changing_resident_sections() {
        let sections = (-4..=4)
            .flat_map(|z| (-4..=4).map(move |x| (ChunkPos { x, z }, 0)))
            .collect::<Vec<_>>();
        let camera = Camera {
            position: Vec3::new(0., 8., -24.),
            yaw: 0.,
            pitch: 0.,
            aspect: 16. / 9.,
            fov_y: 70_f32.to_radians(),
            near: 0.05,
            far: 128.,
        };
        let forward =
            visible_section_positions(sections.iter().copied(), Frustum::from_camera(camera));
        let turned = visible_section_positions(
            sections.iter().copied(),
            Frustum::from_camera(Camera {
                yaw: std::f32::consts::PI,
                ..camera
            }),
        );
        assert!(!forward.is_empty());
        assert_ne!(forward, turned);
        assert!(forward.len() < sections.len());
        assert!(turned.len() < sections.len());
        assert_eq!(sections.len(), 81, "resident snapshot set is unchanged");
    }

    #[test]
    fn section_snapshot_halo_copies_neighbor_voxels_and_light_at_negative_boundaries() {
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: -1, y: 0, z: 0 }, BlockId(4));
        world.set(BlockPos { x: 0, y: 0, z: 0 }, BlockId(5));
        world.set_light(
            BlockPos { x: -1, y: 0, z: 0 },
            rustcraft_engine_core::VoxelLight::new(7, 11),
        );
        let extracted = RenderWorld::from_world(&world);
        let right = extracted
            .chunks()
            .find(|chunk| chunk.position.x == 0)
            .unwrap();
        let left = extracted
            .chunks()
            .find(|chunk| chunk.position.x == -1)
            .unwrap();
        assert_eq!(left.state((16, 0, 0)).block, BlockId(5));
        assert_eq!(right.state((-1, 0, 0)).block, BlockId(4));
        assert_eq!(
            right.light((-1, 0, 0)),
            rustcraft_engine_core::VoxelLight::new(7, 11)
        );
        assert_eq!(extracted.snapshot_bytes(), 2 * 18 * 18 * 18 * 9);
    }

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
                AtlasRegion::grid_cell(TextureHandle(0), 16, 16, 1, 0).unwrap(),
                1.0,
                None,
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
                assert_eq!(
                    vertex.uv,
                    region_uv(
                        AtlasRegion::grid_cell(TextureHandle(0), 16, 16, 1, 0).unwrap(),
                        expected_uv,
                    )
                );
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
            region_uv(
                AtlasRegion::grid_cell(TextureHandle(0), 16, 16, 1, 0).unwrap(),
                [0., 0.],
            ),
            [16.0 / 256.0, 0.]
        );
        assert_eq!(
            region_uv(
                AtlasRegion::grid_cell(TextureHandle(0), 16, 16, 1, 0).unwrap(),
                [1., 1.],
            ),
            [32.0 / 256.0, 16.0 / 256.0]
        );
        assert_eq!(
            region_uv(
                AtlasRegion::grid_cell(TextureHandle(0), 16, 16, 15, 15).unwrap(),
                [1., 1.],
            ),
            [1., 1.]
        );
        let arbitrary =
            AtlasRegion::from_pixels(TextureHandle(3), [1024, 512], [13, 27, 71, 45]).unwrap();
        assert_eq!(arbitrary.texture, TextureHandle(3));
        assert_eq!(arbitrary.uv_min, [13.0 / 1024.0, 27.0 / 512.0]);
        assert_eq!(arbitrary.uv_max, [84.0 / 1024.0, 72.0 / 512.0]);
        assert!(AtlasRegion::from_pixels(TextureHandle(0), [0, 512], [0, 0, 1, 1]).is_none());
        assert!(AtlasRegion::from_pixels(TextureHandle(0), [512, 0], [0, 0, 1, 1]).is_none());
        assert!(AtlasRegion::from_pixels(TextureHandle(0), [512, 512], [0, 0, 0, 1]).is_none());
        assert!(AtlasRegion::from_pixels(TextureHandle(0), [512, 512], [500, 0, 13, 1]).is_none());
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
        fn texture(&self, block: BlockId, _face: Face) -> Option<AtlasRegion> {
            (block.0 != 0).then(|| AtlasRegion::grid_cell(TextureHandle(0), 16, 16, 1, 0).unwrap())
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
    fn missing_sections_use_world_default_instead_of_numeric_zero() {
        let world = World::new(BlockId(41));
        let presentation = RenderWorld::from_world(&world);
        assert_eq!(
            presentation.block(BlockPos { x: 50, y: -7, z: 9 }),
            BlockId(41)
        );
    }

    #[test]
    fn meshing_batches_faces_by_compiled_atlas_page() {
        struct Paged;
        impl BlockTextureResolver for Paged {
            fn texture(&self, block: BlockId, _face: Face) -> Option<AtlasRegion> {
                (block.0 != 0).then(|| AtlasRegion::full(TextureHandle(block.0 - 1)))
            }
            fn opaque(&self, block: BlockId) -> bool {
                block.0 != 0
            }
        }
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 0, y: 0, z: 0 }, BlockId(1));
        world.set(BlockPos { x: 2, y: 0, z: 0 }, BlockId(2));
        let presentation = RenderWorld::from_world(&world);
        let chunk = presentation.chunks().next().unwrap();
        let pages = build_chunk_mesh_pages(&presentation, chunk, &Paged);
        assert_eq!(
            pages.iter().map(|page| page.texture).collect::<Vec<_>>(),
            [TextureHandle(0), TextureHandle(1)]
        );
        assert!(pages.iter().all(|page| page.mesh.indices.len() == 36));
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
    fn arriving_neighbor_culls_both_shared_faces_in_all_horizontal_directions() {
        for (center_block, neighbor_block) in [
            (
                BlockPos { x: 0, y: 0, z: 8 },
                BlockPos { x: -1, y: 0, z: 8 },
            ),
            (
                BlockPos { x: 15, y: 0, z: 8 },
                BlockPos { x: 16, y: 0, z: 8 },
            ),
            (
                BlockPos { x: 8, y: 0, z: 0 },
                BlockPos { x: 8, y: 0, z: -1 },
            ),
            (
                BlockPos { x: 8, y: 0, z: 15 },
                BlockPos { x: 8, y: 0, z: 16 },
            ),
        ] {
            let mut world = World::new(BlockId(0));
            world.set(center_block, BlockId(1));
            world.set(neighbor_block, BlockId(1));
            let presentation = RenderWorld::from_world(&world);
            for position in [center_block, neighbor_block].map(|block| ChunkPos {
                x: block.x.div_euclid(16),
                z: block.z.div_euclid(16),
            }) {
                let chunk = presentation
                    .chunks()
                    .find(|chunk| chunk.position == position && chunk.section_y == 0)
                    .unwrap();
                let mesh = build_chunk_mesh(&presentation, chunk, &Textures);
                assert_eq!(mesh.indices.len(), 5 * 6, "shared edge at {position:?}");
            }
        }
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
                AtlasRegion::grid_cell(TextureHandle(0), 16, 16, 4, 7).unwrap(),
                1.0,
                None,
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

    fn multi_page_model() -> inspection::BlockModel {
        inspection::BlockModel {
            triangles: None,
            state: BlockState::new(BlockId(7)),
            textures: [
                AtlasRegion::full(TextureHandle(1)),
                AtlasRegion::full(TextureHandle(0)),
                AtlasRegion::full(TextureHandle(1)),
                AtlasRegion::full(TextureHandle(0)),
                AtlasRegion::full(TextureHandle(2)),
                AtlasRegion::full(TextureHandle(0)),
            ],
            tints: [[1.; 3]; 6],
            rotation: rustcraft_engine_core::orientation::ModelRotation::IDENTITY,
        }
    }

    #[test]
    fn dropped_block_batches_every_face_by_atlas_page() {
        let model = multi_page_model();
        let mut pages = Vec::new();
        append_dropped_item_pages(
            &mut pages,
            &[ItemSprite {
                model: Some(model.clone()),
                position: Vec3::new(3., 4., 5.),
                top: model.texture(Face::Top),
                side: model.texture(Face::North),
                bottom: model.texture(Face::Bottom),
                tint: [1.; 3],
                age: 17.,
                count: 6,
                hover_start: 0.25,
            }],
        );
        assert_eq!(
            pages.iter().map(|page| page.texture).collect::<Vec<_>>(),
            [TextureHandle(0), TextureHandle(1), TextureHandle(2)]
        );
        assert_eq!(
            pages.iter().map(|page| page.vertices.len()).sum::<usize>(),
            3 * 36
        );
        assert!(pages.iter().all(|page| !page.vertices.is_empty()));
    }
}

#[cfg(test)]
mod m2_tests {
    use super::*;
    struct Materials;
    impl BlockTextureResolver for Materials {
        fn texture(&self, _: BlockId, _: Face) -> Option<AtlasRegion> {
            Some(AtlasRegion::full(TextureHandle(0)))
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

#[cfg(test)]
mod liquid_tests {
    use super::*;

    struct LiquidMaterials;

    impl BlockTextureResolver for LiquidMaterials {
        fn texture(&self, block: BlockId, _face: Face) -> Option<AtlasRegion> {
            (block.0 != 0).then_some(AtlasRegion::full(TextureHandle(0)))
        }
        fn opaque(&self, block: BlockId) -> bool {
            block == BlockId(1)
        }
        fn visible(&self, block: BlockId) -> bool {
            block.0 != 0
        }
        fn translucent(&self, block: BlockId) -> bool {
            block == BlockId(2)
        }
        fn liquid_surface_height(&self, state: BlockState) -> Option<f32> {
            (state.block == BlockId(2)).then_some(0.875)
        }
    }

    fn counts(world: &World) -> (usize, usize, Vec<Vertex>) {
        let snapshot = RenderWorld::from_world(world);
        let pages = snapshot
            .chunks()
            .flat_map(|chunk| build_section_mesh_pages(chunk, &LiquidMaterials))
            .collect::<Vec<_>>();
        let opaque = pages
            .iter()
            .filter(|page| !page.translucent)
            .map(|page| page.mesh.indices.len())
            .sum();
        let liquid = pages
            .iter()
            .filter(|page| page.translucent)
            .map(|page| page.mesh.indices.len())
            .sum();
        let vertices = pages
            .into_iter()
            .flat_map(|page| page.mesh.vertices)
            .collect();
        (opaque, liquid, vertices)
    }

    #[test]
    fn static_liquid_is_lowered_blended_and_emits_no_internal_faces() {
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 1, y: 1, z: 1 }, BlockId(2));
        let (_, liquid, vertices) = counts(&world);
        assert_eq!(liquid, 6 * 6);
        assert!(
            vertices
                .iter()
                .any(|v| (v.position[1] - 1.875).abs() < 1e-6)
        );

        world.set(BlockPos { x: 2, y: 1, z: 1 }, BlockId(2));
        let (_, liquid, _) = counts(&world);
        assert_eq!(liquid, 10 * 6, "water-water face is culled");
    }

    #[test]
    fn liquid_shore_and_floor_keep_the_opaque_neighbor_surface() {
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 1, y: 1, z: 1 }, BlockId(2));
        world.set(BlockPos { x: 2, y: 1, z: 1 }, BlockId(1));
        let (opaque, liquid, _) = counts(&world);
        assert_eq!(liquid, 5 * 6, "liquid face against solid is hidden");
        assert_eq!(opaque, 6 * 6, "the shore block keeps its own exposed faces");

        world.set(BlockPos { x: 2, y: 1, z: 1 }, BlockId(0));
        world.set(BlockPos { x: 1, y: 0, z: 1 }, BlockId(1));
        let (opaque, liquid, _) = counts(&world);
        assert_eq!(liquid, 5 * 6, "liquid bottom face is hidden by the floor");
        assert_eq!(
            opaque,
            6 * 6,
            "floor top stays visible through static water"
        );
    }

    #[test]
    fn water_water_faces_are_culled_across_section_and_chunk_boundaries() {
        for (left, right) in [
            (
                BlockPos { x: 3, y: 15, z: 3 },
                BlockPos { x: 3, y: 16, z: 3 },
            ),
            (
                BlockPos { x: 15, y: 4, z: 3 },
                BlockPos { x: 16, y: 4, z: 3 },
            ),
        ] {
            let mut world = World::new(BlockId(0));
            world.set(left, BlockId(2));
            world.set(right, BlockId(2));
            let (_, liquid, _) = counts(&world);
            assert_eq!(liquid, 10 * 6);
        }
    }
}

/// One fixed-capacity reusable composite text texture. Glyphs are cached on CPU, never one GPU
/// allocation per glyph. Resize changes clipped UVs, not resource count.
struct TextSurface {
    system: text::TextSystem,
    texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    vertices: wgpu::Buffer,
}
impl TextSurface {
    fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        camera: &wgpu::Buffer,
        fonts: rustcraft_content::fonts::FontResources,
    ) -> Result<Self, String> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bounded-text-composite"),
            size: wgpu::Extent3d {
                width: text::SURFACE_LIMIT,
                height: text::SURFACE_LIMIT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text-composite"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text-quad"),
            size: (6 * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            system: text::TextSystem::new(fonts)?,
            texture,
            bind,
            vertices,
        })
    }
    fn update(
        &mut self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        runs: &[text::TextRun],
        width: u32,
        height: u32,
        caret: Option<text::Caret>,
    ) {
        if !self.system.update(runs, width, height, caret) {
            return;
        }
        let w = width.clamp(1, text::SURFACE_LIMIT);
        let h = height.clamp(1, text::SURFACE_LIMIT);
        let upload_started = std::time::Instant::now();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.system.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.system.metrics.upload_us = upload_started.elapsed().as_micros();
        let vertices = [0, 1, 2, 0, 2, 3].map(|i| {
            let p = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]][i];
            Vertex {
                position: [
                    p[0] * w as f32 / width.max(1) as f32 * 2. - 1.,
                    1. - p[1] * h as f32 / height.max(1) as f32 * 2.,
                    0.,
                ],
                uv: [
                    p[0] * w as f32 / text::SURFACE_LIMIT as f32,
                    p[1] * h as f32 / text::SURFACE_LIMIT as f32,
                ],
                shade: 1.,
                color: [1.; 3],
            }
        });
        queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&vertices));
    }
}

/// Resolve whole-resource quads while preserving blend order across arbitrary physical pages.
pub fn resolve_presentation_quads(
    vertices: &mut [Vertex],
    regions: &[AtlasRegion],
) -> Vec<(TextureHandle, std::ops::Range<u32>)> {
    assert!(vertices.is_empty() || vertices.len() == regions.len() * 6);
    let mut ranges = Vec::new();
    for (index, quad) in vertices.as_chunks_mut::<6>().0.iter_mut().enumerate() {
        let region = regions[index];
        for vertex in quad {
            vertex.uv = region_uv(region, vertex.uv);
        }
        ranges.push((region.texture, index as u32 * 6..(index as u32 + 1) * 6));
    }
    ranges
}
