//! Transitional M0-M3 Minecraft content contracts.
//!
//! New engine/game extension points belong in `rustcraft-game-api`. This crate remains while
//! existing survival definitions migrate without changing accepted gameplay behavior.

use rustcraft_engine_core::{BlockId, ItemId};

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
}

impl BlockRegistry {
    pub fn register(&mut self, definition: BlockDefinition) -> Result<(), RegistrationError> {
        if definition.emission > 15 || definition.sky_opacity > 15 || definition.light_opacity > 15
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
