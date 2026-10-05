//! Versionable semantic Bot API concepts. No transport is chosen at bootstrap.

use rustcraft_agent_api::{AgentIntent, Controller};
use rustcraft_engine_core::{BlockPos, EntityId, Vec3};

pub const BOT_API_VERSION: u32 = 4;

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
pub struct Observation<G = ()> {
    pub api_version: u32,
    pub self_state: SelfObservation,
    pub nearby_blocks: Vec<NearbyBlockObservation>,
    pub items: Vec<ItemObservation>,
    pub blocks: Vec<BlockObservation>,
    pub nearby_items: Vec<ItemEntityObservation>,
    /// Last rejected semantic admission, if any; receiving it grants no mutation authority.
    pub last_action_error: Option<String>,
    pub game: G,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ItemEntityObservation {
    /// Authoritative durable identity; never an observation-list index.
    /// Movement/column migration retains it. Merges retire the consumed ID.
    pub id: EntityId,
    pub item_key: String,
    pub count: u16,
    pub position: Vec3,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BotAction<G = ()> {
    pub intent: AgentIntent<G>,
}

#[derive(Debug, Clone)]
pub struct ScriptedBot<G = ()> {
    intents: Vec<AgentIntent<G>>,
    cursor: usize,
}

impl<G> ScriptedBot<G> {
    #[must_use]
    pub fn new(intents: impl Into<Vec<AgentIntent<G>>>) -> Self {
        Self {
            intents: intents.into(),
            cursor: 0,
        }
    }
}

impl<G: Clone + Default> Controller<G> for ScriptedBot<G> {
    fn next_intent(&mut self) -> AgentIntent<G> {
        let intent = self.intents.get(self.cursor).cloned().unwrap_or_default();
        self.cursor = self.cursor.saturating_add(1);
        intent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scripted_controller_emits_semantic_intent_in_order() {
        let intents: Vec<AgentIntent> = vec![
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

/// Explicit Minecraft observation compatibility; not the universal Bot observation contract.
pub mod legacy {
    use super::StackObservation;
    #[derive(Debug, Clone, PartialEq)]
    pub struct MinecraftObservation {
        pub inventory: Vec<Option<StackObservation>>,
        pub selected_hotbar: u8,
        pub mining_progress: Option<f32>,
    }
}
