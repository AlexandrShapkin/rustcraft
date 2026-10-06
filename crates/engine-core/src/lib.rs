//! Generic voxel-engine primitives. This crate has no knowledge of first-party gameplay.

use std::collections::HashMap;
pub mod orientation;
pub mod raycast;
pub mod shape;

pub const CHUNK_SIZE: i32 = 16;
pub const CHUNK_VOLUME: usize = (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub u32);

/// Stable identity for a durable spatial entity.
///
/// The high half identifies one process/session allocation namespace and the low half is a
/// monotonic counter inside that namespace. Runtime collection indexes and addresses are never
/// persistence identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub u128);

impl EntityId {
    pub const NIL: Self = Self(0);

    #[must_use]
    pub const fn from_parts(namespace: u64, counter: u64) -> Self {
        Self((namespace as u128) << 64 | counter as u128)
    }

    #[must_use]
    pub const fn namespace(self) -> u64 {
        (self.0 >> 64) as u64
    }

    #[must_use]
    pub const fn counter(self) -> u64 {
        self.0 as u64
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

/// Compact state handle. The definition interprets variant; legacy adapters retain orientation bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockState {
    pub block: BlockId,
    pub variant: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VoxelLight(pub u8);
impl VoxelLight {
    pub fn new(sky: u8, block: u8) -> Self {
        Self((sky.min(15) << 4) | block.min(15))
    }
    pub fn sky(self) -> u8 {
        self.0 >> 4
    }
    pub fn block(self) -> u8 {
        self.0 & 15
    }
}

pub type SectionPos = (ChunkPos, i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, n: f32) -> Self {
        Self::new(self.x * n, self.y * n, self.z * n)
    }
}

impl Vec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }
    pub fn translated(self, delta: Vec3) -> Self {
        Self::new(
            Vec3::new(
                self.min.x + delta.x,
                self.min.y + delta.y,
                self.min.z + delta.z,
            ),
            Vec3::new(
                self.max.x + delta.x,
                self.max.y + delta.y,
                self.max.z + delta.z,
            ),
        )
    }
    pub fn intersects(self, other: Self) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }
}

#[derive(Debug, Clone)]
pub struct Chunk {
    blocks: Vec<BlockState>,
}

impl Chunk {
    #[must_use]
    pub fn new(fill: BlockId) -> Self {
        Self {
            blocks: vec![
                BlockState {
                    block: fill,
                    variant: 0
                };
                CHUNK_VOLUME
            ],
        }
    }
    #[must_use]
    pub fn get(&self, local: (u8, u8, u8)) -> BlockId {
        self.blocks[block_index(local)].block
    }
    pub fn state(&self, local: (u8, u8, u8)) -> BlockState {
        self.blocks[block_index(local)]
    }
    /// Dense section storage in `y * 256 + z * 16 + x` order.
    #[must_use]
    pub fn states(&self) -> &[BlockState] {
        &self.blocks
    }
    pub fn set_state(&mut self, local: (u8, u8, u8), state: BlockState) {
        self.blocks[block_index(local)] = state;
    }
    pub fn set(&mut self, local: (u8, u8, u8), block: BlockId) {
        self.blocks[block_index(local)] = BlockState { block, variant: 0 };
    }
}

/// Efficient unpublished section construction for generators and storage loaders.
/// The resulting dense section can be published to a `World` in one operation.
#[derive(Debug, Clone)]
pub struct ChunkBuilder {
    chunk: Chunk,
}

impl ChunkBuilder {
    #[must_use]
    pub fn new(fill: BlockState) -> Self {
        Self {
            chunk: Chunk {
                blocks: vec![fill; CHUNK_VOLUME],
            },
        }
    }

    pub fn set(&mut self, local: (u8, u8, u8), state: BlockState) {
        self.chunk.set_state(local, state);
    }

    #[must_use]
    pub fn state(&self, local: (u8, u8, u8)) -> BlockState {
        self.chunk.state(local)
    }

    pub fn fill(&mut self, state: BlockState) {
        self.chunk.blocks.fill(state);
    }

    #[must_use]
    pub fn finish(self) -> Chunk {
        self.chunk
    }
}

#[derive(Debug, Clone)]
pub struct World {
    sections: HashMap<(ChunkPos, i32), Chunk>,
    resident_columns: std::collections::HashSet<ChunkPos>,
    safe_columns: std::collections::HashSet<ChunkPos>,
    enforce_column_availability: bool,
    default_block: BlockId,
    lights: HashMap<SectionPos, Vec<VoxelLight>>,
}

impl World {
    #[must_use]
    pub fn new(default_block: BlockId) -> Self {
        Self {
            sections: HashMap::new(),
            resident_columns: std::collections::HashSet::new(),
            safe_columns: std::collections::HashSet::new(),
            enforce_column_availability: false,
            default_block,
            lights: HashMap::new(),
        }
    }
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.resident_columns.len()
    }
    #[must_use]
    pub fn chunk(&self, position: ChunkPos) -> Option<&Chunk> {
        self.sections.get(&(position, 0))
    }
    pub fn chunk_positions(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.resident_columns.iter().copied()
    }
    /// Enable an explicit unavailable-column boundary for streamed worlds. Standalone authored
    /// worlds retain their historical implicit-default behavior until opting in.
    pub fn enforce_column_availability(&mut self, enforce: bool) {
        self.enforce_column_availability = enforce;
        if enforce && self.safe_columns.is_empty() {
            self.safe_columns.clone_from(&self.resident_columns);
        }
    }
    #[must_use]
    pub fn column_available(&self, position: ChunkPos) -> bool {
        !self.enforce_column_availability || self.safe_columns.contains(&position)
    }
    /// Release or revoke a resident column at the simulation/interaction frontier. Presentation
    /// owners use this after a current renderable mesh exists, keeping authoritative residency
    /// separate from player-observable safety.
    pub fn set_column_safe(&mut self, position: ChunkPos, safe: bool) -> bool {
        if safe && !self.resident_columns.contains(&position) {
            return false;
        }
        if safe {
            self.safe_columns.insert(position);
        } else {
            self.safe_columns.remove(&position);
        }
        true
    }
    pub fn safe_column_positions(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.safe_columns.iter().copied()
    }
    #[must_use]
    pub fn residency_enforced(&self) -> bool {
        self.enforce_column_availability
    }
    /// Scalar lighting ownership; derived sky sections in resident columns are legitimate.
    pub fn light_lifetime_counts(&self) -> (usize, usize) {
        (
            self.lights.len(),
            self.lights
                .keys()
                .filter(|(c, _)| !self.resident_columns.contains(c))
                .count(),
        )
    }
    pub fn column_resident(&self, position: ChunkPos) -> bool {
        self.resident_columns.contains(&position)
    }
    pub fn column_positions(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.resident_columns.iter().copied()
    }
    pub fn section_positions(&self) -> impl Iterator<Item = (ChunkPos, i32)> + '_ {
        self.sections.keys().copied()
    }
    #[must_use]
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }
    /// Publish a fully constructed section. Callers must not expose the builder before finish.
    pub fn publish_section(&mut self, position: ChunkPos, section_y: i32, section: Chunk) {
        self.resident_columns.insert(position);
        if !self.enforce_column_availability {
            self.safe_columns.insert(position);
        }
        self.sections.insert((position, section_y), section);
    }
    /// Atomically replace a column's sections after generation or disk loading has completed.
    pub fn publish_column(
        &mut self,
        position: ChunkPos,
        sections: Vec<(i32, Chunk)>,
    ) -> Result<(), &'static str> {
        let mut ys = std::collections::HashSet::with_capacity(sections.len());
        if sections.iter().any(|(y, _)| !ys.insert(*y)) {
            return Err("duplicate section in column publication");
        }
        self.remove_column(position);
        self.resident_columns.insert(position);
        if !self.enforce_column_availability {
            self.safe_columns.insert(position);
        }
        for (section_y, section) in sections {
            self.sections.insert((position, section_y), section);
        }
        Ok(())
    }
    pub fn remove_section(&mut self, position: ChunkPos, section_y: i32) -> Option<Chunk> {
        self.lights.remove(&(position, section_y));
        self.sections.remove(&(position, section_y))
    }
    pub fn remove_column(&mut self, position: ChunkPos) -> Vec<(i32, Chunk)> {
        self.resident_columns.remove(&position);
        self.safe_columns.remove(&position);
        self.lights
            .retain(|(candidate, _), _| *candidate != position);
        let section_ys = self
            .sections
            .keys()
            .filter_map(|(chunk_pos, section_y)| (*chunk_pos == position).then_some(*section_y))
            .collect::<Vec<_>>();
        section_ys
            .into_iter()
            .filter_map(|section_y| {
                self.sections
                    .remove(&(position, section_y))
                    .map(|chunk| (section_y, chunk))
            })
            .collect()
    }
    pub fn light(&self, p: BlockPos) -> VoxelLight {
        let (c, local) = split_block(p);
        self.lights
            .get(&(c, p.y.div_euclid(16)))
            .map_or(VoxelLight::default(), |v| v[block_index(local)])
    }
    /// Dense light values for a section, when lighting has allocated that section.
    #[must_use]
    pub fn section_lights(&self, position: ChunkPos, section_y: i32) -> Option<&[VoxelLight]> {
        self.lights.get(&(position, section_y)).map(Vec::as_slice)
    }
    /// Install a completely constructed section-light array at an ownership boundary.
    /// Callers must provide exactly one value per voxel so streamed initial lighting can be
    /// applied by moving storage instead of replaying per-voxel updates on the simulation thread.
    pub fn replace_section_lights(
        &mut self,
        position: ChunkPos,
        section_y: i32,
        lights: Vec<VoxelLight>,
    ) -> Result<(), Vec<VoxelLight>> {
        if lights.len() != CHUNK_VOLUME {
            return Err(lights);
        }
        self.lights.insert((position, section_y), lights);
        Ok(())
    }
    pub fn set_light(&mut self, p: BlockPos, light: VoxelLight) {
        let (c, local) = split_block(p);
        self.lights
            .entry((c, p.y.div_euclid(16)))
            .or_insert_with(|| vec![VoxelLight::default(); CHUNK_VOLUME])[block_index(local)] =
            light;
    }
    #[must_use]
    pub fn section(&self, position: ChunkPos, section_y: i32) -> Option<&Chunk> {
        self.sections.get(&(position, section_y))
    }
    fn ensure_section(&mut self, position: ChunkPos, section_y: i32) -> &mut Chunk {
        self.resident_columns.insert(position);
        self.sections
            .entry((position, section_y))
            .or_insert_with(|| Chunk::new(self.default_block))
    }
    #[must_use]
    pub fn get(&self, position: BlockPos) -> BlockId {
        let (chunk, local) = split_block(position);
        let section_y = position.y.div_euclid(CHUNK_SIZE);
        self.sections
            .get(&(chunk, section_y))
            .map_or(self.default_block, |c| c.get(local))
    }
    pub fn set(&mut self, position: BlockPos, block: BlockId) {
        let (chunk, local) = split_block(position);
        let section_y = position.y.div_euclid(CHUNK_SIZE);
        self.ensure_section(chunk, section_y).set(local, block);
    }
    pub fn empty_block(&self) -> BlockId {
        self.default_block
    }
    #[must_use]
    pub const fn default_state(&self) -> BlockState {
        BlockState::new(self.default_block)
    }
    pub fn state(&self, position: BlockPos) -> BlockState {
        let (chunk, local) = split_block(position);
        self.section(chunk, position.y.div_euclid(CHUNK_SIZE))
            .map_or(
                BlockState {
                    block: self.default_block,
                    variant: 0,
                },
                |c| c.state(local),
            )
    }
    pub fn set_state(&mut self, position: BlockPos, state: BlockState) {
        let (chunk, local) = split_block(position);
        self.ensure_section(chunk, position.y.div_euclid(CHUNK_SIZE))
            .set_state(local, state);
    }
    pub fn fill_box(&mut self, min: BlockPos, max_inclusive: BlockPos, block: BlockId) {
        for y in min.y..=max_inclusive.y {
            for z in min.z..=max_inclusive.z {
                for x in min.x..=max_inclusive.x {
                    self.set(BlockPos { x, y, z }, block);
                }
            }
        }
    }
    #[must_use]
    pub fn collides(&self, bounds: Aabb, is_solid: impl Fn(BlockId) -> bool) -> bool {
        self.collides_shapes(bounds, |state, _| is_solid(state.block))
    }
    pub fn collides_shapes(
        &self,
        bounds: Aabb,
        overlaps: impl Fn(BlockState, shape::LocalBox) -> bool,
    ) -> bool {
        let min = BlockPos {
            x: bounds.min.x.floor() as i32,
            y: bounds.min.y.floor() as i32,
            z: bounds.min.z.floor() as i32,
        };
        let max = BlockPos {
            x: (bounds.max.x - f32::EPSILON).floor() as i32,
            y: (bounds.max.y - f32::EPSILON).floor() as i32,
            z: (bounds.max.z - f32::EPSILON).floor() as i32,
        };
        if self.enforce_column_availability {
            for z in min.z.div_euclid(CHUNK_SIZE)..=max.z.div_euclid(CHUNK_SIZE) {
                for x in min.x.div_euclid(CHUNK_SIZE)..=max.x.div_euclid(CHUNK_SIZE) {
                    if !self.column_available(ChunkPos { x, z }) {
                        return true;
                    }
                }
            }
        }
        for y in min.y..=max.y {
            for z in min.z..=max.z {
                for x in min.x..=max.x {
                    let local = shape::LocalBox {
                        min: [
                            bounds.min.x - x as f32,
                            bounds.min.y - y as f32,
                            bounds.min.z - z as f32,
                        ],
                        max: [
                            bounds.max.x - x as f32,
                            bounds.max.y - y as f32,
                            bounds.max.z - z as f32,
                        ],
                    };
                    if overlaps(self.state(BlockPos { x, y, z }), local) {
                        return true;
                    }
                }
            }
        }
        false
    }
    pub fn move_and_collide(
        &self,
        bounds: Aabb,
        delta: Vec3,
        is_solid: impl Fn(BlockId) -> bool + Copy,
    ) -> (Aabb, Vec3) {
        self.move_and_collide_shapes(bounds, delta, |state, _| is_solid(state.block))
    }
    pub fn move_and_collide_shapes(
        &self,
        bounds: Aabb,
        delta: Vec3,
        overlaps: impl Fn(BlockState, shape::LocalBox) -> bool + Copy,
    ) -> (Aabb, Vec3) {
        let mut current = bounds;
        let mut moved = Vec3::ZERO;
        for (axis, amount) in [(0, delta.x), (1, delta.y), (2, delta.z)] {
            if amount == 0.0 {
                continue;
            }
            let attempt = axis_delta(axis, amount);
            if !self.collides_shapes(current.translated(attempt), overlaps) {
                current = current.translated(attempt);
                moved = add(moved, attempt);
                continue;
            }
            let mut low = 0.0;
            let mut high = amount;
            for _ in 0..12 {
                let middle = (low + high) * 0.5;
                if self.collides_shapes(current.translated(axis_delta(axis, middle)), overlaps) {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            let allowed = axis_delta(axis, low);
            current = current.translated(allowed);
            moved = add(moved, allowed);
        }
        (current, moved)
    }
}

fn axis_delta(axis: i32, amount: f32) -> Vec3 {
    match axis {
        0 => Vec3::new(amount, 0.0, 0.0),
        1 => Vec3::new(0.0, amount, 0.0),
        _ => Vec3::new(0.0, 0.0, amount),
    }
}
fn add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

#[must_use]
pub fn split_block(position: BlockPos) -> (ChunkPos, (u8, u8, u8)) {
    let cx = position.x.div_euclid(CHUNK_SIZE);
    let cz = position.z.div_euclid(CHUNK_SIZE);
    let lx = position.x.rem_euclid(CHUNK_SIZE) as u8;
    let lz = position.z.rem_euclid(CHUNK_SIZE) as u8;
    (
        ChunkPos { x: cx, z: cz },
        (lx, position.y.rem_euclid(CHUNK_SIZE) as u8, lz),
    )
}

#[must_use]
pub fn block_index(local: (u8, u8, u8)) -> usize {
    local.1 as usize * 256 + local.2 as usize * 16 + local.0 as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordinates_handle_negative_boundaries() {
        assert_eq!(
            split_block(BlockPos {
                x: -1,
                y: 0,
                z: -17
            }),
            (ChunkPos { x: -1, z: -2 }, (15, 0, 15))
        );
        assert_eq!(
            split_block(BlockPos { x: 16, y: 16, z: 0 }),
            (ChunkPos { x: 1, z: 0 }, (0, 0, 0))
        );
    }
    #[test]
    fn indexing_is_contiguous_and_stable() {
        assert_eq!(block_index((0, 0, 0)), 0);
        assert_eq!(block_index((15, 15, 15)), 4095);
    }
    #[test]
    fn world_spans_chunks() {
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: -1, y: 2, z: 16 }, BlockId(4));
        world.set(BlockPos { x: 16, y: 2, z: 16 }, BlockId(5));
        assert_eq!(world.get(BlockPos { x: -1, y: 2, z: 16 }), BlockId(4));
        assert_eq!(world.chunk_count(), 2);
    }
    #[test]
    fn unavailable_columns_block_motion_and_are_removed_from_ray_queries() {
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 0, y: 0, z: 0 }, BlockId(1));
        world.enforce_column_availability(true);
        let (bounds, moved) = world.move_and_collide(
            Aabb::new(Vec3::new(0.2, 1.0, 0.2), Vec3::new(0.8, 2.0, 0.8)),
            Vec3::new(16.0, 0.0, 0.0),
            |_| false,
        );
        assert!(moved.x < 15.3);
        assert!(bounds.max.x <= 16.0);
        assert!(
            raycast::cast(
                &world,
                Vec3::new(0.5, 1.5, 0.5),
                Vec3::new(1.0, 0.0, 0.0),
                32.0,
                |block| block != BlockId(0)
            )
            .is_none()
        );
        world
            .publish_column(ChunkPos { x: 0, z: 0 }, Vec::new())
            .unwrap();
        world
            .publish_column(ChunkPos { x: 1, z: 0 }, Vec::new())
            .unwrap();
        assert!(!world.column_available(ChunkPos { x: 1, z: 0 }));
        assert!(world.set_column_safe(ChunkPos { x: 0, z: 0 }, true));
        assert!(world.set_column_safe(ChunkPos { x: 1, z: 0 }, true));
        let (_, moved) = world.move_and_collide(
            Aabb::new(Vec3::new(0.2, 1.0, 0.2), Vec3::new(0.8, 2.0, 0.8)),
            Vec3::new(16.0, 0.0, 0.0),
            |_| false,
        );
        assert!(moved.x > 15.0);
    }
    #[test]
    fn removing_a_column_also_releases_light_only_vertical_sections() {
        let mut world = World::new(BlockId(0));
        let column = ChunkPos { x: -2, z: 3 };
        world.publish_column(column, Vec::new()).unwrap();
        world.enforce_column_availability(true);
        for y in [-1, 0, 8, 9] {
            world.set_light(
                BlockPos {
                    x: -32,
                    y: y * 16,
                    z: 48,
                },
                VoxelLight::new(15, 7),
            );
        }
        assert_eq!(
            world.section_lights(column, -1).unwrap().len(),
            CHUNK_VOLUME
        );
        world.remove_column(column);
        for y in [-1, 0, 8, 9] {
            assert!(world.section_lights(column, y).is_none());
            assert_eq!(
                world.light(BlockPos {
                    x: -32,
                    y: y * 16,
                    z: 48
                }),
                VoxelLight::default()
            );
        }
        assert!(!world.column_available(column));
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;
    #[test]
    fn block_state_remains_compact() {
        assert_eq!(std::mem::size_of::<BlockState>(), 8);
    }
    #[test]
    fn states_preserve_variants_and_setting_block_resets_them() {
        let mut w = World::new(BlockId(99));
        let p = BlockPos {
            x: -16,
            y: 16,
            z: -1,
        };
        assert_eq!(w.state(p).block, BlockId(99));
        let state = BlockState {
            block: BlockId(7),
            variant: 12,
        };
        w.set_state(p, state);
        assert_eq!(w.state(p), state);
        assert_eq!(w.get(p), BlockId(7));
        w.set(p, BlockId(8));
        assert_eq!(w.state(p).variant, 0);
    }
}
