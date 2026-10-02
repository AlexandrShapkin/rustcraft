//! First-party definitions. Historical atlas locations stay here.
use rustcraft_engine_core::BlockId;
use rustcraft_mod_api::{
    BlockDefinition, BlockRegistry, FaceTextures, GameplayModule, ItemDefinition, Material,
    ModuleId, RegistrationError, ToolCategory, ToolDefinition, ToolTier,
};
pub const AIR: BlockDefinition = BlockDefinition {
    solid: false,
    targetable: false,
    material: Material::Invisible,
    item: None,
    breakable: false,
    sky_opacity: 0,
    light_opacity: 0,
    ..BlockDefinition::cube(0, "minecraft_b173:air", "minecraft_b173:air")
};
pub const STONE: BlockDefinition = BlockDefinition {
    hardness: 1.5,
    preferred_tool: Some(ToolCategory::Pickaxe),
    drop: Some(rustcraft_engine_core::ItemId(4)),
    ..BlockDefinition::cube(1, "minecraft_b173:stone", "minecraft_b173:stone")
};
pub const GRASS: BlockDefinition = BlockDefinition {
    drop: Some(rustcraft_engine_core::ItemId(3)),
    textures: FaceTextures::TopSideBottom {
        top: "minecraft_b173:grass_top",
        side: "minecraft_b173:grass_side",
        bottom: "minecraft_b173:dirt",
    },
    ..BlockDefinition::cube(2, "minecraft_b173:grass", "minecraft_b173:grass_top")
};
pub const DIRT: BlockDefinition = BlockDefinition {
    hardness: 0.5,
    mining_material: "soil",
    preferred_tool: Some(ToolCategory::Shovel),
    ..BlockDefinition::cube(3, "minecraft_b173:dirt", "minecraft_b173:dirt")
};
pub const COBBLESTONE: BlockDefinition = BlockDefinition::cube(
    4,
    "minecraft_b173:cobblestone",
    "minecraft_b173:cobblestone",
);
pub const PLANKS: BlockDefinition = BlockDefinition {
    hardness: 2.0,
    mining_material: "wood",
    preferred_tool: Some(ToolCategory::Axe),
    ..BlockDefinition::cube(5, "minecraft_b173:planks", "minecraft_b173:planks")
};
pub const BEDROCK: BlockDefinition = BlockDefinition {
    breakable: false,
    ..BlockDefinition::cube(6, "minecraft_b173:bedrock", "minecraft_b173:bedrock")
};
pub const SAND: BlockDefinition =
    BlockDefinition::cube(7, "minecraft_b173:sand", "minecraft_b173:sand");
pub const GRAVEL: BlockDefinition =
    BlockDefinition::cube(8, "minecraft_b173:gravel", "minecraft_b173:gravel");
pub const LOG: BlockDefinition = BlockDefinition {
    hardness: 2.0,
    mining_material: "wood",
    preferred_tool: Some(ToolCategory::Axe),
    textures: FaceTextures::TopSideBottom {
        top: "minecraft_b173:log_top",
        side: "minecraft_b173:log_side",
        bottom: "minecraft_b173:log_top",
    },
    ..BlockDefinition::cube(9, "minecraft_b173:log", "minecraft_b173:log_side")
};
pub const LEAVES: BlockDefinition = BlockDefinition {
    material: Material::Cutout,
    sky_opacity: 1,
    light_opacity: 1,
    ..BlockDefinition::cube(10, "minecraft_b173:leaves", "minecraft_b173:leaves")
};
pub const GLASS: BlockDefinition = BlockDefinition {
    drop: None,
    material: Material::Translucent,
    sky_opacity: 0,
    light_opacity: 0,
    ..BlockDefinition::cube(11, "minecraft_b173:glass", "minecraft_b173:glass")
};
pub const BRICK: BlockDefinition =
    BlockDefinition::cube(12, "minecraft_b173:brick", "minecraft_b173:brick");
pub const BOOKSHELF: BlockDefinition = BlockDefinition {
    textures: FaceTextures::TopSideBottom {
        top: "minecraft_b173:planks",
        side: "minecraft_b173:bookshelf",
        bottom: "minecraft_b173:planks",
    },
    ..BlockDefinition::cube(13, "minecraft_b173:bookshelf", "minecraft_b173:bookshelf")
};
pub const SANDSTONE: BlockDefinition = BlockDefinition {
    textures: FaceTextures::TopSideBottom {
        top: "minecraft_b173:sandstone_top",
        side: "minecraft_b173:sandstone_side",
        bottom: "minecraft_b173:sandstone_bottom",
    },
    ..BlockDefinition::cube(
        14,
        "minecraft_b173:sandstone",
        "minecraft_b173:sandstone_side",
    )
};
// Temporary full emissive voxel, deliberately not a torch model.
pub const LAMP: BlockDefinition = BlockDefinition {
    emission: 15,
    ..BlockDefinition::cube(15, "minecraft_b173:debug_lamp", "minecraft_b173:lamp")
};
pub const WATER: BlockDefinition = BlockDefinition {
    solid: false,
    targetable: false,
    material: Material::Liquid,
    sky_opacity: 1,
    light_opacity: 1,
    ..BlockDefinition::cube(16, "minecraft_b173:water", "minecraft_b173:water")
};
pub const COAL_ORE: BlockDefinition =
    BlockDefinition::cube(17, "minecraft_b173:coal_ore", "minecraft_b173:coal_ore");
pub const IRON_ORE: BlockDefinition =
    BlockDefinition::cube(18, "minecraft_b173:iron_ore", "minecraft_b173:iron_ore");
pub const GOLD_ORE: BlockDefinition =
    BlockDefinition::cube(19, "minecraft_b173:gold_ore", "minecraft_b173:gold_ore");
pub const DIAMOND_ORE: BlockDefinition = BlockDefinition::cube(
    20,
    "minecraft_b173:diamond_ore",
    "minecraft_b173:diamond_ore",
);
pub const STICK_ITEM: ItemDefinition = ItemDefinition {
    id: rustcraft_engine_core::ItemId(100),
    name: "minecraft_b173:stick",
    max_stack: 64,
    placeable: None,
    capabilities: &[],
    tool: None,
};
pub const WOOD_PICKAXE: ItemDefinition = ItemDefinition {
    id: rustcraft_engine_core::ItemId(101),
    name: "minecraft_b173:wood_pickaxe",
    max_stack: 1,
    placeable: None,
    capabilities: &["minecraft_b173:capability/tool"],
    tool: Some(ToolDefinition {
        category: ToolCategory::Pickaxe,
        tier: ToolTier::Wood,
        speed: 2.0,
        durability: 59,
    }),
};
pub const WOOD_AXE: ItemDefinition = ItemDefinition {
    id: rustcraft_engine_core::ItemId(102),
    name: "minecraft_b173:wood_axe",
    max_stack: 1,
    placeable: None,
    capabilities: &["minecraft_b173:capability/tool"],
    tool: Some(ToolDefinition {
        category: ToolCategory::Axe,
        tier: ToolTier::Wood,
        speed: 2.0,
        durability: 59,
    }),
};
pub const WOOD_SHOVEL: ItemDefinition = ItemDefinition {
    id: rustcraft_engine_core::ItemId(103),
    name: "minecraft_b173:wood_shovel",
    max_stack: 1,
    placeable: None,
    capabilities: &["minecraft_b173:capability/tool"],
    tool: Some(ToolDefinition {
        category: ToolCategory::Shovel,
        tier: ToolTier::Wood,
        speed: 2.0,
        durability: 59,
    }),
};
pub const STONE_PICKAXE: ItemDefinition = ItemDefinition {
    id: rustcraft_engine_core::ItemId(104),
    name: "minecraft_b173:stone_pickaxe",
    max_stack: 1,
    placeable: None,
    capabilities: &["minecraft_b173:capability/tool"],
    tool: Some(ToolDefinition {
        category: ToolCategory::Pickaxe,
        tier: ToolTier::Stone,
        speed: 4.0,
        durability: 131,
    }),
};
pub const STONE_AXE: ItemDefinition = ItemDefinition {
    id: rustcraft_engine_core::ItemId(105),
    name: "minecraft_b173:stone_axe",
    max_stack: 1,
    placeable: None,
    capabilities: &["minecraft_b173:capability/tool"],
    tool: Some(ToolDefinition {
        category: ToolCategory::Axe,
        tier: ToolTier::Stone,
        speed: 4.0,
        durability: 131,
    }),
};
pub const STONE_SHOVEL: ItemDefinition = ItemDefinition {
    id: rustcraft_engine_core::ItemId(106),
    name: "minecraft_b173:stone_shovel",
    max_stack: 1,
    placeable: None,
    capabilities: &["minecraft_b173:capability/tool"],
    tool: Some(ToolDefinition {
        category: ToolCategory::Shovel,
        tier: ToolTier::Stone,
        speed: 4.0,
        durability: 131,
    }),
};
pub const BLOCKS: [BlockDefinition; 21] = [
    AIR,
    STONE,
    GRASS,
    DIRT,
    COBBLESTONE,
    PLANKS,
    BEDROCK,
    SAND,
    GRAVEL,
    LOG,
    LEAVES,
    GLASS,
    BRICK,
    BOOKSHELF,
    SANDSTONE,
    LAMP,
    WATER,
    COAL_ORE,
    IRON_ORE,
    GOLD_ORE,
    DIAMOND_ORE,
];
pub const DEVELOPMENT_LOADOUT: [&str; 9] = [
    "minecraft_b173:stone",
    "minecraft_b173:cobblestone",
    "minecraft_b173:planks",
    "minecraft_b173:log",
    "minecraft_b173:leaves",
    "minecraft_b173:glass",
    "minecraft_b173:brick",
    "minecraft_b173:sandstone",
    "minecraft_b173:debug_lamp",
];
pub fn atlas_tile(resource: &str) -> Option<(u8, u8)> {
    let tile = match resource {
        "minecraft_b173:grass_top" => 0,
        "minecraft_b173:stone" => 1,
        "minecraft_b173:dirt" => 2,
        "minecraft_b173:grass_side" => 3,
        "minecraft_b173:planks" => 4,
        "minecraft_b173:brick" => 7,
        "minecraft_b173:cobblestone" => 16,
        "minecraft_b173:bedrock" => 17,
        "minecraft_b173:sand" => 18,
        "minecraft_b173:gravel" => 19,
        "minecraft_b173:log_side" => 20,
        "minecraft_b173:log_top" => 21,
        "minecraft_b173:bookshelf" => 35,
        "minecraft_b173:glass" => 49,
        "minecraft_b173:leaves" => 52,
        "minecraft_b173:sandstone_top" => 176,
        "minecraft_b173:sandstone_side" => 192,
        "minecraft_b173:sandstone_bottom" => 208,
        "minecraft_b173:lamp" => 105,
        "minecraft_b173:water" => 14,
        "minecraft_b173:coal_ore" => 34,
        "minecraft_b173:iron_ore" => 33,
        "minecraft_b173:gold_ore" => 32,
        "minecraft_b173:diamond_ore" => 50,
        _ => return None,
    };
    Some((tile % 16, tile / 16))
}
pub fn tint(block: BlockId, top: bool) -> [f32; 3] {
    if (block == GRASS.id && top) || block == LEAVES.id {
        [124.0_f32, 189., 107.].map(|c| ((c / 255. + 0.055) / 1.055).powf(2.4))
    } else {
        [1.; 3]
    }
}
#[derive(Debug, Default)]
pub struct BlocksModule;
impl GameplayModule for BlocksModule {
    fn id(&self) -> ModuleId {
        ModuleId("minecraft_b173:blocks")
    }
    fn register(&self, r: &mut BlockRegistry) -> Result<(), RegistrationError> {
        for b in BLOCKS {
            r.register(b)?;
        }
        for b in BLOCKS {
            if let Some(id) = b.item {
                r.register_item(ItemDefinition {
                    id,
                    name: b.name,
                    max_stack: 64,
                    placeable: Some(b.id),
                    capabilities: &["voxel_std:capability/placeable"],
                    tool: None,
                })?;
            }
        }
        for item in [
            STICK_ITEM,
            WOOD_PICKAXE,
            WOOD_AXE,
            WOOD_SHOVEL,
            STONE_PICKAXE,
            STONE_AXE,
            STONE_SHOVEL,
        ] {
            r.register_item(item)?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn module_registers_first_party_blocks() {
        let mut r = BlockRegistry::default();
        BlocksModule.register(&mut r).unwrap();
        assert_eq!(r.definitions().len(), 21);
        assert_eq!(r.items().len(), 27);
        for b in BLOCKS.into_iter().skip(1) {
            for f in 0..6 {
                assert!(atlas_tile(b.textures.face(f)).is_some());
            }
        }
    }
}
