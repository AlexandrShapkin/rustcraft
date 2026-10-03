//! Beta-recognizable, order-independent overworld policy. This module owns all block choices;
//! `rustcraft-world` only supplies the generic generation contract and immutable result boundary.

use rustcraft_engine_core::{BlockId, BlockState, Chunk, ChunkBuilder, ChunkPos};
use rustcraft_world::{ChunkGenerator, SemanticBlockResolver, WorldError};
use std::sync::Arc;

mod v2;
pub use v2::{
    Biome, Climate, MinecraftOverworldGeneratorV2, V2GenerationMetrics, minecraft_overworld_v2,
    select_safe_spawn,
};

pub const OVERWORLD_GENERATOR_ID: &str = "minecraft_b173:overworld";
pub const OVERWORLD_V1_VERSION: u32 = 1;
pub const OVERWORLD_V2_VERSION: u32 = 2;
pub const DEFAULT_OVERWORLD_VERSION: u32 = OVERWORLD_V2_VERSION;

const HEIGHT: i32 = 128;
const SEA_LEVEL: i32 = 64;
/// Minecraft package-owned water state: zero currently means a still/source cell. Other variant
/// values remain available for a future level/flow policy without changing generic storage.
pub const WATER_SOURCE_VARIANT: u16 = 0;

fn water_source_state(block: BlockId) -> BlockState {
    BlockState {
        block,
        variant: WATER_SOURCE_VARIANT,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OverworldBlocks {
    pub air: BlockId,
    pub stone: BlockId,
    pub grass: BlockId,
    pub dirt: BlockId,
    pub bedrock: BlockId,
    pub water: BlockId,
    pub sand: BlockId,
    pub log: BlockId,
    pub leaves: BlockId,
    pub coal_ore: BlockId,
    pub iron_ore: BlockId,
    pub gold_ore: BlockId,
    pub diamond_ore: BlockId,
}

pub struct MinecraftOverworldGenerator {
    blocks: OverworldBlocks,
}

pub struct MinecraftLegacyBlockResolver;
impl SemanticBlockResolver for MinecraftLegacyBlockResolver {
    fn key_for(&self, state: BlockState) -> Option<&str> {
        super::blocks::BLOCKS
            .iter()
            .find(|definition| definition.id == state.block)
            .map(|definition| definition.name)
    }

    fn state_for(&self, key: &str, variant: u16) -> Option<BlockState> {
        super::blocks::BLOCKS
            .iter()
            .find(|definition| definition.name == key)
            .map(|definition| BlockState {
                block: definition.id,
                variant,
            })
    }
}

/// Canonical game-owned semantic state encoding for diagnostics/regression hashes. Runtime dense
/// handles are resolved before hashing and therefore never become the v2 determinism contract.
pub fn update_semantic_state_hash(hasher: &mut blake3::Hasher, state: BlockState) {
    let key = super::blocks::BLOCKS
        .iter()
        .find(|definition| definition.id == state.block)
        .map(|definition| definition.name)
        .expect("generated Minecraft state resolves to a semantic BlockKey");
    hasher.update(&(key.len() as u32).to_le_bytes());
    hasher.update(key.as_bytes());
    hasher.update(&state.variant.to_le_bytes());
}

pub fn minecraft_overworld() -> MinecraftOverworldGenerator {
    use super::blocks as b;
    MinecraftOverworldGenerator::new(OverworldBlocks {
        air: b::AIR.id,
        stone: b::STONE.id,
        grass: b::GRASS.id,
        dirt: b::DIRT.id,
        bedrock: b::BEDROCK.id,
        water: b::WATER.id,
        sand: b::SAND.id,
        log: b::LOG.id,
        leaves: b::LEAVES.id,
        coal_ore: b::COAL_ORE.id,
        iron_ore: b::IRON_ORE.id,
        gold_ore: b::GOLD_ORE.id,
        diamond_ore: b::DIAMOND_ORE.id,
    })
}

impl MinecraftOverworldGenerator {
    pub const fn new(blocks: OverworldBlocks) -> Self {
        Self { blocks }
    }
}

impl ChunkGenerator for MinecraftOverworldGenerator {
    fn id(&self) -> &str {
        OVERWORLD_GENERATOR_ID
    }
    fn version(&self) -> u32 {
        OVERWORLD_V1_VERSION
    }

    fn generate(&self, seed: i64, position: ChunkPos) -> Result<Vec<(i32, Chunk)>, WorldError> {
        let mut sections = (0..HEIGHT / 16)
            .map(|_| ChunkBuilder::new(BlockState::new(self.blocks.air)))
            .collect::<Vec<_>>();
        let mut top_heights = [[0_i32; 16]; 16];
        for z in 0..16_i32 {
            for x in 0..16_i32 {
                let wx = position
                    .x
                    .checked_mul(16)
                    .and_then(|v| v.checked_add(x))
                    .ok_or(WorldError::InvalidData("world coordinate overflow"))?;
                let wz = position
                    .z
                    .checked_mul(16)
                    .and_then(|v| v.checked_add(z))
                    .ok_or(WorldError::InvalidData("world coordinate overflow"))?;
                let height = terrain_height(seed, wx, wz);
                top_heights[z as usize][x as usize] = height;
                let beach = (SEA_LEVEL - 4..=SEA_LEVEL + 2).contains(&height);
                for y in 0..HEIGHT {
                    let block = if y <= bedrock_top(seed, wx, wz) {
                        self.blocks.bedrock
                    } else if y < height - 4 {
                        self.blocks.stone
                    } else if y < height {
                        if beach {
                            self.blocks.sand
                        } else {
                            self.blocks.dirt
                        }
                    } else if y == height {
                        if beach {
                            self.blocks.sand
                        } else {
                            self.blocks.grass
                        }
                    } else if y <= SEA_LEVEL {
                        self.blocks.water
                    } else {
                        self.blocks.air
                    };
                    if block == self.blocks.water {
                        set_world_state(&mut sections, x, y, z, water_source_state(block));
                    } else {
                        set_world(&mut sections, x, y, z, block);
                    }
                }
            }
        }

        // Feature origins are sampled from a one-chunk halo and clipped into this result.
        // Thus adjacent chunks agree without depending on generation order.
        carve_caves(seed, position, &mut sections, &self.blocks);
        place_ores(seed, position, &mut sections, &self.blocks);
        place_trees(seed, position, &mut sections, &top_heights, &self.blocks);
        Ok(sections
            .into_iter()
            .enumerate()
            .map(|(y, section)| (y as i32, section.finish()))
            .collect())
    }
}

/// Resolve the exact persisted Minecraft generator identity. There is intentionally no
/// "nearest" or latest-version fallback because mixing algorithms creates permanent seams.
pub fn resolve_overworld_generator(
    generator_id: &str,
    version: u32,
) -> Result<Arc<dyn ChunkGenerator>, String> {
    if generator_id != OVERWORLD_GENERATOR_ID {
        return Err(format!(
            "unsupported Minecraft generator {generator_id}:v{version}; expected {OVERWORLD_GENERATOR_ID}"
        ));
    }
    match version {
        OVERWORLD_V1_VERSION => Ok(Arc::new(minecraft_overworld())),
        OVERWORLD_V2_VERSION => Ok(Arc::new(minecraft_overworld_v2())),
        _ => Err(format!(
            "unsupported Minecraft generator {generator_id}:v{version}; supported versions are 1 and 2"
        )),
    }
}

pub fn default_overworld_generator() -> Arc<dyn ChunkGenerator> {
    resolve_overworld_generator(OVERWORLD_GENERATOR_ID, DEFAULT_OVERWORLD_VERSION)
        .expect("compiled default Minecraft generator is registered")
}

/// Select a bounded first-time startup column using the exact generator version's game policy.
/// The resulting player position itself is persisted by the existing player record.
pub fn initial_spawn_column(seed: i64, version: u32) -> ChunkPos {
    for radius in 0..=64_i32 {
        for grid_z in -radius..=radius {
            for grid_x in -radius..=radius {
                if radius != 0 && grid_x.abs().max(grid_z.abs()) != radius {
                    continue;
                }
                let x = grid_x * 8;
                let z = grid_z * 8;
                let suitable = match version {
                    OVERWORLD_V1_VERSION => terrain_height(seed, x, z) > SEA_LEVEL + 2,
                    OVERWORLD_V2_VERSION => {
                        let biome = MinecraftOverworldGeneratorV2::biome(seed, x, z);
                        let height = MinecraftOverworldGeneratorV2::terrain_height(seed, x, z);
                        height > SEA_LEVEL + 2 && !matches!(biome, Biome::Ocean | Biome::Beach)
                    }
                    _ => false,
                };
                if suitable {
                    return ChunkPos {
                        x: x.div_euclid(16),
                        z: z.div_euclid(16),
                    };
                }
            }
        }
    }
    ChunkPos { x: 0, z: 0 }
}

fn set_world(sections: &mut [ChunkBuilder], x: i32, y: i32, z: i32, block: BlockId) {
    set_world_state(sections, x, y, z, BlockState::new(block));
}
fn set_world_state(sections: &mut [ChunkBuilder], x: i32, y: i32, z: i32, state: BlockState) {
    sections[(y / 16) as usize].set((x as u8, (y % 16) as u8, z as u8), state);
}
fn get_world(sections: &[ChunkBuilder], x: i32, y: i32, z: i32) -> BlockId {
    let state = sections[(y / 16) as usize].state((x as u8, (y % 16) as u8, z as u8));
    state.block
}

fn terrain_height(seed: i64, x: i32, z: i32) -> i32 {
    let broad = value_noise_2d(seed, x, z, 72, 0x7465727261696e01) * 12.0;
    let hills = value_noise_2d(seed, x, z, 28, 0x7465727261696e02) * 5.0;
    let detail = value_noise_2d(seed, x, z, 11, 0x7465727261696e03) * 2.0;
    (62.0 + broad + hills + detail).round().clamp(45.0, 91.0) as i32
}

/// Diagnostic access to the frozen v1 height policy. Generation callers should resolve the
/// versioned `ChunkGenerator` instead.
pub fn v1_terrain_height(seed: i64, x: i32, z: i32) -> i32 {
    terrain_height(seed, x, z)
}

fn bedrock_top(seed: i64, x: i32, z: i32) -> i32 {
    (hash(seed, 0x626564726f636b00, x as i64, 0, z as i64) % 5) as i32
}

fn carve_caves(seed: i64, pos: ChunkPos, sections: &mut [ChunkBuilder], blocks: &OverworldBlocks) {
    for oz in pos.z as i64 - 1..=pos.z as i64 + 1 {
        for ox in pos.x as i64 - 1..=pos.x as i64 + 1 {
            for cave in 0..2_i64 {
                let h = hash(seed, 0x6361766573000001 + cave as u64, ox, 0, oz);
                if !h.is_multiple_of(4) {
                    continue;
                }
                let cx = ox * 16 + ((h >> 8) % 16) as i64;
                let cz = oz * 16 + ((h >> 16) % 16) as i64;
                let cy = 8 + ((h >> 24) % 47) as i64;
                let length = 7 + ((h >> 32) % 12) as i64;
                let radius = 1.4 + ((h >> 40) % 17) as f64 / 10.0;
                let dx = (((h >> 48) % 2001) as f64 / 1000.0) - 1.0;
                let dz = (((h >> 36) % 2001) as f64 / 1000.0) - 1.0;
                for step in 0..length {
                    let px = cx + (dx * step as f64).round() as i64;
                    let pz = cz + (dz * step as f64).round() as i64;
                    let py = cy + ((step as f64 * 0.31).sin() * 2.0) as i64;
                    for z in -2_i64..=2 {
                        for x in -2_i64..=2 {
                            for y in -2_i64..=2 {
                                if (x * x + y * y + z * z) as f64 > radius * radius {
                                    continue;
                                }
                                let lx = px + x - pos.x as i64 * 16;
                                let ly = py + y;
                                let lz = pz + z - pos.z as i64 * 16;
                                if !(0..16).contains(&lx)
                                    || !(1..HEIGHT as i64).contains(&ly)
                                    || !(0..16).contains(&lz)
                                {
                                    continue;
                                }
                                let (lx, ly, lz) = (lx as i32, ly as i32, lz as i32);
                                if get_world(sections, lx, ly, lz) != blocks.bedrock {
                                    set_world(
                                        sections,
                                        lx,
                                        ly,
                                        lz,
                                        if ly <= SEA_LEVEL {
                                            blocks.water
                                        } else {
                                            blocks.air
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn place_ores(seed: i64, pos: ChunkPos, sections: &mut [ChunkBuilder], blocks: &OverworldBlocks) {
    let ores = [
        (blocks.coal_ore, 18, 19),
        (blocks.iron_ore, 12, 13),
        (blocks.gold_ore, 7, 8),
        (blocks.diamond_ore, 4, 5),
    ];
    for (ore, attempts, max_y) in ores {
        for vein in 0..attempts {
            let h = hash(
                seed,
                0x6f72657300000000 | u64::from(ore.0),
                pos.x as i64 * 131 + vein,
                0,
                pos.z as i64 * 313 - vein,
            );
            let cx = ((h >> 8) % 16) as i32;
            let cy = 1 + ((h >> 16) % max_y as u64) as i32;
            let cz = ((h >> 24) % 16) as i32;
            let radius = 1 + ((h >> 32) % 2) as i32;
            for z in -radius..=radius {
                for y in -radius..=radius {
                    for x in -radius..=radius {
                        if x * x + y * y + z * z > radius * radius + 1 {
                            continue;
                        }
                        let (px, py, pz) = (cx + x, cy + y, cz + z);
                        if (0..16).contains(&px)
                            && (1..HEIGHT).contains(&py)
                            && (0..16).contains(&pz)
                            && get_world(sections, px, py, pz) == blocks.stone
                        {
                            set_world(sections, px, py, pz, ore);
                        }
                    }
                }
            }
        }
    }
}

fn place_trees(
    seed: i64,
    pos: ChunkPos,
    sections: &mut [ChunkBuilder],
    heights: &[[i32; 16]; 16],
    blocks: &OverworldBlocks,
) {
    for oz in pos.z as i64 - 1..=pos.z as i64 + 1 {
        for ox in pos.x as i64 - 1..=pos.x as i64 + 1 {
            let h = hash(seed, 0x7472656573000001, ox, 0, oz);
            if !h.is_multiple_of(5) {
                continue;
            }
            let wx = ox * 16 + ((h >> 8) % 16) as i64;
            let wz = oz * 16 + ((h >> 16) % 16) as i64;
            let (Ok(wx_i32), Ok(wz_i32)) = (i32::try_from(wx), i32::try_from(wz)) else {
                continue;
            };
            let x = (wx - pos.x as i64 * 16) as i32;
            let z = (wz - pos.z as i64 * 16) as i32;
            let surface = if (0..16).contains(&x) && (0..16).contains(&z) {
                heights[z as usize][x as usize]
            } else {
                terrain_height(seed, wx_i32, wz_i32)
            };
            if surface <= SEA_LEVEL + 1 || surface >= 84 {
                continue;
            }
            let trunk_height = 4 + ((h >> 24) % 3) as i32;
            for dy in 1..=trunk_height {
                if (0..16).contains(&x)
                    && (0..16).contains(&z)
                    && (0..HEIGHT).contains(&(surface + dy))
                {
                    set_world(sections, x, surface + dy, z, blocks.log);
                }
            }
            for dy in trunk_height - 2..=trunk_height + 1 {
                for dz in -2_i32..=2 {
                    for dx in -2_i32..=2 {
                        if dx.abs() == 2 && dz.abs() == 2 && dy > trunk_height {
                            continue;
                        }
                        let (lx, ly, lz) = (x + dx, surface + dy, z + dz);
                        if (0..16).contains(&lx)
                            && (0..HEIGHT).contains(&ly)
                            && (0..16).contains(&lz)
                            && get_world(sections, lx, ly, lz) == blocks.air
                        {
                            set_world(sections, lx, ly, lz, blocks.leaves);
                        }
                    }
                }
            }
        }
    }
}

fn value_noise_2d(seed: i64, x: i32, z: i32, scale: i32, domain: u64) -> f64 {
    let gx = x.div_euclid(scale);
    let gz = z.div_euclid(scale);
    let fx = smooth(x.rem_euclid(scale) as f64 / scale as f64);
    let fz = smooth(z.rem_euclid(scale) as f64 / scale as f64);
    let at = |dx, dz| {
        let value = hash(seed, domain, (gx + dx) as i64, 0, (gz + dz) as i64);
        (value as f64 / u64::MAX as f64) * 2.0 - 1.0
    };
    lerp(
        lerp(at(0, 0), at(1, 0), fx),
        lerp(at(0, 1), at(1, 1), fx),
        fz,
    )
}
fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
fn hash(seed: i64, domain: u64, x: i64, y: i64, z: i64) -> u64 {
    let mut v = seed as u64
        ^ domain
        ^ (x as u64).wrapping_mul(0x9e3779b185ebca87)
        ^ (y as u64).wrapping_mul(0xc2b2ae3d27d4eb4f)
        ^ (z as u64).wrapping_mul(0x165667b19e3779f9);
    v ^= v >> 30;
    v = v.wrapping_mul(0xbf58476d1ce4e5b9);
    v ^= v >> 27;
    v = v.wrapping_mul(0x94d049bb133111eb);
    v ^ (v >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn generator() -> MinecraftOverworldGenerator {
        MinecraftOverworldGenerator::new(OverworldBlocks {
            air: BlockId(0),
            stone: BlockId(1),
            grass: BlockId(2),
            dirt: BlockId(3),
            bedrock: BlockId(6),
            water: BlockId(16),
            sand: BlockId(7),
            log: BlockId(9),
            leaves: BlockId(10),
            coal_ore: BlockId(17),
            iron_ore: BlockId(18),
            gold_ore: BlockId(19),
            diamond_ore: BlockId(20),
        })
    }
    fn canonical(sections: &[(i32, Chunk)]) -> blake3::Hash {
        let mut hasher = blake3::Hasher::new();
        for (y, chunk) in sections {
            hasher.update(&y.to_le_bytes());
            for state in chunk.states() {
                hasher.update(&state.block.0.to_le_bytes());
                hasher.update(&state.variant.to_le_bytes());
            }
        }
        hasher.finalize()
    }
    #[test]
    fn generation_is_repeatable_and_cross_chunk_order_independent() {
        let generator = generator();
        let positions = [
            ChunkPos { x: -1, z: 0 },
            ChunkPos { x: 0, z: 0 },
            ChunkPos { x: 0, z: -1 },
        ];
        let a = positions.map(|p| canonical(&generator.generate(98_765, p).unwrap()));
        let mut b = positions
            .into_iter()
            .rev()
            .map(|p| canonical(&generator.generate(98_765, p).unwrap()))
            .collect::<Vec<_>>();
        b.reverse();
        assert_eq!(a.as_slice(), b.as_slice());
        assert_eq!(
            a[1],
            canonical(&generator.generate(98_765, positions[1]).unwrap())
        );
    }
    #[test]
    fn terrain_has_bedrock_surface_and_sea_water_policy() {
        let seed = (0..10_000)
            .find(|seed| terrain_height(*seed, 0, 0) < SEA_LEVEL)
            .unwrap();
        let sections = generator().generate(seed, ChunkPos { x: 0, z: 0 }).unwrap();
        assert_eq!(sections.len(), 8);
        assert!(sections[0].1.states().iter().any(|s| s.block == BlockId(6)));
        assert!(
            sections
                .iter()
                .flat_map(|(_, c)| c.states())
                .any(|s| s.block == BlockId(16))
        );
        assert!(
            sections
                .iter()
                .flat_map(|(_, chunk)| chunk.states())
                .filter(|state| state.block == BlockId(16))
                .all(|state| state.variant == WATER_SOURCE_VARIANT)
        );
    }

    #[test]
    fn bounded_worker_count_does_not_change_region_hash() {
        use rustcraft_world::GenerationScheduler;
        use std::{
            sync::Arc,
            time::{Duration, Instant},
        };
        let positions = (-1..=1)
            .flat_map(|z| (-1..=1).map(move |x| ChunkPos { x, z }))
            .collect::<Vec<_>>();
        let generate = |workers| {
            let mut scheduler = GenerationScheduler::new(workers, 9);
            let generator: Arc<dyn ChunkGenerator> = Arc::new(generator());
            for (i, pos) in positions.iter().copied().enumerate() {
                scheduler
                    .request(generator.clone(), -17, pos, i as u64 + 1)
                    .unwrap();
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut done = Vec::new();
            while done.len() < positions.len() && Instant::now() < deadline {
                done.extend(scheduler.take_ready());
                std::thread::yield_now();
            }
            assert_eq!(done.len(), positions.len());
            done.sort_by_key(|column| (column.position.x, column.position.z));
            let mut hasher = blake3::Hasher::new();
            for column in done {
                hasher.update(&column.position.x.to_le_bytes());
                hasher.update(&column.position.z.to_le_bytes());
                hasher.update(canonical(&column.sections.unwrap()).as_bytes());
            }
            *hasher.finalize().as_bytes()
        };
        assert_eq!(generate(1), generate(4));
    }

    #[test]
    fn generator_v1_canonical_region_is_frozen() {
        let generator = generator();
        let mut hasher = blake3::Hasher::new();
        for x in -2..2 {
            for z in -2..2 {
                let position = ChunkPos { x, z };
                hasher.update(&x.to_le_bytes());
                hasher.update(&z.to_le_bytes());
                let mut sections = generator.generate(731_173, position).unwrap();
                sections.sort_by_key(|(y, _)| *y);
                for (y, chunk) in sections {
                    hasher.update(&y.to_le_bytes());
                    for state in chunk.states() {
                        hasher.update(&state.block.0.to_le_bytes());
                        hasher.update(&state.variant.to_le_bytes());
                    }
                }
            }
        }
        assert_eq!(
            hasher.finalize().to_hex().as_str(),
            "e0d1f83c16b281124b7a9c190f667d7eddaa8b2f35ef5ef98ccb4bab434c1bb6"
        );
    }
}
