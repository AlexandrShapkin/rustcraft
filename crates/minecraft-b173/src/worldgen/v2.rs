use super::{
    HEIGHT, OVERWORLD_GENERATOR_ID, OVERWORLD_V2_VERSION, SEA_LEVEL, hash, lerp, set_world,
    set_world_state, smooth, water_source_state,
};
use rustcraft_engine_core::{BlockId, BlockState, Chunk, ChunkBuilder, ChunkPos, Vec3, World};
use rustcraft_world::{ChunkGenerator, WorldError};
use std::time::{Duration, Instant};

const CLIMATE_DOMAIN: u64 = 0x76325f636c696d61;
const MOISTURE_DOMAIN: u64 = 0x76325f6d6f697374;
const CONTINENT_DOMAIN: u64 = 0x76325f636f6e7469;
const EROSION_DOMAIN: u64 = 0x76325f65726f7369;
const HILLS_DOMAIN: u64 = 0x76325f68696c6c73;
const DETAIL_DOMAIN: u64 = 0x76325f6465746169;
const SURFACE_DOMAIN: u64 = 0x76325f7375726661;
const CAVE_DOMAIN: u64 = 0x76325f6361766573;
const ORE_DOMAIN: u64 = 0x76325f6f72657300;
const TREE_DOMAIN: u64 = 0x76325f7472656573;
const LAKE_DOMAIN: u64 = 0x76325f6c616b6573;
// Maximum center travel is 49 cells plus radius < 3.3: ceil(52.3 / 16) = 4.
// Every destination must evaluate all potentially contributing origins, including at corners.
const CAVE_ORIGIN_HALO: i64 = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Climate {
    pub temperature: f64,
    pub moisture: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Biome {
    Ocean,
    Beach,
    Plains,
    Forest,
    Desert,
    Hills,
}

impl Biome {
    pub const ALL: [Self; 6] = [
        Self::Ocean,
        Self::Beach,
        Self::Plains,
        Self::Forest,
        Self::Desert,
        Self::Hills,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Ocean => "ocean",
            Self::Beach => "beach",
            Self::Plains => "plains",
            Self::Forest => "forest",
            Self::Desert => "desert",
            Self::Hills => "hills",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct V2Blocks {
    air: BlockId,
    stone: BlockId,
    grass: BlockId,
    dirt: BlockId,
    bedrock: BlockId,
    water: BlockId,
    sand: BlockId,
    gravel: BlockId,
    log: BlockId,
    leaves: BlockId,
    coal_ore: BlockId,
    iron_ore: BlockId,
    gold_ore: BlockId,
    diamond_ore: BlockId,
}

impl V2Blocks {
    fn package() -> Self {
        use crate::blocks as b;
        Self {
            air: b::AIR.id,
            stone: b::STONE.id,
            grass: b::GRASS.id,
            dirt: b::DIRT.id,
            bedrock: b::BEDROCK.id,
            water: b::WATER.id,
            sand: b::SAND.id,
            gravel: b::GRAVEL.id,
            log: b::LOG.id,
            leaves: b::LEAVES.id,
            coal_ore: b::COAL_ORE.id,
            iron_ore: b::IRON_ORE.id,
            gold_ore: b::GOLD_ORE.id,
            diamond_ore: b::DIAMOND_ORE.id,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct V2GenerationMetrics {
    pub climate: Duration,
    pub terrain: Duration,
    pub surface: Duration,
    pub caves: Duration,
    pub ores: Duration,
    pub vegetation: Duration,
    pub structures: Duration,
    pub cave_blocks: u64,
    pub ore_blocks: [u64; 4],
    pub trees: u64,
    pub lakes: u64,
}

pub struct MinecraftOverworldGeneratorV2 {
    blocks: V2Blocks,
}

pub fn minecraft_overworld_v2() -> MinecraftOverworldGeneratorV2 {
    MinecraftOverworldGeneratorV2 {
        blocks: V2Blocks::package(),
    }
}

impl MinecraftOverworldGeneratorV2 {
    #[cfg(test)]
    fn with_blocks(blocks: V2Blocks) -> Self {
        Self { blocks }
    }

    pub fn climate(seed: i64, x: i32, z: i32) -> Climate {
        let temperature = (0.58
            + fbm(seed, x, z, 384, CLIMATE_DOMAIN) * 0.32
            + value_noise(seed, x, z, 96, CLIMATE_DOMAIN ^ 0x10) * 0.10)
            .clamp(0.0, 1.0);
        let moisture = (0.52
            + fbm(seed, x, z, 320, MOISTURE_DOMAIN) * 0.38
            + value_noise(seed, x, z, 80, MOISTURE_DOMAIN ^ 0x20) * 0.10)
            .clamp(0.0, 1.0);
        Climate {
            temperature,
            moisture,
        }
    }

    pub fn terrain_height(seed: i64, x: i32, z: i32) -> i32 {
        macro_height(seed, x, z)
    }

    pub fn biome(seed: i64, x: i32, z: i32) -> Biome {
        classify_biome(seed, x, z)
    }

    pub fn generate_with_metrics(
        &self,
        seed: i64,
        position: ChunkPos,
    ) -> Result<(Vec<(i32, Chunk)>, V2GenerationMetrics), WorldError> {
        let mut metrics = V2GenerationMetrics::default();
        let started = Instant::now();
        let origin_x = position
            .x
            .checked_mul(16)
            .ok_or(WorldError::InvalidData("world coordinate overflow"))?;
        let origin_z = position
            .z
            .checked_mul(16)
            .ok_or(WorldError::InvalidData("world coordinate overflow"))?;
        let mut heights = [[0_i32; 16]; 16];
        let mut biomes = [[Biome::Plains; 16]; 16];
        for z in 0..16_i32 {
            for x in 0..16_i32 {
                let wx = origin_x
                    .checked_add(x)
                    .ok_or(WorldError::InvalidData("world coordinate overflow"))?;
                let wz = origin_z
                    .checked_add(z)
                    .ok_or(WorldError::InvalidData("world coordinate overflow"))?;
                heights[z as usize][x as usize] = macro_height(seed, wx, wz);
                biomes[z as usize][x as usize] = classify_biome(seed, wx, wz);
            }
        }
        metrics.climate = started.elapsed();

        let started = Instant::now();
        let mut sections = (0..HEIGHT / 16)
            .map(|_| ChunkBuilder::new(BlockState::new(self.blocks.air)))
            .collect::<Vec<_>>();
        for z in 0..16_i32 {
            for x in 0..16_i32 {
                let wx = origin_x + x;
                let wz = origin_z + z;
                let height = heights[z as usize][x as usize];
                for y in 0..HEIGHT {
                    let block = if y <= bedrock_top(seed, wx, wz) {
                        self.blocks.bedrock
                    } else if y <= height {
                        self.blocks.stone
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
        metrics.terrain = started.elapsed();

        let started = Instant::now();
        replace_surface(
            seed,
            origin_x,
            origin_z,
            &heights,
            &biomes,
            &mut sections,
            &self.blocks,
        );
        metrics.surface = started.elapsed();

        let started = Instant::now();
        metrics.cave_blocks = carve_caves(seed, position, &mut sections, &self.blocks);
        metrics.caves = started.elapsed();

        let started = Instant::now();
        metrics.ore_blocks = place_ores(seed, position, &mut sections, &self.blocks);
        metrics.ores = started.elapsed();

        let started = Instant::now();
        metrics.lakes = place_lakes(seed, position, &mut sections, &self.blocks);
        metrics.structures = started.elapsed();

        let started = Instant::now();
        metrics.trees = place_trees(seed, position, &mut sections, &self.blocks);
        metrics.vegetation = started.elapsed();

        Ok((
            sections
                .into_iter()
                .enumerate()
                .map(|(y, section)| (y as i32, section.finish()))
                .collect(),
            metrics,
        ))
    }
}

impl ChunkGenerator for MinecraftOverworldGeneratorV2 {
    fn id(&self) -> &str {
        OVERWORLD_GENERATOR_ID
    }

    fn version(&self) -> u32 {
        OVERWORLD_V2_VERSION
    }

    fn generate(&self, seed: i64, position: ChunkPos) -> Result<Vec<(i32, Chunk)>, WorldError> {
        self.generate_with_metrics(seed, position)
            .map(|(sections, _)| sections)
    }
}

fn value_noise(seed: i64, x: i32, z: i32, scale: i32, domain: u64) -> f64 {
    let gx = x.div_euclid(scale);
    let gz = z.div_euclid(scale);
    let fx = smooth(x.rem_euclid(scale) as f64 / scale as f64);
    let fz = smooth(z.rem_euclid(scale) as f64 / scale as f64);
    let sample = |dx, dz| {
        let value = hash(seed, domain, i64::from(gx + dx), 0, i64::from(gz + dz));
        (value as f64 / u64::MAX as f64) * 2.0 - 1.0
    };
    lerp(
        lerp(sample(0, 0), sample(1, 0), fx),
        lerp(sample(0, 1), sample(1, 1), fx),
        fz,
    )
}

fn fbm(seed: i64, x: i32, z: i32, scale: i32, domain: u64) -> f64 {
    value_noise(seed, x, z, scale, domain) * 0.58
        + value_noise(seed, x, z, (scale / 2).max(1), domain ^ 0x9e37) * 0.28
        + value_noise(seed, x, z, (scale / 4).max(1), domain ^ 0xc2b2) * 0.14
}

fn macro_height(seed: i64, x: i32, z: i32) -> i32 {
    let continent = fbm(seed, x, z, 448, CONTINENT_DOMAIN);
    let erosion = fbm(seed, x, z, 192, EROSION_DOMAIN);
    let hills_mask = ((fbm(seed, x, z, 288, HILLS_DOMAIN) + 0.18) / 1.18).clamp(0.0, 1.0);
    let rolling = fbm(seed, x, z, 88, DETAIL_DOMAIN) * 6.5;
    let detail = value_noise(seed, x, z, 28, DETAIL_DOMAIN ^ 0x44) * 2.2;
    let coast = continent * 21.0;
    let hills = hills_mask * hills_mask * (7.0 + erosion.max(0.0) * 15.0);
    (62.0 + coast + rolling + detail + hills)
        .round()
        .clamp(34.0, 112.0) as i32
}

fn classify_biome(seed: i64, x: i32, z: i32) -> Biome {
    let height = macro_height(seed, x, z);
    if height <= SEA_LEVEL - 2 {
        return Biome::Ocean;
    }
    let coast = height <= SEA_LEVEL + 2
        && [-4, 0, 4].into_iter().any(|dx| {
            [-4, 0, 4].into_iter().any(|dz| {
                (dx != 0 || dz != 0)
                    && macro_height(seed, x.saturating_add(dx), z.saturating_add(dz)) <= SEA_LEVEL
            })
        });
    if coast {
        return Biome::Beach;
    }
    let climate = MinecraftOverworldGeneratorV2::climate(seed, x, z);
    let rugged = fbm(seed, x, z, 144, HILLS_DOMAIN ^ 0x55);
    if height >= 78 || rugged > 0.55 {
        Biome::Hills
    } else if climate.temperature > 0.58 && climate.moisture < 0.52 {
        Biome::Desert
    } else if climate.moisture > 0.62 {
        Biome::Forest
    } else {
        Biome::Plains
    }
}

fn bedrock_top(seed: i64, x: i32, z: i32) -> i32 {
    (hash(seed, SURFACE_DOMAIN ^ 0xbed0, i64::from(x), 0, i64::from(z)) % 5) as i32
}

fn replace_surface(
    seed: i64,
    origin_x: i32,
    origin_z: i32,
    heights: &[[i32; 16]; 16],
    biomes: &[[Biome; 16]; 16],
    sections: &mut [ChunkBuilder],
    blocks: &V2Blocks,
) {
    for z in 0..16_i32 {
        for x in 0..16_i32 {
            let wx = origin_x + x;
            let wz = origin_z + z;
            let height = heights[z as usize][x as usize];
            let biome = biomes[z as usize][x as usize];
            let depth =
                3 + (hash(seed, SURFACE_DOMAIN, i64::from(wx), 0, i64::from(wz)) % 3) as i32;
            let gravel_patch = value_noise(seed, wx, wz, 22, SURFACE_DOMAIN ^ 0x777) > 0.58;
            let steep = [(-2, 0), (2, 0), (0, -2), (0, 2)]
                .into_iter()
                .map(|(dx, dz)| {
                    (height - macro_height(seed, wx.saturating_add(dx), wz.saturating_add(dz)))
                        .abs()
                })
                .max()
                .unwrap_or(0)
                >= 5;
            let (top, filler) = match biome {
                Biome::Ocean if gravel_patch => (blocks.gravel, blocks.gravel),
                Biome::Ocean => (blocks.sand, blocks.sand),
                Biome::Beach if gravel_patch => (blocks.gravel, blocks.gravel),
                Biome::Beach | Biome::Desert => (blocks.sand, blocks.sand),
                Biome::Hills if steep || height >= 91 => (blocks.stone, blocks.stone),
                Biome::Plains | Biome::Forest | Biome::Hills => (blocks.grass, blocks.dirt),
            };
            for y in (height - depth + 1).max(1)..=height {
                set_world(sections, x, y, z, if y == height { top } else { filler });
            }
        }
    }
}

fn carve_caves(
    seed: i64,
    destination: ChunkPos,
    sections: &mut [ChunkBuilder],
    blocks: &V2Blocks,
) -> u64 {
    carve_caves_in_halo(seed, destination, sections, blocks, CAVE_ORIGIN_HALO)
}

fn carve_caves_in_halo(
    seed: i64,
    destination: ChunkPos,
    sections: &mut [ChunkBuilder],
    blocks: &V2Blocks,
    halo: i64,
) -> u64 {
    let mut carved = 0;
    for oz in i64::from(destination.z) - halo..=i64::from(destination.z) + halo {
        for ox in i64::from(destination.x) - halo..=i64::from(destination.x) + halo {
            let count_hash = hash(seed, CAVE_DOMAIN, ox, 0, oz);
            let count = if count_hash.is_multiple_of(7) {
                1 + ((count_hash >> 8) % 3) as i64
            } else {
                0
            };
            for index in 0..count {
                let h = hash(seed, CAVE_DOMAIN ^ 0x100, ox, index, oz);
                let start_x = ox * 16 + ((h >> 8) % 16) as i64;
                let start_z = oz * 16 + ((h >> 16) % 16) as i64;
                let start_y = 10.0 + ((h >> 24) % 55) as f64;
                let yaw = ((h >> 32) as u32 as f64 / u32::MAX as f64) * std::f64::consts::TAU;
                let pitch = (((h >> 48) % 2001) as f64 / 1000.0 - 1.0) * 0.18;
                let length = 22 + ((h >> 40) % 28) as i64;
                carved += carve_tunnel(
                    seed,
                    destination,
                    sections,
                    blocks,
                    (start_x as f64, start_y, start_z as f64),
                    yaw,
                    pitch,
                    length,
                    h,
                );
                if h.is_multiple_of(5) {
                    carved += carve_tunnel(
                        seed,
                        destination,
                        sections,
                        blocks,
                        (start_x as f64, start_y, start_z as f64),
                        yaw + 1.25,
                        pitch * 0.5,
                        length / 2,
                        h ^ 0x51,
                    );
                }
            }
        }
    }
    carved
}

#[allow(clippy::too_many_arguments)]
fn carve_tunnel(
    seed: i64,
    destination: ChunkPos,
    sections: &mut [ChunkBuilder],
    blocks: &V2Blocks,
    start: (f64, f64, f64),
    mut yaw: f64,
    mut pitch: f64,
    length: i64,
    feature_hash: u64,
) -> u64 {
    let mut carved = 0;
    let (mut x, mut y, mut z) = start;
    let destination_x = i64::from(destination.x) * 16;
    let destination_z = i64::from(destination.z) * 16;
    for step in 0..length {
        let radius = 1.25
            + ((std::f64::consts::PI * step as f64 / length as f64).sin() * 1.7)
            + ((feature_hash >> 56) % 5) as f64 * 0.08;
        x += yaw.cos() * pitch.cos();
        z += yaw.sin() * pitch.cos();
        y += pitch.sin();
        yaw += value_noise(
            seed,
            x.round() as i32,
            z.round() as i32,
            17,
            CAVE_DOMAIN ^ feature_hash,
        ) * 0.11;
        pitch = (pitch * 0.82
            + value_noise(
                seed,
                step as i32,
                feature_hash as i32,
                9,
                CAVE_DOMAIN ^ 0x222,
            ) * 0.07)
            .clamp(-0.32, 0.32);
        let min_x = (x - radius).floor() as i64;
        let max_x = (x + radius).ceil() as i64;
        let min_z = (z - radius).floor() as i64;
        let max_z = (z + radius).ceil() as i64;
        let min_y = (y - radius * 0.75).floor().max(2.0) as i32;
        let max_y = (y + radius * 0.75).ceil().min((HEIGHT - 7) as f64) as i32;
        for wz in min_z..=max_z {
            for wx in min_x..=max_x {
                let lx = wx - destination_x;
                let lz = wz - destination_z;
                if !(0..16).contains(&lx) || !(0..16).contains(&lz) {
                    continue;
                }
                for wy in min_y..=max_y {
                    let normalized = ((wx as f64 + 0.5 - x) / radius).powi(2)
                        + ((wy as f64 + 0.5 - y) / (radius * 0.75)).powi(2)
                        + ((wz as f64 + 0.5 - z) / radius).powi(2);
                    if normalized >= 1.0 {
                        continue;
                    }
                    // Beta caves stop when a candidate volume meets surface water. This
                    // deterministic approximation keeps ocean/shore caves sealed near sea level.
                    if wy >= SEA_LEVEL - 10
                        && macro_height(seed, wx as i32, wz as i32) <= SEA_LEVEL + 1
                    {
                        continue;
                    }
                    let current = super::get_world(sections, lx as i32, wy, lz as i32);
                    if matches!(current, b if b == blocks.stone || b == blocks.dirt || b == blocks.grass || b == blocks.sand || b == blocks.gravel)
                    {
                        set_world(sections, lx as i32, wy, lz as i32, blocks.air);
                        carved += 1;
                    }
                }
            }
        }
    }
    carved
}

fn place_ores(
    seed: i64,
    destination: ChunkPos,
    sections: &mut [ChunkBuilder],
    blocks: &V2Blocks,
) -> [u64; 4] {
    let policies = [
        (blocks.coal_ore, 12_i64, 5_i32, 1_i32, 120_i32),
        (blocks.iron_ore, 10, 4, 1, 64),
        (blocks.gold_ore, 2, 3, 1, 32),
        (blocks.diamond_ore, 1, 3, 1, 16),
    ];
    let mut counts = [0_u64; 4];
    for oz in i64::from(destination.z) - 1..=i64::from(destination.z) + 1 {
        for ox in i64::from(destination.x) - 1..=i64::from(destination.x) + 1 {
            for (kind, &(ore, attempts, nodes, min_y, max_y)) in policies.iter().enumerate() {
                for attempt in 0..attempts {
                    let h = hash(seed, ORE_DOMAIN ^ kind as u64, ox, attempt, oz);
                    let start_x = ox * 16 + ((h >> 8) % 16) as i64;
                    let start_z = oz * 16 + ((h >> 16) % 16) as i64;
                    let start_y = min_y + ((h >> 24) % (max_y - min_y) as u64) as i32;
                    let angle = ((h >> 36) as u32 as f64 / u32::MAX as f64) * std::f64::consts::TAU;
                    for node in 0..nodes {
                        let t = node as f64 / nodes.max(1) as f64;
                        let cx = start_x as f64 + angle.cos() * (t - 0.5) * nodes as f64;
                        let cz = start_z as f64 + angle.sin() * (t - 0.5) * nodes as f64;
                        let cy = start_y as f64 + ((h >> (node % 48)) & 3) as f64 - 1.5;
                        let radius = 0.6 + (std::f64::consts::PI * t).sin() * 0.4;
                        counts[kind] += replace_ore_ellipsoid(
                            destination,
                            sections,
                            blocks,
                            ore,
                            min_y,
                            max_y,
                            cx,
                            cy,
                            cz,
                            radius,
                        );
                    }
                }
            }
        }
    }
    counts
}

#[allow(clippy::too_many_arguments)]
fn replace_ore_ellipsoid(
    destination: ChunkPos,
    sections: &mut [ChunkBuilder],
    blocks: &V2Blocks,
    ore: BlockId,
    minimum_y: i32,
    maximum_y: i32,
    cx: f64,
    cy: f64,
    cz: f64,
    radius: f64,
) -> u64 {
    let mut replaced = 0;
    let base_x = i64::from(destination.x) * 16;
    let base_z = i64::from(destination.z) * 16;
    for wz in (cz - radius).floor() as i64..=(cz + radius).ceil() as i64 {
        for wx in (cx - radius).floor() as i64..=(cx + radius).ceil() as i64 {
            let lx = wx - base_x;
            let lz = wz - base_z;
            if !(0..16).contains(&lx) || !(0..16).contains(&lz) {
                continue;
            }
            for y in (cy - radius).floor().max(f64::from(minimum_y)) as i32
                ..=(cy + radius).ceil().min(f64::from(maximum_y - 1)) as i32
            {
                let d = ((wx as f64 + 0.5 - cx) / radius).powi(2)
                    + ((y as f64 + 0.5 - cy) / radius).powi(2)
                    + ((wz as f64 + 0.5 - cz) / radius).powi(2);
                if d < 1.0 && super::get_world(sections, lx as i32, y, lz as i32) == blocks.stone {
                    set_world(sections, lx as i32, y, lz as i32, ore);
                    replaced += 1;
                }
            }
        }
    }
    replaced
}

fn lake_descriptor(seed: i64, ox: i64, oz: i64) -> Option<(f64, f64, f64, f64)> {
    let h = hash(seed, LAKE_DOMAIN, ox, 0, oz);
    if !h.is_multiple_of(7) {
        return None;
    }
    let x = ox * 16 + ((h >> 8) % 16) as i64;
    let z = oz * 16 + ((h >> 16) % 16) as i64;
    let (Ok(ix), Ok(iz)) = (i32::try_from(x), i32::try_from(z)) else {
        return None;
    };
    let surface = macro_height(seed, ix, iz);
    if !(SEA_LEVEL + 3..=92).contains(&surface) || classify_biome(seed, ix, iz) == Biome::Desert {
        return None;
    }
    Some((
        x as f64,
        (surface - 2) as f64,
        z as f64,
        4.5 + ((h >> 28) % 25) as f64 / 10.0,
    ))
}

fn place_lakes(
    seed: i64,
    destination: ChunkPos,
    sections: &mut [ChunkBuilder],
    blocks: &V2Blocks,
) -> u64 {
    let mut origins = 0;
    let base_x = i64::from(destination.x) * 16;
    let base_z = i64::from(destination.z) * 16;
    for oz in i64::from(destination.z) - 1..=i64::from(destination.z) + 1 {
        for ox in i64::from(destination.x) - 1..=i64::from(destination.x) + 1 {
            let Some((cx, cy, cz, radius)) = lake_descriptor(seed, ox, oz) else {
                continue;
            };
            let mut touched = false;
            for wz in (cz - radius).floor() as i64..=(cz + radius).ceil() as i64 {
                for wx in (cx - radius).floor() as i64..=(cx + radius).ceil() as i64 {
                    let lx = wx - base_x;
                    let lz = wz - base_z;
                    if !(0..16).contains(&lx) || !(0..16).contains(&lz) {
                        continue;
                    }
                    for y in (cy - 3.0).floor().max(2.0) as i32
                        ..=(cy + 3.0).ceil().min((HEIGHT - 2) as f64) as i32
                    {
                        let d = ((wx as f64 + 0.5 - cx) / radius).powi(2)
                            + ((wz as f64 + 0.5 - cz) / (radius * 0.82)).powi(2)
                            + ((y as f64 + 0.5 - cy) / 3.0).powi(2);
                        if d >= 1.0 {
                            continue;
                        }
                        let block = if y <= cy.floor() as i32 {
                            blocks.water
                        } else {
                            blocks.air
                        };
                        if block == blocks.water {
                            set_world_state(
                                sections,
                                lx as i32,
                                y,
                                lz as i32,
                                water_source_state(block),
                            );
                        } else {
                            set_world(sections, lx as i32, y, lz as i32, block);
                        }
                        touched = true;
                    }
                }
            }
            origins += u64::from(touched);
        }
    }
    origins
}

fn place_trees(
    seed: i64,
    destination: ChunkPos,
    sections: &mut [ChunkBuilder],
    blocks: &V2Blocks,
) -> u64 {
    let mut trees = 0;
    let base_x = i64::from(destination.x) * 16;
    let base_z = i64::from(destination.z) * 16;
    for oz in i64::from(destination.z) - 1..=i64::from(destination.z) + 1 {
        for ox in i64::from(destination.x) - 1..=i64::from(destination.x) + 1 {
            for index in 0..6_i64 {
                let h = hash(seed, TREE_DOMAIN, ox, index, oz);
                let wx = ox * 16 + ((h >> 8) % 16) as i64;
                let wz = oz * 16 + ((h >> 16) % 16) as i64;
                let (Ok(ix), Ok(iz)) = (i32::try_from(wx), i32::try_from(wz)) else {
                    continue;
                };
                let biome = classify_biome(seed, ix, iz);
                let accepted = match biome {
                    Biome::Forest => !h.is_multiple_of(4),
                    Biome::Plains => h.is_multiple_of(11),
                    Biome::Hills => h.is_multiple_of(7),
                    Biome::Ocean | Biome::Beach | Biome::Desert => false,
                };
                if !accepted {
                    continue;
                }
                let surface = macro_height(seed, ix, iz);
                if surface <= SEA_LEVEL + 1 || surface >= HEIGHT - 9 {
                    continue;
                }
                if near_lake(seed, wx, wz) {
                    continue;
                }
                let trunk_height = 4 + ((h >> 24) % 3) as i32;
                let local_x = wx - base_x;
                let local_z = wz - base_z;
                let mut touched = false;
                for dy in 1..=trunk_height {
                    if (0..16).contains(&local_x) && (0..16).contains(&local_z) {
                        set_world(
                            sections,
                            local_x as i32,
                            surface + dy,
                            local_z as i32,
                            blocks.log,
                        );
                        touched = true;
                    }
                }
                for dy in trunk_height - 2..=trunk_height + 1 {
                    let radius: i32 = if dy >= trunk_height { 1 } else { 2 };
                    for dz in -radius..=radius {
                        for dx in -radius..=radius {
                            if dx.abs() == radius
                                && dz.abs() == radius
                                && hash(
                                    seed,
                                    TREE_DOMAIN ^ 0x55,
                                    wx + i64::from(dx),
                                    i64::from(dy),
                                    wz + i64::from(dz),
                                )
                                .is_multiple_of(2)
                            {
                                continue;
                            }
                            let lx = local_x + i64::from(dx);
                            let lz = local_z + i64::from(dz);
                            let y = surface + dy;
                            if (0..16).contains(&lx)
                                && (0..16).contains(&lz)
                                && super::get_world(sections, lx as i32, y, lz as i32) == blocks.air
                            {
                                set_world(sections, lx as i32, y, lz as i32, blocks.leaves);
                                touched = true;
                            }
                        }
                    }
                }
                trees += u64::from(touched);
            }
        }
    }
    trees
}

fn near_lake(seed: i64, x: i64, z: i64) -> bool {
    let chunk_x = x.div_euclid(16);
    let chunk_z = z.div_euclid(16);
    for oz in chunk_z - 1..=chunk_z + 1 {
        for ox in chunk_x - 1..=chunk_x + 1 {
            if let Some((cx, _, cz, radius)) = lake_descriptor(seed, ox, oz) {
                let dx = x as f64 - cx;
                let dz = z as f64 - cz;
                if dx * dx + dz * dz <= (radius + 2.0).powi(2) {
                    return true;
                }
            }
        }
    }
    false
}

/// Select a first-time Minecraft spawn from already authoritative terrain. Existing persisted
/// players never call this policy. The bounded search avoids water, foliage and unsupported air.
pub fn select_safe_spawn(world: &World) -> Option<Vec3> {
    let blocks = V2Blocks::package();
    let columns = world.column_positions().collect::<Vec<_>>();
    let min_x = columns.iter().map(|position| position.x).min()?;
    let max_x = columns.iter().map(|position| position.x).max()?;
    let min_z = columns.iter().map(|position| position.z).min()?;
    let max_z = columns.iter().map(|position| position.z).max()?;
    let center_x = (min_x + (max_x - min_x) / 2) * 16 + 8;
    let center_z = (min_z + (max_z - min_z) / 2) * 16 + 8;
    for radius in 0..=24_i32 {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if radius != 0 && dx.abs().max(dz.abs()) != radius {
                    continue;
                }
                let x = center_x + dx;
                let z = center_z + dz;
                for y in (1..HEIGHT - 3).rev() {
                    let support = world.get(rustcraft_engine_core::BlockPos { x, y, z });
                    if support == blocks.air {
                        continue;
                    }
                    let stable = matches!(support, b if b == blocks.grass || b == blocks.dirt || b == blocks.stone || b == blocks.sand || b == blocks.gravel);
                    if stable {
                        return Some(Vec3::new(x as f32 + 0.5, y as f32 + 2.0, z as f32 + 0.5));
                    }
                    break;
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks() -> V2Blocks {
        V2Blocks {
            air: BlockId(0),
            stone: BlockId(1),
            grass: BlockId(2),
            dirt: BlockId(3),
            bedrock: BlockId(6),
            water: BlockId(16),
            sand: BlockId(7),
            gravel: BlockId(8),
            log: BlockId(9),
            leaves: BlockId(10),
            coal_ore: BlockId(17),
            iron_ore: BlockId(18),
            gold_ore: BlockId(19),
            diamond_ore: BlockId(20),
        }
    }

    fn generator() -> MinecraftOverworldGeneratorV2 {
        MinecraftOverworldGeneratorV2::with_blocks(blocks())
    }

    fn hash_sections(sections: &[(i32, Chunk)]) -> blake3::Hash {
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
    fn cave_halo_includes_every_origin_that_can_reach_destination() {
        let blocks = blocks();
        for seed in 0..32 {
            for destination in [ChunkPos { x: 0, z: 0 }, ChunkPos { x: -1, z: -1 }] {
                let carved = |halo| {
                    let mut sections = (0..8)
                        .map(|_| ChunkBuilder::new(BlockState::new(blocks.stone)))
                        .collect::<Vec<_>>();
                    carve_caves_in_halo(seed, destination, &mut sections, &blocks, halo);
                    let sections = sections
                        .into_iter()
                        .enumerate()
                        .map(|(y, section)| (y as i32, section.finish()))
                        .collect::<Vec<_>>();
                    hash_sections(&sections)
                };
                assert_eq!(
                    carved(CAVE_ORIGIN_HALO),
                    carved(5),
                    "incomplete cave origins seed={seed} destination={destination:?}"
                );
            }
        }
    }

    #[test]
    fn same_column_and_negative_coordinates_are_deterministic() {
        let generator = generator();
        for position in [
            ChunkPos { x: 0, z: 0 },
            ChunkPos { x: -1, z: 0 },
            ChunkPos { x: 0, z: -1 },
            ChunkPos { x: -1, z: -1 },
            ChunkPos {
                x: -12_345,
                z: -54_321,
            },
        ] {
            let a = generator.generate(731_173, position).unwrap();
            let b = generator.generate(731_173, position).unwrap();
            assert_eq!(hash_sections(&a), hash_sections(&b));
        }
    }

    #[test]
    fn climate_height_and_biomes_are_continuous_and_diverse() {
        let seeds = [0, 1, -1, 731_173, i64::from(i32::MAX), -9_223_372_036];
        let mut biomes = std::collections::BTreeSet::new();
        let mut min_height = i32::MAX;
        let mut max_height = i32::MIN;
        for seed in seeds {
            for z in (-512..=512).step_by(16) {
                for x in (-512..=512).step_by(16) {
                    let height = macro_height(seed, x, z);
                    min_height = min_height.min(height);
                    max_height = max_height.max(height);
                    biomes.insert(classify_biome(seed, x, z));
                    assert!((height - macro_height(seed, x + 1, z)).abs() <= 5);
                    let a = MinecraftOverworldGeneratorV2::climate(seed, x, z);
                    let b = MinecraftOverworldGeneratorV2::climate(seed, x + 1, z);
                    assert!((a.temperature - b.temperature).abs() < 0.08);
                    assert!((a.moisture - b.moisture).abs() < 0.08);
                }
            }
        }
        assert!(
            min_height < SEA_LEVEL - 5,
            "no meaningful ocean: {min_height}"
        );
        assert!(
            max_height > SEA_LEVEL + 20,
            "no meaningful hills: {max_height}"
        );
        for biome in Biome::ALL {
            assert!(biomes.contains(&biome), "missing biome {}", biome.name());
        }
    }

    #[test]
    fn beaches_require_local_sea_context() {
        let mut beaches = 0;
        let mut inland_lowlands = 0;
        for seed in [0, 1, -1, 731_173, 8_675_309] {
            for z in (-768..=768).step_by(8) {
                for x in (-768..=768).step_by(8) {
                    let biome = classify_biome(seed, x, z);
                    let has_sea_neighbor = [-4, 0, 4].into_iter().any(|dx| {
                        [-4, 0, 4].into_iter().any(|dz| {
                            (dx != 0 || dz != 0) && macro_height(seed, x + dx, z + dz) <= SEA_LEVEL
                        })
                    });
                    if biome == Biome::Beach {
                        beaches += 1;
                        assert!(has_sea_neighbor);
                    } else if (SEA_LEVEL - 1..=SEA_LEVEL + 2).contains(&macro_height(seed, x, z))
                        && !has_sea_neighbor
                    {
                        inland_lowlands += 1;
                    }
                }
            }
        }
        assert!(beaches > 0);
        assert!(inland_lowlands > 0);
    }

    #[test]
    fn ore_depth_rarity_and_biome_tree_policies_are_structural() {
        let generator = generator();
        let blocks = blocks();
        let mut ore_counts = [0_u64; 4];
        let mut maximum_y = [0_i32; 4];
        let mut logs = 0_u64;
        for seed in [1, 731_173, 8_675_309] {
            for z in -2..=2 {
                for x in -2..=2 {
                    let position = ChunkPos { x, z };
                    let sections = generator.generate(seed, position).unwrap();
                    for (section_y, chunk) in sections {
                        for local_z in 0..16_u8 {
                            for local_y in 0..16_u8 {
                                for local_x in 0..16_u8 {
                                    let block = chunk.state((local_x, local_y, local_z)).block;
                                    let y = section_y * 16 + i32::from(local_y);
                                    for (index, ore) in [
                                        blocks.coal_ore,
                                        blocks.iron_ore,
                                        blocks.gold_ore,
                                        blocks.diamond_ore,
                                    ]
                                    .into_iter()
                                    .enumerate()
                                    {
                                        if block == ore {
                                            ore_counts[index] += 1;
                                            maximum_y[index] = maximum_y[index].max(y);
                                        }
                                    }
                                    if block == blocks.log {
                                        logs += 1;
                                        let world_x = x * 16 + i32::from(local_x);
                                        let world_z = z * 16 + i32::from(local_z);
                                        assert!(matches!(
                                            classify_biome(seed, world_x, world_z),
                                            Biome::Forest | Biome::Plains | Biome::Hills
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(ore_counts[0] > ore_counts[2] * 3);
        assert!(ore_counts[1] > ore_counts[2] * 3);
        assert!(ore_counts[2] > ore_counts[3]);
        assert!(maximum_y[0] < 120);
        assert!(maximum_y[1] < 64);
        assert!(maximum_y[2] < 32);
        assert!(maximum_y[3] < 16);
        assert!(logs > 0);
    }

    #[test]
    fn every_stage_can_cross_a_border_without_order_state() {
        let generator = generator();
        let positions = [
            ChunkPos { x: 0, z: 0 },
            ChunkPos { x: 1, z: 0 },
            ChunkPos { x: 0, z: 1 },
            ChunkPos { x: 1, z: 1 },
        ];
        let forward =
            positions.map(|position| hash_sections(&generator.generate(98_765, position).unwrap()));
        let mut reverse = positions
            .into_iter()
            .rev()
            .map(|position| hash_sections(&generator.generate(98_765, position).unwrap()))
            .collect::<Vec<_>>();
        reverse.reverse();
        assert_eq!(forward.as_slice(), reverse.as_slice());
    }

    #[test]
    fn request_order_and_worker_count_do_not_change_region() {
        use rustcraft_world::GenerationScheduler;
        use std::{sync::Arc, time::Duration};

        let positions = (-2..=2)
            .flat_map(|z| (-2..=2).map(move |x| ChunkPos { x, z }))
            .collect::<Vec<_>>();
        let generate = |workers: usize, order: u8| {
            let mut requested = positions.clone();
            if order == 1 {
                requested.reverse();
            } else if order == 2 {
                requested.sort_by_key(|position| {
                    hash(
                        99,
                        0x746573745f6f7264,
                        i64::from(position.x),
                        0,
                        i64::from(position.z),
                    )
                });
            }
            let mut scheduler = GenerationScheduler::new(workers, requested.len());
            let generator: Arc<dyn ChunkGenerator> = Arc::new(generator());
            for (token, position) in requested.into_iter().enumerate() {
                scheduler
                    .request(generator.clone(), -731_173, position, token as u64 + 1)
                    .unwrap();
            }
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut completed = Vec::new();
            while completed.len() < positions.len() && Instant::now() < deadline {
                completed.extend(scheduler.take_ready());
                std::thread::yield_now();
            }
            assert_eq!(completed.len(), positions.len());
            completed.sort_by_key(|column| (column.position.x, column.position.z));
            let mut hasher = blake3::Hasher::new();
            for column in completed {
                hasher.update(&column.position.x.to_le_bytes());
                hasher.update(&column.position.z.to_le_bytes());
                hasher.update(hash_sections(&column.sections.unwrap()).as_bytes());
            }
            hasher.finalize()
        };
        assert_eq!(generate(1, 0), generate(2, 1));
        assert_eq!(generate(1, 0), generate(4, 2));
    }

    #[test]
    fn generator_v2_canonical_region_is_frozen() {
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
                        super::super::update_semantic_state_hash(&mut hasher, *state);
                    }
                }
            }
        }
        assert_eq!(
            hasher.finalize().to_hex().as_str(),
            "4e134fa137fa5477fc3d28c9a610afd14019b0d0304246e43cd9940deb4684e7"
        );
    }

    fn state_at(sections: &[(i32, Chunk)], x: u8, y: i32, z: u8) -> BlockId {
        sections[(y / 16) as usize]
            .1
            .state((x, (y % 16) as u8, z))
            .block
    }

    #[test]
    fn cave_ore_tree_and_lake_features_materially_cross_cardinal_borders() {
        let generator = generator();
        let blocks = blocks();
        let ores = [
            blocks.coal_ore,
            blocks.iron_ore,
            blocks.gold_ore,
            blocks.diamond_ore,
        ];
        let mut cave_x = false;
        let mut cave_z = false;
        let mut ore_x = false;
        let mut ore_z = false;
        let mut tree_x = false;
        let mut tree_z = false;
        let mut tree_corner = false;
        let mut lake_x = false;
        let mut lake_z = false;
        for seed in 0..96_i64 {
            let a = generator.generate(seed, ChunkPos { x: 0, z: 0 }).unwrap();
            let east = generator.generate(seed, ChunkPos { x: 1, z: 0 }).unwrap();
            let south = generator.generate(seed, ChunkPos { x: 0, z: 1 }).unwrap();
            let southeast = generator.generate(seed, ChunkPos { x: 1, z: 1 }).unwrap();
            for offset in 0..16_u8 {
                for y in 3..HEIGHT - 4 {
                    let ax = state_at(&a, 15, y, offset);
                    let ex = state_at(&east, 0, y, offset);
                    let az = state_at(&a, offset, y, 15);
                    let sz = state_at(&south, offset, y, 0);
                    cave_x |= y < 58 && ax == blocks.air && ex == blocks.air;
                    cave_z |= y < 58 && az == blocks.air && sz == blocks.air;
                    ore_x |= ax == ex && ores.contains(&ax);
                    ore_z |= az == sz && ores.contains(&az);
                    tree_x |= matches!(ax, b if b == blocks.log || b == blocks.leaves)
                        && matches!(ex, b if b == blocks.log || b == blocks.leaves);
                    tree_z |= matches!(az, b if b == blocks.log || b == blocks.leaves)
                        && matches!(sz, b if b == blocks.log || b == blocks.leaves);
                    lake_x |= y > SEA_LEVEL && ax == blocks.water && ex == blocks.water;
                    lake_z |= y > SEA_LEVEL && az == blocks.water && sz == blocks.water;
                }
            }
            for y in SEA_LEVEL + 1..HEIGHT - 4 {
                let corners = [
                    state_at(&a, 15, y, 15),
                    state_at(&east, 0, y, 15),
                    state_at(&south, 15, y, 0),
                    state_at(&southeast, 0, y, 0),
                ];
                tree_corner |= corners
                    .iter()
                    .all(|block| *block == blocks.log || *block == blocks.leaves);
            }
            if cave_x
                && cave_z
                && ore_x
                && ore_z
                && tree_x
                && tree_z
                && tree_corner
                && lake_x
                && lake_z
            {
                break;
            }
        }
        assert!(cave_x && cave_z, "no cave crossed a cardinal border");
        assert!(ore_x && ore_z, "no ore vein crossed a cardinal border");
        assert!(tree_x && tree_z, "no tree crossed a cardinal border");
        assert!(tree_corner, "no tree canopy crossed a four-column corner");
        assert!(
            lake_x && lake_z,
            "no surface lake crossed a cardinal border"
        );
    }

    #[test]
    fn coordinate_overflow_is_a_controlled_error() {
        assert!(
            generator()
                .generate(0, ChunkPos { x: i32::MAX, z: 0 })
                .is_err()
        );
        assert!(
            generator()
                .generate(0, ChunkPos { x: 0, z: i32::MIN })
                .is_err()
        );
    }

    #[test]
    fn exact_version_resolution_never_falls_forward() {
        assert_eq!(
            super::super::resolve_overworld_generator(OVERWORLD_GENERATOR_ID, 1)
                .unwrap()
                .version(),
            1
        );
        assert_eq!(
            super::super::resolve_overworld_generator(OVERWORLD_GENERATOR_ID, 2)
                .unwrap()
                .version(),
            2
        );
        assert!(super::super::resolve_overworld_generator(OVERWORLD_GENERATOR_ID, 3).is_err());
        assert!(super::super::resolve_overworld_generator("other:world", 2).is_err());
    }

    #[test]
    fn multi_seed_spawn_search_finds_dry_supported_headroom() {
        let blocks = blocks();
        for seed in [0, 1, -1, 731_173, 8_675_309, -9_223_372_036] {
            let generator = generator();
            let mut world = World::new(blocks.air);
            let center = super::super::initial_spawn_column(seed, OVERWORLD_V2_VERSION);
            for z in -1..=1 {
                for x in -1..=1 {
                    let position = ChunkPos {
                        x: center.x + x,
                        z: center.z + z,
                    };
                    world
                        .publish_column(position, generator.generate(seed, position).unwrap())
                        .unwrap();
                }
            }
            let spawn = select_safe_spawn(&world).unwrap_or_else(|| {
                panic!("seed {seed} did not provide a safe spawn in the initial 3x3")
            });
            let x = spawn.x.floor() as i32;
            let feet_y = spawn.y.floor() as i32;
            let z = spawn.z.floor() as i32;
            let support = world.get(rustcraft_engine_core::BlockPos {
                x,
                y: feet_y - 2,
                z,
            });
            assert!(
                matches!(support, b if b == blocks.grass || b == blocks.dirt || b == blocks.stone || b == blocks.sand || b == blocks.gravel)
            );
            assert_eq!(
                world.get(rustcraft_engine_core::BlockPos {
                    x,
                    y: feet_y - 1,
                    z
                }),
                blocks.air
            );
            assert_eq!(
                world.get(rustcraft_engine_core::BlockPos { x, y: feet_y, z }),
                blocks.air
            );
        }
    }
}
