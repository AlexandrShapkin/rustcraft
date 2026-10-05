//! Semantic intent shared by humans, network players, bots, replays and tests.

use rustcraft_engine_core::{BlockPos, Vec3};
use rustcraft_game_api::BlockKey;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveIntent {
    pub forward: f32,
    pub strafe: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceIntent {
    pub position: BlockPos,
    pub block: BlockKey,
}

impl Default for MoveIntent {
    fn default() -> Self {
        Self {
            forward: 0.0,
            strafe: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AgentIntent {
    pub movement: MoveIntent,
    pub look_delta: Vec3,
    pub jump: bool,
    pub crouch: bool,
    /// Generic held primary action. The active game decides what it means.
    pub primary_action: bool,
    /// Generic held/pressed secondary action. The active game decides what it means.
    pub secondary_action: bool,
    /// Legacy M0-M3 Minecraft action aliases retained during incremental migration.
    pub attack: bool,
    pub use_action: bool,
    pub select_hotbar: Option<u8>,
    pub scroll_hotbar: i8,
    pub break_block: Option<BlockPos>,
    pub place_block: Option<PlaceIntent>,
    pub craft: bool,
    pub inventory_click: Option<(u8, u8)>,
}

pub trait Controller {
    fn next_intent(&mut self) -> AgentIntent;
}
