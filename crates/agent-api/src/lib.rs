//! Semantic intent shared by humans, network players, bots, replays and tests.

use rustcraft_engine_core::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveIntent {
    pub forward: f32,
    pub strafe: f32,
}

impl Default for MoveIntent {
    fn default() -> Self {
        Self { forward: 0.0, strafe: 0.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AgentIntent {
    pub movement: MoveIntent,
    pub look_delta: Vec3,
    pub jump: bool,
    pub crouch: bool,
    pub attack: bool,
    pub use_action: bool,
}
