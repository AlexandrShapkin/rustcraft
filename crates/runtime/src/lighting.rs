//! Two packed channels in World; direct sky seeds plus deterministic local relaxation.
//! Removal converges because every propagated edge costs at least one light level.
use rustcraft_engine_core::{
    BlockPos, ChunkPos, SectionPos, VoxelLight, World, block_index, split_block,
};
use rustcraft_mod_api::BlockRegistry;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

pub fn neighbors(p: BlockPos) -> [BlockPos; 6] {
    [
        BlockPos { x: p.x + 1, ..p },
        BlockPos { x: p.x - 1, ..p },
        BlockPos { y: p.y + 1, ..p },
        BlockPos { y: p.y - 1, ..p },
        BlockPos { z: p.z + 1, ..p },
        BlockPos { z: p.z - 1, ..p },
    ]
}
pub fn section(p: BlockPos) -> SectionPos {
    (
        ChunkPos {
            x: p.x.div_euclid(16),
            z: p.z.div_euclid(16),
        },
        p.y.div_euclid(16),
    )
}
pub fn dirty_neighbors(dirty: &mut HashSet<SectionPos>, p: BlockPos) {
    dirty.insert(section(p));
    for n in neighbors(p) {
        dirty.insert(section(n));
    }
}

#[derive(Debug, Default)]
pub struct Lighting {
    direct: HashMap<SectionPos, Vec<u8>>,
    columns: HashMap<ChunkPos, (i32, i32)>,
    pub initial_ms: f64,
    pub last_update_ms: f64,
    pub last_visited: usize,
}
impl Lighting {
    pub fn initialize(world: &mut World, r: &BlockRegistry) -> Self {
        let started = Instant::now();
        let mut s = Self::default();
        for (c, y) in world.section_positions() {
            let b = s.columns.entry(c).or_insert((y, y));
            b.0 = b.0.min(y);
            b.1 = b.1.max(y);
        }
        // One sky section and one lower section per loaded column, including light-only air.
        for (&c, b) in &mut s.columns {
            b.0 -= 1;
            b.1 += 1;
            for y in b.0..=b.1 {
                s.direct.insert((c, y), vec![0; 4096]);
            }
        }
        let mut queue = VecDeque::new();
        let columns: Vec<_> = s.columns.keys().copied().collect();
        for c in columns {
            for z in 0..16 {
                for x in 0..16 {
                    s.seed_column(world, r, c.x * 16 + x, c.z * 16 + z, &mut queue);
                }
            }
        }
        let mut dirty = HashSet::new();
        s.propagate(world, r, queue, &mut dirty);
        s.initial_ms = started.elapsed().as_secs_f64() * 1000.;
        s
    }
    fn seed_column(
        &mut self,
        world: &World,
        r: &BlockRegistry,
        x: i32,
        z: i32,
        queue: &mut VecDeque<BlockPos>,
    ) {
        let c = ChunkPos {
            x: x.div_euclid(16),
            z: z.div_euclid(16),
        };
        let Some(&(low, high)) = self.columns.get(&c) else {
            return;
        };
        let mut sky = 15_u8;
        for y in (low * 16..=(high + 1) * 16 - 1).rev() {
            let p = BlockPos { x, y, z };
            let opacity = r.get(world.get(p)).map_or(0, |b| b.sky_opacity);
            sky = sky.saturating_sub(opacity);
            let (_, local) = split_block(p);
            let seed = &mut self.direct.get_mut(&section(p)).unwrap()[block_index(local)];
            if *seed != sky
                || world.light(p).sky() != sky
                || r.get(world.get(p)).is_some_and(|b| b.emission > 0)
            {
                *seed = sky;
                queue.push_back(p);
            }
        }
    }
    fn source(&self, p: BlockPos) -> Option<u8> {
        let (_, local) = split_block(p);
        self.direct.get(&section(p)).map(|v| v[block_index(local)])
    }
    fn propagate(
        &mut self,
        world: &mut World,
        r: &BlockRegistry,
        mut queue: VecDeque<BlockPos>,
        dirty: &mut HashSet<SectionPos>,
    ) {
        let mut queued: HashSet<_> = queue.iter().copied().collect();
        self.last_visited = 0;
        while let Some(p) = queue.pop_front() {
            queued.remove(&p);
            let Some(source) = self.source(p) else {
                continue;
            };
            self.last_visited += 1;
            let b = r.get(world.get(p));
            let sky_cost = b.map_or(1, |b| b.sky_opacity.max(1));
            let block_cost = b.map_or(1, |b| b.light_opacity.max(1));
            let mut sky = source;
            let mut block = b.map_or(0, |b| b.emission);
            for n in neighbors(p) {
                let light = world.light(n);
                sky = sky.max(light.sky().saturating_sub(sky_cost));
                block = block.max(light.block().saturating_sub(block_cost));
            }
            let new = VoxelLight::new(sky, block);
            if new != world.light(p) {
                world.set_light(p, new);
                dirty_neighbors(dirty, p);
                for n in neighbors(p) {
                    if self.source(n).is_some() && queued.insert(n) {
                        queue.push_back(n);
                    }
                }
            }
        }
    }
    pub fn update(
        &mut self,
        world: &mut World,
        r: &BlockRegistry,
        p: BlockPos,
        dirty: &mut HashSet<SectionPos>,
    ) {
        let started = Instant::now();
        let (c, y) = section(p);
        let b = self.columns.entry(c).or_insert((y - 1, y + 1));
        b.0 = b.0.min(y - 1);
        b.1 = b.1.max(y + 1);
        let mut added = false;
        for sy in b.0..=b.1 {
            if let std::collections::hash_map::Entry::Vacant(e) = self.direct.entry((c, sy)) {
                e.insert(vec![0; 4096]);
                added = true;
            }
        }
        let mut queue = VecDeque::new();
        if added {
            for z in 0..16 {
                for x in 0..16 {
                    self.seed_column(world, r, c.x * 16 + x, c.z * 16 + z, &mut queue);
                }
            }
        } else {
            self.seed_column(world, r, p.x, p.z, &mut queue);
        }
        queue.push_back(p);
        queue.extend(neighbors(p));
        self.propagate(world, r, queue, dirty);
        self.last_update_ms = started.elapsed().as_secs_f64() * 1000.;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_engine_core::BlockId;
    use rustcraft_mod_api::BlockDefinition;
    fn registry() -> BlockRegistry {
        let mut r = BlockRegistry::default();
        r.register(BlockDefinition::cube(1, "test:stone", "test:stone"))
            .unwrap();
        r.register(BlockDefinition {
            emission: 15,
            ..BlockDefinition::cube(2, "test:lamp", "test:lamp")
        })
        .unwrap();
        r
    }
    #[test]
    fn sky_emission_removal_and_signed_boundaries() {
        let r = registry();
        let mut w = World::new(BlockId(0));
        w.fill_box(
            BlockPos {
                x: -2,
                y: -2,
                z: -2,
            },
            BlockPos { x: 17, y: -2, z: 2 },
            BlockId(1),
        );
        let mut l = Lighting::initialize(&mut w, &r);
        assert_eq!(w.light(BlockPos { x: 0, y: 0, z: 0 }).sky(), 15);
        let p = BlockPos { x: 15, y: -1, z: 0 };
        let mut dirty = HashSet::new();
        w.set(p, BlockId(2));
        l.update(&mut w, &r, p, &mut dirty);
        assert_eq!(w.light(BlockPos { x: 16, ..p }).block(), 14);
        assert_eq!(w.light(BlockPos { y: 0, ..p }).block(), 14);
        assert!(dirty.contains(&(ChunkPos { x: 1, z: 0 }, -1)));
        w.set(p, BlockId(0));
        l.update(&mut w, &r, p, &mut dirty);
        assert_eq!(w.light(BlockPos { x: 16, ..p }).block(), 0);
        w.fill_box(
            BlockPos { x: -2, y: 2, z: -2 },
            BlockPos { x: 2, y: 2, z: 2 },
            BlockId(1),
        );
        for z in -2..=2 {
            for x in -2..=2 {
                l.update(&mut w, &r, BlockPos { x, y: 2, z }, &mut dirty);
            }
        }
        assert!(w.light(BlockPos { x: 0, y: 1, z: 0 }).sky() < 15);
        let p = BlockPos { x: 0, y: 2, z: 0 };
        w.set(p, BlockId(0));
        l.update(&mut w, &r, p, &mut dirty);
        assert_eq!(w.light(BlockPos { x: 0, y: 1, z: 0 }).sky(), 15);
    }
}

#[cfg(test)]
mod enclosure_tests {
    use super::*;
    use rustcraft_engine_core::BlockId;
    use rustcraft_mod_api::BlockDefinition;
    #[test]
    fn enclosed_sky_two_sources_and_removal_match_fresh_initialization() {
        let mut r = BlockRegistry::default();
        r.register(BlockDefinition::cube(1, "test:wall", "test:wall"))
            .unwrap();
        r.register(BlockDefinition {
            emission: 15,
            ..BlockDefinition::cube(2, "test:lamp", "test:lamp")
        })
        .unwrap();
        let mut w = World::new(BlockId(0));
        // Room crosses x=16 and y=16, so both channels must cross section seams.
        w.fill_box(
            BlockPos { x: 12, y: 12, z: 0 },
            BlockPos { x: 20, y: 20, z: 8 },
            BlockId(1),
        );
        w.fill_box(
            BlockPos { x: 13, y: 13, z: 1 },
            BlockPos { x: 19, y: 19, z: 7 },
            BlockId(0),
        );
        let mut light = Lighting::initialize(&mut w, &r);
        let center = BlockPos { x: 16, y: 16, z: 4 };
        assert_eq!(w.light(center), VoxelLight::new(0, 0));
        let a = BlockPos { x: 15, y: 15, z: 4 };
        let b = BlockPos { x: 18, y: 17, z: 4 };
        let mut dirty = HashSet::new();
        for p in [a, b] {
            w.set(p, BlockId(2));
            light.update(&mut w, &r, p, &mut dirty);
        }
        assert_eq!(w.light(center).block(), 13);
        w.set(a, BlockId(0));
        light.update(&mut w, &r, a, &mut dirty);
        assert_eq!(w.light(center).block(), 12);
        w.set(b, BlockId(0));
        light.update(&mut w, &r, b, &mut dirty);
        assert_eq!(w.light(center).block(), 0);
        let roof = BlockPos { y: 20, ..center };
        w.set(roof, BlockId(0));
        light.update(&mut w, &r, roof, &mut dirty);
        assert_eq!(w.light(center).sky(), 15);
        w.set(roof, BlockId(1));
        light.update(&mut w, &r, roof, &mut dirty);
        assert_eq!(w.light(center).sky(), 0);
        let mut fresh = World::new(BlockId(0));
        for y in 12..=20 {
            for z in 0..=8 {
                for x in 12..=20 {
                    let p = BlockPos { x, y, z };
                    fresh.set(p, w.get(p));
                }
            }
        }
        Lighting::initialize(&mut fresh, &r);
        for y in 12..=20 {
            for z in 0..=8 {
                for x in 12..=20 {
                    let p = BlockPos { x, y, z };
                    assert_eq!(w.light(p), fresh.light(p), "{p:?}");
                }
            }
        }
    }
}
