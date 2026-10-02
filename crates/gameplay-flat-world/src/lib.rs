//! Deterministic first-party flat world provider.

use rustcraft_engine_core::{BlockId, BlockPos, BlockState, Chunk, ChunkBuilder, ChunkPos, World};
use rustcraft_mod_api::{BlockRegistry, GameplayModule, ModuleId, RegistrationError};
use rustcraft_world::{ChunkGenerator, WorldError};

#[derive(Debug, Clone, Copy)]
pub struct FlatChunkGenerator {
    pub floor_y: i32,
    pub dirt_depth: i32,
    pub air: BlockId,
    pub stone: BlockId,
    pub dirt: BlockId,
    pub grass: BlockId,
}

impl ChunkGenerator for FlatChunkGenerator {
    fn id(&self) -> &str {
        "rustcraft:flat"
    }
    fn version(&self) -> u32 {
        1
    }

    fn generate(&self, _seed: i64, _position: ChunkPos) -> Result<Vec<(i32, Chunk)>, WorldError> {
        let min_y = self
            .floor_y
            .saturating_sub(self.dirt_depth.max(0))
            .saturating_sub(1);
        let low_section = min_y.div_euclid(16);
        let high_section = self.floor_y.div_euclid(16);
        let mut sections = (low_section..=high_section)
            .map(|_| ChunkBuilder::new(BlockState::new(self.air)))
            .collect::<Vec<_>>();
        for y in min_y..=self.floor_y {
            let block = if y == self.floor_y {
                self.grass
            } else if y >= self.floor_y.saturating_sub(self.dirt_depth.max(0)) {
                self.dirt
            } else {
                self.stone
            };
            let section_index = (y.div_euclid(16) - low_section) as usize;
            let local_y = y.rem_euclid(16) as u8;
            for z in 0..16 {
                for x in 0..16 {
                    sections[section_index].set((x, local_y, z), BlockState::new(block));
                }
            }
        }
        Ok(sections
            .into_iter()
            .enumerate()
            .map(|(i, section)| (low_section + i as i32, section.finish()))
            .collect())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FlatWorldModule {
    pub floor_y: i32,
    pub dirt_depth: i32,
    pub stone: BlockId,
    pub dirt: BlockId,
    pub grass: BlockId,
}

impl FlatWorldModule {
    pub fn generate(&self, world: &mut World, min_x: i32, max_x: i32, min_z: i32, max_z: i32) {
        world.fill_box(
            BlockPos {
                x: min_x,
                y: self.floor_y - self.dirt_depth - 1,
                z: min_z,
            },
            BlockPos {
                x: max_x,
                y: self.floor_y - self.dirt_depth - 1,
                z: max_z,
            },
            self.stone,
        );
        if self.dirt_depth > 0 {
            world.fill_box(
                BlockPos {
                    x: min_x,
                    y: self.floor_y - self.dirt_depth,
                    z: min_z,
                },
                BlockPos {
                    x: max_x,
                    y: self.floor_y - 1,
                    z: max_z,
                },
                self.dirt,
            );
        }
        world.fill_box(
            BlockPos {
                x: min_x,
                y: self.floor_y,
                z: min_z,
            },
            BlockPos {
                x: max_x,
                y: self.floor_y,
                z: max_z,
            },
            self.grass,
        );
    }
}

impl GameplayModule for FlatWorldModule {
    fn id(&self) -> ModuleId {
        ModuleId("minecraft_b173:flat-world")
    }
    fn register(&self, _registry: &mut BlockRegistry) -> Result<(), RegistrationError> {
        Ok(())
    }
}

/// Small authored building area, not procedural terrain generation. Definitions are
/// resolved from the active profile; missing optional content is simply omitted.
pub fn decorate_sandbox(world: &mut World, registry: &BlockRegistry) {
    decorate_sandbox_at(world, registry, 0);
}

/// Same presentation sample placed relative to a generated world's surface elevation.
pub fn decorate_sandbox_at(world: &mut World, registry: &BlockRegistry, base_y: i32) {
    let mut place = |x, y, z, name: &str| {
        if let Some(block) = registry.by_name(name) {
            world.set(
                BlockPos {
                    x,
                    y: y + base_y,
                    z,
                },
                block.id,
            );
        }
    };
    place(0, 1, 4, "minecraft_b173:cobblestone");
    place(0, 2, 4, "minecraft_b173:cobblestone");
    // Open-front shelter: roof shadow, glass window and a removable interior lamp.
    for x in -4..=4 {
        for z in 7..=14 {
            place(x, 0, z, "minecraft_b173:planks");
            place(x, 4, z, "minecraft_b173:planks");
            for y in 1..=3 {
                if z == 14 || x == -4 || x == 4 {
                    let material = if y == 2 && z > 8 && z < 13 {
                        "minecraft_b173:glass"
                    } else {
                        "minecraft_b173:cobblestone"
                    };
                    place(x, y, z, material);
                }
            }
        }
    }
    place(0, 1, 13, "minecraft_b173:debug_lamp");
    place(-2, 1, 13, "minecraft_b173:bookshelf");
    place(2, 1, 13, "minecraft_b173:brick");
    // A small log/leaves form and sample building blocks beside the approach.
    for y in 1..=4 {
        place(-7, y, 6, "minecraft_b173:log");
    }
    for x in -9..=-5 {
        for z in 4..=8 {
            place(x, 5, z, "minecraft_b173:leaves");
        }
    }
    for (x, name) in [
        (3, "minecraft_b173:sand"),
        (5, "minecraft_b173:gravel"),
        (7, "minecraft_b173:sandstone"),
        (9, "minecraft_b173:bedrock"),
    ] {
        place(x, 1, 4, name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generates_deterministic_layers() {
        let mut world = World::new(BlockId(0));
        FlatWorldModule {
            floor_y: 0,
            dirt_depth: 2,
            stone: BlockId(1),
            dirt: BlockId(3),
            grass: BlockId(2),
        }
        .generate(&mut world, -16, 16, -16, 16);
        assert_eq!(world.get(BlockPos { x: 0, y: 0, z: 0 }), BlockId(2));
        assert_eq!(world.get(BlockPos { x: 0, y: -1, z: 0 }), BlockId(3));
        assert_eq!(world.get(BlockPos { x: 0, y: -3, z: 0 }), BlockId(1));
    }

    #[test]
    fn flat_generator_uses_generic_contract_and_signed_sections() {
        let generator = FlatChunkGenerator {
            floor_y: -1,
            dirt_depth: 2,
            air: BlockId(0),
            stone: BlockId(1),
            dirt: BlockId(3),
            grass: BlockId(2),
        };
        let sections = generator
            .generate(42, rustcraft_engine_core::ChunkPos { x: -3, z: 7 })
            .unwrap();
        assert_eq!(
            sections.iter().map(|(y, _)| *y).collect::<Vec<_>>(),
            vec![-1]
        );
        let section = &sections[0].1;
        assert_eq!(section.get((0, 15, 0)), BlockId(2));
        assert_eq!(section.get((0, 14, 0)), BlockId(3));
        assert_eq!(section.get((0, 12, 0)), BlockId(1));
    }
}
