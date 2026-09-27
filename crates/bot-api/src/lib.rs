//! Versionable semantic Bot API concepts. No transport is chosen at bootstrap.

use rustcraft_agent_api::AgentIntent;
use rustcraft_engine_core::{BlockPos, Vec3};

pub const BOT_API_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct SelfObservation {
    pub position: Vec3,
    pub velocity: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NearbyBlockObservation {
    pub position: BlockPos,
    pub block_key: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub api_version: u32,
    pub self_state: SelfObservation,
    pub nearby_blocks: Vec<NearbyBlockObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BotAction {
    pub intent: AgentIntent,
}
