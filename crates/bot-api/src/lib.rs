//! Versionable semantic Bot API concepts. No transport is chosen at bootstrap.

use rustcraft_agent_api::{AgentIntent, Controller};
use rustcraft_engine_core::{BlockPos, Vec3};

pub const BOT_API_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackObservation {
    pub item_key: String,
    pub count: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemObservation {
    pub key: String,
    pub max_stack: u16,
    pub placeable_block: Option<String>,
    pub capabilities: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockObservation {
    pub key: String,
    pub solid: bool,
    pub opaque: bool,
    pub breakable: bool,
    pub emission: u8,
    pub sky_opacity: u8,
    pub light_opacity: u8,
}

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
    pub inventory: Vec<Option<StackObservation>>,
    pub selected_hotbar: u8,
    pub items: Vec<ItemObservation>,
    pub blocks: Vec<BlockObservation>,
    pub nearby_items: Vec<ItemEntityObservation>,
    pub mining_progress: Option<f32>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ItemEntityObservation {
    pub item_key: String,
    pub count: u16,
    pub position: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BotAction {
    pub intent: AgentIntent,
}

#[derive(Debug, Clone)]
pub struct ScriptedBot {
    intents: Vec<AgentIntent>,
    cursor: usize,
}

impl ScriptedBot {
    #[must_use]
    pub fn new(intents: impl Into<Vec<AgentIntent>>) -> Self {
        Self {
            intents: intents.into(),
            cursor: 0,
        }
    }
}

impl Controller for ScriptedBot {
    fn next_intent(&mut self) -> AgentIntent {
        let intent = self.intents.get(self.cursor).copied().unwrap_or_default();
        self.cursor = self.cursor.saturating_add(1);
        intent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scripted_controller_emits_semantic_intent_in_order() {
        let intents = vec![
            AgentIntent {
                jump: true,
                ..Default::default()
            },
            AgentIntent::default(),
        ];
        let mut bot = ScriptedBot::new(intents);
        assert!(bot.next_intent().jump);
        assert_eq!(bot.next_intent(), AgentIntent::default());
    }
}
