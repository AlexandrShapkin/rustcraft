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
pub struct AgentIntent<G = ()> {
    pub movement: MoveIntent,
    pub look_delta: Vec3,
    pub jump: bool,
    pub crouch: bool,
    /// Generic held primary action. The active game decides what it means.
    pub primary_action: bool,
    /// Generic held/pressed secondary action. The active game decides what it means.
    pub secondary_action: bool,
    /// Typed game-specific adapter payload; generic controllers use ().
    pub game: G,
}

pub trait Controller<G = ()> {
    fn next_intent(&mut self) -> AgentIntent<G>;
}

impl<G> AgentIntent<G> {
    pub fn map_game<H>(self, map: impl FnOnce(G) -> H) -> AgentIntent<H> {
        AgentIntent {
            movement: self.movement,
            look_delta: self.look_delta,
            jump: self.jump,
            crouch: self.crouch,
            primary_action: self.primary_action,
            secondary_action: self.secondary_action,
            game: map(self.game),
        }
    }
}
