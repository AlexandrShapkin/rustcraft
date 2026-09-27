//! Bootstrap first-party block module.

use rustcraft_engine_core::BlockId;
use rustcraft_mod_api::{BlockDefinition, GameplayModule, ModuleId};

pub const AIR: BlockDefinition = BlockDefinition { id: BlockId(0), name: "core:air" };
pub const STONE: BlockDefinition = BlockDefinition { id: BlockId(1), name: "core:stone" };
pub const GRASS: BlockDefinition = BlockDefinition { id: BlockId(2), name: "core:grass" };
pub const DIRT: BlockDefinition = BlockDefinition { id: BlockId(3), name: "core:dirt" };

#[derive(Debug, Default)]
pub struct BlocksModule;

impl GameplayModule for BlocksModule {
    fn id(&self) -> ModuleId {
        ModuleId("core:blocks")
    }
}
