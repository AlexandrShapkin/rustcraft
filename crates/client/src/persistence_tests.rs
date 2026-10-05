use super::*;
use rustcraft_engine_core::{ChunkPos, ItemId, World};
use rustcraft_mod_api::GameplayModule;

#[test]
fn failed_entity_save_generations_retire_and_latest_reopens() {
    let path = std::env::temp_dir().join(format!(
        "rustcraft-client-save-failures-{}",
        std::process::id()
    ));
    let storage = rustcraft_world::WorldStorage::open(&path, "test").unwrap();
    let mut registry = rustcraft_mod_api::BlockRegistry::default();
    BlocksModule.register(&mut registry).unwrap();
    let mut sim = Simulation::new_with_entity_namespace(
        World::new(rustcraft_minecraft_b173::blocks::AIR.id),
        registry,
        Vec3::ZERO,
        99,
    );
    let pos = ChunkPos { x: 0, z: 0 };
    sim.spawn_item(ItemId(1), 30, Vec3::new(0.5, 3., 0.5));
    storage
        .store_player(
            &rustcraft_minecraft_b173::player_persistence::encode_revision(&sim, 1, &[]).unwrap(),
        )
        .unwrap();
    let mut dirty = rustcraft_world::PersistenceDirtyTracker::default();
    let mut snapshots = HashMap::new();
    let chunks = storage.root().join("chunks");
    let held = storage.root().join("chunks-held");
    std::fs::rename(&chunks, &held).unwrap();
    std::fs::write(&chunks, b"injected save failure").unwrap();
    for _ in 0..8 {
        dirty.mark_dirty(pos);
        let token = dirty.begin_save(pos).unwrap();
        let (chunk, snapshot) = encode_simulation_column(&sim, pos).unwrap();
        snapshots.insert((pos.x, pos.z, token.generation), snapshot);
        sim.items[0].position.x += 0.1;
        sim.items[0].persistence_revision += 1;
        dirty.mark_dirty(pos);
        assert!(storage.store_chunk(&chunk).is_err());
        dirty.complete_save(token, false);
        assert!(retire_entity_save_snapshot(&mut snapshots, token, false).is_none());
        assert!(snapshots.is_empty());
        assert!(dirty.is_dirty(pos));
        assert!(!dirty.is_saving(pos));
        // A failed write must not establish a durable source/owner.
        assert_eq!(sim.entity_lifetime_counts()[1], 0);
    }
    std::fs::remove_file(&chunks).unwrap();
    std::fs::rename(&held, &chunks).unwrap();
    let token = dirty.begin_save(pos).unwrap();
    let (chunk, snapshot) = encode_simulation_column(&sim, pos).unwrap();
    snapshots.insert((pos.x, pos.z, token.generation), snapshot);
    storage.store_chunk(&chunk).unwrap();
    dirty.complete_save(token, true);
    sim.note_entity_column_persisted(
        &retire_entity_save_snapshot(&mut snapshots, token, true).unwrap(),
    );
    assert!(snapshots.is_empty());
    assert!(!dirty.is_dirty(pos));
    let reopened = rustcraft_world::WorldStorage::open(&path, "test").unwrap();
    let chunk = reopened.load_chunk(pos).unwrap();
    let entity = rustcraft_minecraft_b173::world_persistence::decode_item_entity(
        &chunk.spatial_records[0],
        &sim.registry,
        "test",
        pos,
    )
    .unwrap();
    assert_eq!(entity, sim.items[0]);
    assert_eq!(entity.stack.count, 30);
    let player = reopened
        .load_player(rustcraft_minecraft_b173::player_persistence::LOCAL_PLAYER_ID)
        .unwrap()
        .unwrap();
    let player =
        rustcraft_minecraft_b173::player_persistence::decode(&player, &sim.registry).unwrap();
    let mut recovered = Simulation::new(
        World::new(rustcraft_minecraft_b173::blocks::AIR.id),
        sim.registry.clone(),
        Vec3::ZERO,
    );
    rustcraft_minecraft_b173::player_persistence::apply(&mut recovered, player);
    recovered
        .activate_entity_column(pos, vec![entity], &[])
        .unwrap();
    assert_eq!(
        recovered
            .inventory
            .slots()
            .iter()
            .flatten()
            .map(|stack| u32::from(stack.count))
            .sum::<u32>()
            + u32::from(recovered.items[0].stack.count),
        30
    );
    std::fs::remove_dir_all(path).unwrap();
}
