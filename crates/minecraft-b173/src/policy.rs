//! Minecraft rule values registered through the same public native mechanisms as any game.
use rustcraft_content::ResourceId;
use rustcraft_game_api::{BlockKey, RecipeDefinition, WorkDefinition};
use rustcraft_mod_api::{BlockRegistry, RegistrationError, ToolCategory};
fn key(name: &str) -> ResourceId {
    ResourceId::parse(name).expect("registered Minecraft identity")
}
pub fn register(r: &mut BlockRegistry) -> Result<(), RegistrationError> {
    for b in r.definitions().to_vec() {
        if !b.breakable {
            continue;
        }
        let rates = r
            .items()
            .iter()
            .filter_map(|i| {
                i.tool
                    .filter(|tool| Some(tool.category) == b.preferred_tool)
                    .map(|tool| (key(i.name), tool.speed))
            })
            .collect();
        r.register_work(WorkDefinition {
            target: BlockKey::parse(b.name).unwrap(),
            duration_seconds: b.hardness.max(0.05),
            item_multipliers: rates,
            reward: b.drop.and_then(|id| r.item(id)).map(|i| (key(i.name), 1)),
        })?;
    }
    let mut recipe =
        |name: &'static str, shaped, inputs: Vec<Option<ResourceId>>, output: &str, count| {
            r.register_recipe(RecipeDefinition {
                key: key(&format!("minecraft_b173:recipe/{name}")),
                local_alias: Some(name),
                shaped,
                width: 2,
                inputs,
                output: key(output),
                count,
            })
        };
    recipe(
        "log_to_planks",
        false,
        vec![Some(key("minecraft_b173:log"))],
        "minecraft_b173:planks",
        4,
    )?;
    recipe(
        "planks_to_sticks",
        true,
        vec![
            Some(key("minecraft_b173:planks")),
            None,
            Some(key("minecraft_b173:planks")),
            None,
        ],
        "minecraft_b173:stick",
        4,
    )?;
    // Recipe material/category is game policy; no historical numeric comparison.
    for (name, material, category) in [
        ("wood_pickaxe", "planks", ToolCategory::Pickaxe),
        ("wood_axe", "planks", ToolCategory::Axe),
        ("wood_shovel", "planks", ToolCategory::Shovel),
        ("stone_pickaxe", "cobblestone", ToolCategory::Pickaxe),
        ("stone_axe", "cobblestone", ToolCategory::Axe),
        ("stone_shovel", "cobblestone", ToolCategory::Shovel),
    ] {
        let material = Some(key(&format!("minecraft_b173:{material}")));
        let stick = Some(key("minecraft_b173:stick"));
        let inputs = match category {
            ToolCategory::Pickaxe => vec![material.clone(), material, stick.clone(), stick],
            ToolCategory::Axe => vec![material.clone(), material.clone(), material, stick],
            ToolCategory::Shovel => vec![material, None, stick, None],
        };
        r.register_recipe(RecipeDefinition {
            key: key(&format!("minecraft_b173:recipe/{name}")),
            local_alias: Some("tool"),
            shaped: true,
            width: 2,
            inputs,
            output: key(&format!("minecraft_b173:{name}")),
            count: 1,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_engine_core::{BlockId, BlockPos, ItemId, Vec3, World};
    use rustcraft_mod_api::GameplayModule;
    use rustcraft_runtime::{Simulation, inventory::ItemStack};
    #[test]
    fn rules_survive_reordered_and_renumbered_local_registry_with_semantic_save_roundtrip() {
        let mut source = BlockRegistry::default();
        crate::blocks::BlocksModule.register(&mut source).unwrap();
        let mut r = BlockRegistry::default();
        for mut b in source.definitions().iter().copied().rev() {
            b.id = BlockId(b.id.0 + 1000);
            b.item = b.item.map(|id| ItemId(id.0 + 2000));
            b.drop = b.drop.map(|id| ItemId(id.0 + 2000));
            r.register(b).unwrap();
        }
        for mut item in source.items().iter().cloned().rev() {
            item.id = ItemId(item.id.0 + 2000);
            item.placeable = item.placeable.map(|id| BlockId(id.0 + 1000));
            r.register_item(item).unwrap();
        }
        r.bind_profile(crate::compile_profile().unwrap()).unwrap();
        register(&mut r).unwrap();
        let empty = r
            .resolve_block_key(&BlockKey::parse("minecraft_b173:air").unwrap())
            .unwrap();
        let mut world = World::new(empty);
        let stone = r
            .resolve_block_key(&BlockKey::parse("minecraft_b173:stone").unwrap())
            .unwrap();
        for x in -4..=4 {
            for z in -4..=4 {
                world.set(BlockPos { x, y: 14, z }, stone);
            }
        }
        let target = BlockPos { x: 0, y: 16, z: 3 };
        world.set(target, stone);
        let mut sim = Simulation::new(world, r, Vec3::new(0.5, 15., 0.5));
        sim.set_survival();
        let item =
            |name: &str, sim: &Simulation| sim.registry.resolve_item_key(&key(name)).unwrap();
        let log = item("minecraft_b173:log", &sim);
        sim.inventory.insert(log, 1, &sim.registry);
        assert!(sim.craft_first_available("log_to_planks"));
        let planks = item("minecraft_b173:planks", &sim);
        assert_eq!(
            sim.inventory
                .slots()
                .iter()
                .flatten()
                .filter(|s| s.item == planks)
                .map(|s| s.count)
                .sum::<u16>(),
            4
        );
        let pickaxe = item("minecraft_b173:wood_pickaxe", &sim);
        sim.inventory.put_slot(
            0,
            Some(ItemStack {
                item: pickaxe,
                count: 1,
                damage: 0,
            }),
        );
        for _ in 0..8 {
            sim.step(
                rustcraft_agent_api::AgentIntent::<()> {
                    primary_action: true,
                    ..Default::default()
                },
                0.1,
            );
        }
        assert_eq!(sim.world.get(target), empty);
        assert_eq!(sim.inventory.held().unwrap().damage, 1);
        let cobble = item("minecraft_b173:cobblestone", &sim);
        assert!(
            sim.items
                .iter()
                .any(|e| e.stack.item == cobble && e.stack.count == 1)
        );
        let record = crate::player_persistence::encode_revision(&sim, 1, &[]).unwrap();
        let decoded = crate::player_persistence::decode(&record, &source).unwrap();
        let mut reopened = Simulation::new(World::new(crate::blocks::AIR.id), source, Vec3::ZERO);
        crate::player_persistence::apply(&mut reopened, decoded);
        assert_eq!(
            reopened
                .registry
                .item(reopened.inventory.held().unwrap().item)
                .unwrap()
                .name,
            "minecraft_b173:wood_pickaxe"
        );
    }
}
