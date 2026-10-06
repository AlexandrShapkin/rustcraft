//! Transitional M0-M3 Minecraft content contracts.
//!
//! New engine/game extension points belong in `rustcraft-game-api`. This crate remains while
//! existing survival definitions migrate without changing accepted gameplay behavior.

use rustcraft_engine_core::{BlockId, ItemId};

/// Retained local M0-M3 definition adapter. Policy-rich fields here are not generic authored contracts.
pub mod legacy {
    use super::*;
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Material {
        Opaque,
        Cutout,
        Translucent,
        /// A static voxel liquid: non-solid, alpha blended and rendered with liquid surface geometry.
        Liquid,
        Invisible,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FaceTextures {
        All(&'static str),
        TopSideBottom {
            top: &'static str,
            side: &'static str,
            bottom: &'static str,
        },
        Faces([&'static str; 6]),
    }
    impl FaceTextures {
        /// +Z, -Z, +X, -X, +Y, -Y.
        pub fn face(self, index: usize) -> &'static str {
            match self {
                Self::All(id) => id,
                Self::TopSideBottom { top, side, bottom } => {
                    if index == 4 {
                        top
                    } else if index == 5 {
                        bottom
                    } else {
                        side
                    }
                }
                Self::Faces(ids) => ids[index],
            }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct ItemDefinition {
        pub id: ItemId,
        pub name: &'static str,
        pub max_stack: u16,
        pub placeable: Option<BlockId>,
        pub capabilities: &'static [&'static str],
        pub tool: Option<ToolDefinition>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ToolCategory {
        Pickaxe,
        Axe,
        Shovel,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub enum ToolTier {
        Wood,
        Stone,
    }
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct ToolDefinition {
        pub category: ToolCategory,
        pub tier: ToolTier,
        pub speed: f32,
        pub durability: u16,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ModuleId(pub &'static str);

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct BlockDefinition {
        pub id: BlockId,
        pub name: &'static str,
        pub solid: bool,
        /// Whether the normal player block raycast can target this block.
        pub targetable: bool,
        pub material: Material,
        pub textures: FaceTextures,
        pub base_model_rotation: rustcraft_engine_core::orientation::ModelRotation,
        pub orientation_property: rustcraft_engine_core::orientation::OrientationProperty,
        pub breakable: bool,
        pub item: Option<ItemId>,
        pub emission: u8,
        pub sky_opacity: u8,
        pub light_opacity: u8,
        pub hardness: f32,
        pub mining_material: &'static str,
        pub preferred_tool: Option<ToolCategory>,
        pub drop: Option<ItemId>,
    }

    impl BlockDefinition {
        pub const fn cube(id: u32, name: &'static str, texture: &'static str) -> Self {
            Self {
                id: BlockId(id),
                name,
                solid: true,
                targetable: true,
                material: Material::Opaque,
                textures: FaceTextures::All(texture),
                base_model_rotation: rustcraft_engine_core::orientation::ModelRotation::IDENTITY,
                orientation_property: rustcraft_engine_core::orientation::OrientationProperty::None,
                breakable: true,
                item: Some(ItemId(id)),
                emission: 0,
                sky_opacity: 15,
                light_opacity: 15,
                hardness: 1.0,
                mining_material: "stone",
                preferred_tool: None,
                drop: Some(ItemId(id)),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RegistrationError {
        DuplicateId(BlockId),
        DuplicateName,
        InvalidDefinition,
        DuplicateItem,
    }

    #[derive(Debug, Default, Clone)]
    pub struct BlockRegistry {
        definitions: Vec<BlockDefinition>,
        items: Vec<ItemDefinition>,
        work: Vec<rustcraft_game_api::WorkDefinition>,
        recipes: Vec<rustcraft_game_api::RecipeDefinition>,
        profile: Option<rustcraft_game_api::CompiledGameProfile>,
        profile_ids: std::collections::HashMap<BlockId, BlockId>,
    }

    impl BlockRegistry {
        pub fn bind_profile(
            &mut self,
            profile: rustcraft_game_api::CompiledGameProfile,
        ) -> Result<(), RegistrationError> {
            for block in &self.definitions {
                let key = rustcraft_game_api::BlockKey::parse(block.name)
                    .map_err(|_| RegistrationError::InvalidDefinition)?;
                if profile.block_id(&key).is_none() {
                    return Err(RegistrationError::InvalidDefinition);
                }
            }
            self.profile_ids = self
                .definitions
                .iter()
                .map(|block| {
                    let key = rustcraft_game_api::BlockKey::parse(block.name).unwrap();
                    (block.id, profile.block_id(&key).unwrap())
                })
                .collect();
            self.profile = Some(profile);
            Ok(())
        }
        /// Semantic admission through the authoritative profile, followed by retained local translation.
        pub fn resolve_block_key(&self, key: &rustcraft_game_api::BlockKey) -> Option<BlockId> {
            let name = if let Some(profile) = &self.profile {
                let handle = profile.block_id(key)?;
                profile.block(handle)?.key.as_str()
            } else {
                key.as_str()
            };
            self.by_name(name).map(|d| d.id)
        }
        pub fn resolve_item_key(&self, key: &rustcraft_content::ResourceId) -> Option<ItemId> {
            self.items
                .iter()
                .find(|item| item.name == key.as_str())
                .map(|item| item.id)
        }
        pub fn profile(&self) -> Option<&rustcraft_game_api::CompiledGameProfile> {
            self.profile.as_ref()
        }
        pub fn register_work(
            &mut self,
            work: rustcraft_game_api::WorkDefinition,
        ) -> Result<(), RegistrationError> {
            if self.by_name(work.target.as_str()).is_none()
                || !work.duration_seconds.is_finite()
                || work.duration_seconds <= 0.
                || work.item_multipliers.iter().any(|(key, rate)| {
                    !rate.is_finite()
                        || *rate <= 0.
                        || self.items.iter().all(|i| i.name != key.as_str())
                })
                || work.reward.as_ref().is_some_and(|(key, count)| {
                    *count == 0 || self.items.iter().all(|i| i.name != key.as_str())
                })
                || self.work.iter().any(|w| w.target == work.target)
            {
                return Err(RegistrationError::InvalidDefinition);
            }
            self.work.push(work);
            Ok(())
        }
        pub fn work(&self) -> &[rustcraft_game_api::WorkDefinition] {
            &self.work
        }
        pub fn register_recipe(
            &mut self,
            recipe: rustcraft_game_api::RecipeDefinition,
        ) -> Result<(), RegistrationError> {
            if recipe.count == 0
                || recipe.width == 0
                || recipe.width > 3
                || recipe.inputs.len() > 9
                || self.items.iter().all(|i| i.name != recipe.output.as_str())
                || recipe
                    .inputs
                    .iter()
                    .flatten()
                    .any(|key| self.items.iter().all(|i| i.name != key.as_str()))
                || self.recipes.iter().any(|r| r.key == recipe.key)
            {
                return Err(RegistrationError::InvalidDefinition);
            }
            self.recipes.push(recipe);
            Ok(())
        }
        pub fn recipes(&self) -> &[rustcraft_game_api::RecipeDefinition] {
            &self.recipes
        }
        pub fn register(&mut self, definition: BlockDefinition) -> Result<(), RegistrationError> {
            if definition.emission > 15
                || definition.sky_opacity > 15
                || definition.light_opacity > 15
            {
                return Err(RegistrationError::InvalidDefinition);
            }
            if self.definitions.iter().any(|d| d.id == definition.id) {
                return Err(RegistrationError::DuplicateId(definition.id));
            }
            if self.definitions.iter().any(|d| d.name == definition.name) {
                return Err(RegistrationError::DuplicateName);
            }
            self.definitions.push(definition);
            self.definitions.sort_by_key(|d| d.id);
            Ok(())
        }
        #[must_use]
        pub fn get(&self, id: BlockId) -> Option<BlockDefinition> {
            self.definitions.iter().copied().find(|d| d.id == id)
        }
        #[must_use]
        pub fn overlaps_state(
            &self,
            state: rustcraft_engine_core::BlockState,
            bounds: rustcraft_engine_core::shape::LocalBox,
        ) -> bool {
            if let Some(profile) = &self.profile
                && let Some(id) = self.profile_ids.get(&state.block)
            {
                return profile
                    .overlaps_state(
                        rustcraft_engine_core::BlockState {
                            block: *id,
                            ..state
                        },
                        bounds,
                    )
                    .unwrap_or(true);
            }
            self.is_solid(state.block)
                && rustcraft_engine_core::shape::box_overlap(
                    bounds,
                    rustcraft_engine_core::shape::LocalBox::UNIT,
                )
        }
        pub fn selection_hit(
            &self,
            state: rustcraft_engine_core::BlockState,
            origin: [f32; 3],
            direction: [f32; 3],
            reach: f32,
        ) -> Option<rustcraft_engine_core::shape::ShapeHit> {
            if let Some(profile) = &self.profile
                && let Some(id) = self.profile_ids.get(&state.block)
            {
                return profile
                    .selection_hit(
                        rustcraft_engine_core::BlockState {
                            block: *id,
                            ..state
                        },
                        origin,
                        direction,
                        reach,
                    )
                    .ok()
                    .flatten();
            }
            if self
                .get(state.block)
                .is_some_and(|d| d.targetable && d.material != Material::Invisible)
            {
                rustcraft_engine_core::shape::Shape::FullCube.ray(
                    origin,
                    direction,
                    rustcraft_engine_core::orientation::ModelRotation::IDENTITY,
                    reach,
                )
            } else {
                None
            }
        }
        pub fn get_state(
            &self,
            state: rustcraft_engine_core::BlockState,
        ) -> Option<BlockDefinition> {
            let mut definition = self.get(state.block)?;
            if let Some(profile) = &self.profile
                && let Some(id) = self.profile_ids.get(&state.block)
            {
                let state = rustcraft_engine_core::BlockState {
                    block: *id,
                    ..state
                };
                if let Ok(light) = profile.light_for_state(state) {
                    definition.sky_opacity = light.sky_opacity;
                    definition.light_opacity = light.block_opacity;
                }
            }
            Some(definition)
        }
        #[must_use]
        pub fn is_solid(&self, id: BlockId) -> bool {
            self.get(id).is_some_and(|d| d.solid)
        }
        #[must_use]
        pub fn definitions(&self) -> &[BlockDefinition] {
            &self.definitions
        }
        pub fn by_name(&self, name: &str) -> Option<BlockDefinition> {
            self.definitions.iter().copied().find(|d| d.name == name)
        }
        pub fn register_item(&mut self, item: ItemDefinition) -> Result<(), RegistrationError> {
            if item.max_stack == 0 || item.placeable.is_some_and(|b| self.get(b).is_none()) {
                return Err(RegistrationError::InvalidDefinition);
            }
            if self
                .items
                .iter()
                .any(|i| i.id == item.id || i.name == item.name)
            {
                return Err(RegistrationError::DuplicateItem);
            }
            self.items.push(item);
            Ok(())
        }
        pub fn item(&self, id: ItemId) -> Option<&ItemDefinition> {
            self.items.iter().find(|i| i.id == id)
        }
        pub fn items(&self) -> &[ItemDefinition] {
            &self.items
        }
    }

    pub trait GameplayModule {
        fn id(&self) -> ModuleId;
        fn register(&self, registry: &mut BlockRegistry) -> Result<(), RegistrationError>;
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn registration_rejects_duplicate_ids() {
            let mut registry = BlockRegistry::default();
            let block = BlockDefinition::cube(1, "test:block", "test:texture");
            assert!(registry.register(block).is_ok());
            assert_eq!(
                registry.register(BlockDefinition {
                    name: "other",
                    ..block
                }),
                Err(RegistrationError::DuplicateId(BlockId(1)))
            );
        }
    }
}
// Source compatibility for untouched renderer/world/storage adapters.
pub use legacy::*;
pub use rustcraft_game_api::{RecipeDefinition, VoxelDefinition, WorkDefinition};

/// Explicit local M0-M3 Minecraft controller compatibility, not universal Agent semantics.
pub mod legacy_actions {
    use rustcraft_agent_api::{AgentIntent, PlaceIntent};
    use rustcraft_engine_core::BlockPos;
    #[derive(Debug, Clone, PartialEq, Default)]
    pub struct MinecraftActions {
        pub attack: bool,
        pub use_action: bool,
        pub select_hotbar: Option<u8>,
        pub scroll_hotbar: i8,
        pub break_block: Option<BlockPos>,
        pub place_block: Option<PlaceIntent>,
        pub craft: bool,
        pub inventory_click: Option<(u8, u8)>,
    }
    impl From<()> for MinecraftActions {
        fn from(_: ()) -> Self {
            Self::default()
        }
    }
    pub type PlayerIntent = AgentIntent<MinecraftActions>;
}
