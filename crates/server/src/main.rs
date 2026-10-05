use rustcraft_agent_api::{AgentIntent, MoveIntent};
use rustcraft_bot_api::ScriptedBot;
use rustcraft_content::{
    ContentHash, ContentManifest, PackageDescriptor, PackageId, PackageKind, PackageTarget,
    PackageVersion,
};
use rustcraft_engine_core::{BlockId, BlockPos, ChunkPos, Vec3, World};
use rustcraft_minecraft_b173::blocks::{BlocksModule, GRASS, STONE};
use rustcraft_runtime::survival::GameMode;
use rustcraft_runtime::{RuntimeBootstrap, Simulation, run_controller};

fn main() {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.iter().any(|argument| argument == "--version") {
        println!("{}", rustcraft_build_info::identity());
        return;
    }
    if arguments.iter().any(|a| a == "--config-report") {
        let mut config = rustcraft_control::config::settings::engine(false);
        rustcraft_control::config::settings::load(
            &mut config,
            &arguments,
            |k| std::env::var(k).ok(),
            rustcraft_control::config::user_path(),
        )
        .expect("configuration");
        println!("{}", config.snapshot());
        return;
    }
    if arguments.iter().any(|a| a == "--config-smoke") {
        if let Err(e) = run_config_smoke(&arguments) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    rustcraft_minecraft_b173::validate_package()
        .expect("minecraft_b173 must register through the public Game API");
    if let Some(index) = arguments.iter().position(|arg| arg == "--scenario") {
        let path = arguments.get(index + 1).unwrap_or_else(|| {
            eprintln!("--scenario requires PATH");
            std::process::exit(2)
        });
        if let Err(error) = run_dx_scenario(path) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(index) = arguments.iter().position(|arg| arg == "--script-check") {
        let path = arguments.get(index + 1).map_or("scripts", String::as_str);
        if let Err(error) = rustcraft_scripting_rhai::check_scripts(std::path::Path::new(path)) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if arguments.iter().any(|arg| arg == "--script-bench") {
        rustcraft_scripting_rhai::benchmark();
        return;
    }
    let smoke = arguments.iter().any(|arg| arg == "--smoke");
    if let Some(index) = arguments.iter().position(|arg| arg == "--world-info") {
        run_world_info(&arguments[index + 1..]);
    } else if let Some(index) = arguments.iter().position(|arg| arg == "--inspect-chunk") {
        run_chunk_info(&arguments[index + 1..]);
    } else if arguments.iter().any(|arg| arg == "--world-roundtrip") {
        run_world_roundtrip();
    } else if arguments.iter().any(|arg| arg == "--world-state-roundtrip") {
        run_world_state_roundtrip();
    } else if arguments
        .iter()
        .any(|arg| arg == "--entity-persistence-bench")
    {
        run_entity_persistence_bench();
    } else if arguments.iter().any(|arg| arg == "--worldgen-bench") {
        run_worldgen_bench();
    } else if let Some(index) = arguments.iter().position(|arg| arg == "--worldgen-report") {
        run_worldgen_report(&arguments[index + 1..], false);
    } else if let Some(index) = arguments.iter().position(|arg| arg == "--worldgen-map") {
        run_worldgen_report(&arguments[index + 1..], true);
    } else if arguments.iter().any(|arg| arg == "--persistence-bench") {
        run_persistence_bench();
    } else if arguments.iter().any(|arg| arg == "--world-stream-bench") {
        run_world_stream_bench();
    } else if smoke {
        run_smoke();
    } else if arguments.iter().any(|arg| arg == "--survival") {
        run_survival();
    } else {
        println!("rustcraft server bootstrap; run with --smoke for the headless scenario");
    }
}

fn run_world_info(arguments: &[String]) {
    let [root, world, ..] = arguments else {
        panic!("usage: --world-info <saves-root> <world>");
    };
    let storage = rustcraft_world::WorldStorage::open(root, world).unwrap();
    let metadata = storage.load_metadata().unwrap();
    let global = storage.load_world_state().unwrap();
    let (global_revision, world_time, component_count, recovered) =
        global.as_ref().map_or((0, 0, 0, false), |record| {
            let time = rustcraft_minecraft_b173::world_persistence::decode_world_state(record)
                .map(|(time, _)| time)
                .unwrap_or_default();
            (
                record.revision,
                time,
                record.components.len(),
                record.recovered_from_checkpoint,
            )
        });
    println!(
        "world={} root={} world_format={} metadata_format={} persistence_schema={} game={} seed={} generator={}:v{} global_envelope={} global_revision={} world_time_ticks={} global_components={} recovered_previous={}",
        world,
        storage.root().display(),
        rustcraft_world::WORLD_FORMAT_VERSION,
        rustcraft_world::WORLD_METADATA_VERSION,
        metadata.persistence_schema_version,
        metadata.game_id,
        metadata.seed,
        metadata.generator_id,
        metadata.generator_version,
        rustcraft_world::WORLD_STATE_RECORD_VERSION,
        global_revision,
        world_time,
        component_count,
        recovered,
    );
    if let Some(global) = global {
        for component in global.components {
            println!(
                "  global_component={} schema={} payload_bytes={}",
                component.id,
                component.schema_version,
                component.payload.len()
            );
        }
    }
    if let Some(record) = storage
        .load_player(rustcraft_minecraft_b173::player_persistence::LOCAL_PLAYER_ID)
        .unwrap()
    {
        let player =
            rustcraft_minecraft_b173::player_persistence::decode(&record, &persistence_registry())
                .unwrap();
        println!(
            "  player_position={:?} yaw={} pitch={} revision={}",
            player.position, player.yaw, player.pitch, record.revision
        );
    }
}

fn run_chunk_info(arguments: &[String]) {
    let [root, world, x, z, ..] = arguments else {
        panic!("usage: --inspect-chunk <saves-root> <world> <x> <z>");
    };
    let position = ChunkPos {
        x: x.parse().expect("chunk x must be i32"),
        z: z.parse().expect("chunk z must be i32"),
    };
    let storage = rustcraft_world::WorldStorage::open(root, world).unwrap();
    let chunk = storage.load_chunk(position).unwrap();
    let palette_entries = chunk
        .sections
        .iter()
        .map(|section| {
            section
                .states
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
        })
        .sum::<usize>();
    println!(
        "world={} chunk=({}, {}) chunk_format={} sections={} palette_entries={} voxel_states={} spatial_entities={} tombstones={} file_bytes={}",
        world,
        position.x,
        position.z,
        rustcraft_world::CHUNK_FORMAT_VERSION,
        chunk.sections.len(),
        palette_entries,
        chunk
            .sections
            .iter()
            .map(|section| section.states.len())
            .sum::<usize>(),
        chunk.spatial_records.len(),
        chunk.spatial_tombstones.len(),
        storage.chunk_file_bytes(position).unwrap_or_default(),
    );
    for record in chunk.spatial_records {
        println!(
            "  entity={} type={} schema={} revision={} payload_bytes={}",
            record.entity_id,
            record.entity_type,
            record.schema_version,
            record.entity_revision,
            record.payload.len()
        );
    }
}

fn persisted_column(
    simulation: &Simulation,
    position: ChunkPos,
) -> (
    rustcraft_world::StoredChunk,
    rustcraft_runtime::EntityColumnSnapshot,
) {
    let resolver = rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver;
    let sections = simulation
        .world
        .section_positions()
        .filter(|(candidate, _)| *candidate == position)
        .filter_map(|(_, y)| {
            simulation
                .world
                .section(position, y)
                .cloned()
                .map(|chunk| (y, chunk))
        })
        .collect::<Vec<_>>();
    let snapshot = simulation.entity_column_snapshot(position);
    let records = snapshot
        .records
        .iter()
        .map(|entity| {
            rustcraft_minecraft_b173::world_persistence::encode_item_entity(
                entity,
                &simulation.registry,
            )
            .unwrap()
        })
        .collect();
    let tombstones = snapshot
        .tombstones
        .iter()
        .map(
            |(entity_id, source, entity_revision)| rustcraft_world::SpatialTombstone {
                entity_id: *entity_id,
                entity_revision: *entity_revision,
                source: *source,
            },
        )
        .collect();
    (
        rustcraft_world::WorldStorage::encode_runtime_column(
            position, sections, &resolver, records, tombstones,
        )
        .unwrap(),
        snapshot,
    )
}

fn store_simulation_column(
    storage: &rustcraft_world::WorldStorage,
    simulation: &mut Simulation,
    position: ChunkPos,
) -> rustcraft_world::ChunkEncodingMetrics {
    let (stored, snapshot) = persisted_column(simulation, position);
    let metrics = storage.store_chunk_measured(&stored).unwrap();
    let _ = simulation.note_entity_column_persisted(&snapshot);
    metrics
}

fn activate_stored_column(
    storage: &rustcraft_world::WorldStorage,
    simulation: &mut Simulation,
    world_name: &str,
    position: ChunkPos,
) {
    let resolver = rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver;
    let column = storage.load_runtime_column(position, &resolver).unwrap();
    simulation
        .publish_column(position, column.sections, false)
        .unwrap();
    let entities = column
        .spatial_records
        .iter()
        .map(|record| {
            rustcraft_minecraft_b173::world_persistence::decode_item_entity(
                record,
                &simulation.registry,
                world_name,
                position,
            )
            .unwrap()
        })
        .collect();
    let tombstones = column
        .spatial_tombstones
        .iter()
        .map(|tombstone| {
            (
                tombstone.entity_id,
                tombstone.source,
                tombstone.entity_revision,
            )
        })
        .collect::<Vec<_>>();
    simulation
        .activate_entity_column(position, entities, &tombstones)
        .unwrap();
}

fn persistence_registry() -> rustcraft_mod_api::BlockRegistry {
    use rustcraft_mod_api::GameplayModule;
    let mut registry = rustcraft_mod_api::BlockRegistry::default();
    BlocksModule.register(&mut registry).unwrap();
    registry
}

fn run_world_state_roundtrip() {
    use rustcraft_engine_core::ItemId;
    use rustcraft_world::{WorldStateComponent, WorldStorage};
    use std::time::{SystemTime, UNIX_EPOCH};

    let root = std::env::temp_dir().join(format!(
        "rustcraft-m4-world-state-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let world_name = "roundtrip";
    let storage = WorldStorage::open(&root, world_name).unwrap();
    let registry = persistence_registry();
    let mut world = World::new(rustcraft_minecraft_b173::blocks::AIR.id);
    world.set(BlockPos { x: 0, y: 0, z: 0 }, STONE.id);
    world.set(BlockPos { x: 16, y: 0, z: 0 }, STONE.id);
    let edited = BlockPos { x: 2, y: 1, z: 2 };
    world.set(edited, STONE.id);
    let mut simulation = Simulation::new_with_entity_namespace(
        world,
        registry.clone(),
        Vec3::new(100.5, 10.0, 100.5),
        0x004d_3430_3033,
    );
    simulation.set_mode(GameMode::Survival);
    simulation.time = 98_765;

    simulation.spawn_item(ItemId(1), 3, Vec3::new(8.25, 4.5, 2.75));
    simulation.items[0].velocity = Vec3::new(0.125, -0.25, 0.5);
    simulation.items[0].age = 219.5;
    simulation.items[0].pickup_delay = 1.25;
    simulation.spawn_item(ItemId(3), 2, Vec3::new(15.85, 4.0, 8.0));
    simulation.items[1].velocity = Vec3::new(4.0, 0.0, 0.0);
    simulation.items[1].pickup_delay = 10.0;
    simulation.spawn_item(ItemId(4), 2, Vec3::new(5.0, 4.0, 12.0));
    simulation.spawn_item(ItemId(4), 3, Vec3::new(5.4, 4.0, 12.0));
    let merge_ids = [simulation.items[2].id, simulation.items[3].id];
    for entity in &mut simulation.items[2..] {
        entity.pickup_delay = 10.0;
    }
    simulation.step(AgentIntent::default(), 0.0); // production merge path
    assert_eq!(
        simulation
            .items
            .iter()
            .filter(|entity| entity.stack.item == ItemId(4))
            .map(|entity| u32::from(entity.stack.count))
            .sum::<u32>(),
        5
    );
    let merge_survivors = simulation
        .items
        .iter()
        .filter(|entity| merge_ids.contains(&entity.id))
        .map(|entity| entity.id)
        .collect::<Vec<_>>();
    assert_eq!(merge_survivors.len(), 1);
    assert!(simulation.break_block(edited));
    let block_drop_id = simulation.items.last().unwrap().id;
    simulation.spawn_item(ItemId(5), 1, Vec3::new(10.5, 4.0, 10.5));
    let near_despawn_id = simulation.items.last().unwrap().id;
    simulation.items.last_mut().unwrap().age = 299.92;
    simulation.items.last_mut().unwrap().pickup_delay = 10.0;

    let source = ChunkPos { x: 0, z: 0 };
    let destination = ChunkPos { x: 1, z: 0 };
    store_simulation_column(&storage, &mut simulation, source);
    let moving_id = simulation.items[1].id;
    simulation.step(AgentIntent::default(), 0.05); // crosses x=16 through normal physics
    assert_eq!(
        simulation
            .items
            .iter()
            .find(|entity| entity.id == moving_id)
            .unwrap()
            .column(),
        destination
    );
    let moving_after_cross = *simulation
        .items
        .iter()
        .find(|entity| entity.id == moving_id)
        .unwrap();
    // Destination-before-source plus stable-ID tombstone is the cross-file crash protocol.
    store_simulation_column(&storage, &mut simulation, destination);
    store_simulation_column(&storage, &mut simulation, source);
    store_simulation_column(&storage, &mut simulation, destination); // prune acknowledged marker

    let unknown = WorldStateComponent {
        id: "test:world/future".into(),
        schema_version: 9,
        payload: vec![9, 8, 7],
    };
    storage
        .store_world_state(
            &rustcraft_minecraft_b173::world_persistence::encode_world_state(
                simulation.time,
                1,
                std::slice::from_ref(&unknown),
            ),
        )
        .unwrap();
    let player =
        rustcraft_minecraft_b173::player_persistence::encode_revision(&simulation, 1, &[]).unwrap();
    storage.store_player(&player).unwrap();

    let expected_ids = simulation
        .items
        .iter()
        .map(|entity| entity.id)
        .collect::<std::collections::HashSet<_>>();
    simulation.evict_entity_column(source);
    simulation.evict_entity_column(destination);
    simulation.world.remove_column(source);
    simulation.world.remove_column(destination);
    assert!(simulation.items.is_empty());

    let mut restored = Simulation::new_with_entity_namespace(
        World::new(rustcraft_minecraft_b173::blocks::AIR.id),
        registry.clone(),
        Vec3::ZERO,
        0x004d_3430_3034,
    );
    activate_stored_column(&storage, &mut restored, world_name, source);
    activate_stored_column(&storage, &mut restored, world_name, destination);
    assert_eq!(
        restored
            .items
            .iter()
            .map(|entity| entity.id)
            .collect::<std::collections::HashSet<_>>(),
        expected_ids
    );
    let restored_moving = restored
        .items
        .iter()
        .find(|entity| entity.id == moving_id)
        .unwrap();
    assert_eq!(restored_moving.velocity, moving_after_cross.velocity);
    assert_eq!(restored_moving.position, moving_after_cross.position);
    assert_eq!(restored_moving.age, moving_after_cross.age);
    assert_eq!(
        restored_moving.pickup_delay,
        moving_after_cross.pickup_delay
    );
    assert_eq!(
        restored
            .items
            .iter()
            .filter(|entity| merge_ids.contains(&entity.id))
            .count(),
        1,
        "consumed merge participant resurrected"
    );
    assert_eq!(
        restored.world.get(edited),
        rustcraft_minecraft_b173::blocks::AIR.id
    );
    let global = storage.load_world_state().unwrap().unwrap();
    let (time, preserved) =
        rustcraft_minecraft_b173::world_persistence::decode_world_state(&global).unwrap();
    assert_eq!(time, simulation.time);
    assert_eq!(preserved, vec![unknown.clone()]);
    restored.time = time;

    restored.player.position = Vec3::new(100.5, 10.0, 100.5);
    restored.step(AgentIntent::default(), 0.04);
    assert!(
        !restored
            .items
            .iter()
            .any(|entity| entity.id == near_despawn_id),
        "near-threshold entity did not despawn after reopen"
    );
    store_simulation_column(&storage, &mut restored, source);

    let picked = *restored
        .items
        .iter()
        .find(|entity| entity.id == block_drop_id)
        .expect("block-break drop survived reopen");
    assert!(picked.pickup_delay > 0.0);
    restored.player.position = picked.position;
    restored.step(AgentIntent::default(), 0.05);
    assert!(
        restored.items.iter().any(|entity| entity.id == picked.id),
        "pickup succeeded before persisted delay expired"
    );
    restored.step(AgentIntent::default(), 0.25);
    assert!(!restored.items.iter().any(|entity| entity.id == picked.id));
    assert!(
        restored
            .pickup_receipts()
            .iter()
            .any(|receipt| receipt.entity_id == picked.id)
    );
    let receipt_player =
        rustcraft_minecraft_b173::player_persistence::encode_revision(&restored, 2, &[]).unwrap();
    storage.store_player(&receipt_player).unwrap(); // inventory+receipt first
    restored.commit_pickup_receipts(&restored.pickup_receipts());
    store_simulation_column(&storage, &mut restored, picked.column());
    let pruned_player =
        rustcraft_minecraft_b173::player_persistence::encode_revision(&restored, 3, &[]).unwrap();
    storage.store_player(&pruned_player).unwrap();

    restored.time += 400;
    storage
        .store_world_state(
            &rustcraft_minecraft_b173::world_persistence::encode_world_state(
                restored.time,
                2,
                std::slice::from_ref(&unknown),
            ),
        )
        .unwrap();
    let player_record = storage
        .load_player(rustcraft_minecraft_b173::player_persistence::LOCAL_PLAYER_ID)
        .unwrap()
        .unwrap();
    let player_state =
        rustcraft_minecraft_b173::player_persistence::decode(&player_record, &registry).unwrap();
    assert!(player_state.pickup_receipts.is_empty());
    assert!(
        player_state
            .inventory
            .slots()
            .iter()
            .flatten()
            .any(|stack| stack.item == picked.stack.item && stack.count >= picked.stack.count)
    );
    let mut verified = Simulation::new_with_entity_namespace(
        World::new(rustcraft_minecraft_b173::blocks::AIR.id),
        registry.clone(),
        Vec3::ZERO,
        0x004d_3430_3035,
    );
    activate_stored_column(&storage, &mut verified, world_name, source);
    activate_stored_column(&storage, &mut verified, world_name, destination);
    assert!(
        !verified
            .items
            .iter()
            .any(|entity| { entity.id == picked.id || entity.id == near_despawn_id })
    );
    assert_eq!(
        verified.world.get(edited),
        rustcraft_minecraft_b173::blocks::AIR.id
    );
    let final_global = storage.load_world_state().unwrap().unwrap();
    let (final_time, final_unknown) =
        rustcraft_minecraft_b173::world_persistence::decode_world_state(&final_global).unwrap();
    assert_eq!(final_time, restored.time);
    assert!(final_time > time);
    assert_eq!(final_unknown, vec![unknown]);

    println!(
        "world-state-roundtrip: time={} entities_before={} entities_after_pickup={} stable_ids=ok merge=ok cross_column=ok eviction_reload=ok block_break_drop=ok pickup_delay=ok velocity_restore=ok despawn_reopen=ok inventory_receipt=ok unknown_global=preserved",
        final_time,
        expected_ids.len(),
        restored.items.len()
    );
    std::fs::remove_dir_all(&root).unwrap();
}

fn run_entity_persistence_bench() {
    use rustcraft_engine_core::ItemId;
    use rustcraft_world::WorldStorage;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};
    let root = std::env::temp_dir().join(format!(
        "rustcraft-m4-entity-bench-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let storage = WorldStorage::open(&root, "bench").unwrap();
    for count in [16usize, 1_000] {
        let mut simulation = Simulation::new_with_entity_namespace(
            World::new(rustcraft_minecraft_b173::blocks::AIR.id),
            persistence_registry(),
            Vec3::ZERO,
            count as u64 + 1,
        );
        for index in 0..count {
            simulation.spawn_item(
                ItemId(1),
                1,
                Vec3::new(
                    0.125 + (index % 16) as f32 * 0.9,
                    4.0 + (index / 256) as f32,
                    0.125 + ((index / 16) % 16) as f32 * 0.9,
                ),
            );
        }
        let position = ChunkPos { x: 0, z: 0 };
        let encode_started = Instant::now();
        let (chunk, _) = persisted_column(&simulation, position);
        let encode_ms = encode_started.elapsed().as_secs_f64() * 1000.0;
        let write_started = Instant::now();
        let metrics = storage.store_chunk_measured(&chunk).unwrap();
        let write_ms = write_started.elapsed().as_secs_f64() * 1000.0;
        let read_started = Instant::now();
        let resolver = rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver;
        let loaded = storage.load_runtime_column(position, &resolver).unwrap();
        let decoded = loaded
            .spatial_records
            .iter()
            .map(|record| {
                rustcraft_minecraft_b173::world_persistence::decode_item_entity(
                    record,
                    &simulation.registry,
                    "bench",
                    position,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let read_decode_ms = read_started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(decoded.len(), count);
        println!(
            "entity-persistence-bench records={} raw_bytes={} stored_bytes={} compression={} encode_ms={:.3} write_sync_ms={:.3} read_decode_ms={:.3}",
            count,
            metrics.raw_payload_bytes,
            metrics.stored_file_bytes,
            metrics.compression_method,
            encode_ms,
            write_ms,
            read_decode_ms
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

fn run_worldgen_bench() {
    use rustcraft_world::GenerationScheduler;
    use std::time::{Duration, Instant};
    let positions = (-2..2)
        .flat_map(|z| (-2..2).map(move |x| ChunkPos { x, z }))
        .collect::<Vec<_>>();
    for version in [1, 2] {
        let generator = rustcraft_minecraft_b173::worldgen::resolve_overworld_generator(
            rustcraft_minecraft_b173::worldgen::OVERWORLD_GENERATOR_ID,
            version,
        )
        .unwrap();
        let mut scheduler = GenerationScheduler::new(4, positions.len());
        let started = Instant::now();
        for (i, position) in positions.iter().copied().enumerate() {
            scheduler
                .request(generator.clone(), 731_173, position, i as u64 + 1)
                .unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut generated = Vec::new();
        while generated.len() < positions.len() && Instant::now() < deadline {
            generated.extend(scheduler.take_ready());
            std::thread::yield_now();
        }
        assert_eq!(
            generated.len(),
            positions.len(),
            "worldgen v{version} benchmark timed out"
        );
        generated.sort_by_key(|column| (column.position.x, column.position.z));
        let mut legacy_hash = blake3::Hasher::new();
        let mut semantic_hash = blake3::Hasher::new();
        let mut section_count = 0usize;
        let mut estimated_bytes = 0usize;
        for column in &generated {
            legacy_hash.update(&column.position.x.to_le_bytes());
            legacy_hash.update(&column.position.z.to_le_bytes());
            semantic_hash.update(&column.position.x.to_le_bytes());
            semantic_hash.update(&column.position.z.to_le_bytes());
            let mut sections = column.sections.as_ref().unwrap().iter().collect::<Vec<_>>();
            sections.sort_by_key(|(y, _)| *y);
            for (y, chunk) in sections {
                section_count += 1;
                estimated_bytes += std::mem::size_of_val(chunk.states());
                legacy_hash.update(&y.to_le_bytes());
                semantic_hash.update(&y.to_le_bytes());
                for state in chunk.states() {
                    legacy_hash.update(&state.block.0.to_le_bytes());
                    legacy_hash.update(&state.variant.to_le_bytes());
                    rustcraft_minecraft_b173::worldgen::update_semantic_state_hash(
                        &mut semantic_hash,
                        *state,
                    );
                }
            }
        }
        let elapsed = started.elapsed().as_secs_f64();
        let worker_ms = scheduler.metrics().generation_total_ms;
        println!(
            "worldgen-bench profile={} generator={}:v{} workers=4 seed=731173 chunks={} sections={} wall_ms={:.3} worker_generation_ms={:.3} ms_per_column={:.3} chunks_per_s={:.2} estimated_authoritative_bytes={} canonical_hash={} semantic_hash={} legacy_handle_hash={}",
            if cfg!(debug_assertions) {
                "dev"
            } else {
                "release"
            },
            generator.id(),
            generator.version(),
            positions.len(),
            section_count,
            elapsed * 1000.0,
            worker_ms,
            worker_ms / positions.len() as f64,
            positions.len() as f64 / elapsed,
            estimated_bytes,
            if version == 1 {
                legacy_hash.clone().finalize().to_hex()
            } else {
                semantic_hash.clone().finalize().to_hex()
            },
            semantic_hash.finalize().to_hex(),
            legacy_hash.finalize().to_hex(),
        );
    }
}

fn run_worldgen_report(arguments: &[String], emit_map: bool) {
    use rustcraft_minecraft_b173::worldgen::{
        Biome, MinecraftOverworldGeneratorV2, OVERWORLD_GENERATOR_ID, minecraft_overworld,
        minecraft_overworld_v2,
    };
    use rustcraft_world::ChunkGenerator;
    use std::{collections::BTreeMap, time::Instant};

    let version = arguments
        .first()
        .map_or(Ok(2), |value| value.parse::<u32>())
        .expect("worldgen report version must be u32");
    let seed = arguments
        .get(1)
        .map_or(Ok(731_173), |value| value.parse::<i64>())
        .expect("worldgen report seed must be i64");
    let radius = arguments
        .get(2)
        .map_or(Ok(6), |value| value.parse::<i32>())
        .expect("worldgen report radius must be i32")
        .clamp(1, 24);
    let v1 = minecraft_overworld();
    let v2 = minecraft_overworld_v2();
    let spawn_column = rustcraft_minecraft_b173::worldgen::initial_spawn_column(seed, version);
    let generator: &dyn ChunkGenerator = match version {
        1 => &v1,
        2 => &v2,
        _ => panic!("unsupported worldgen report version {version}"),
    };
    let mut columns = Vec::new();
    let mut generation_ms = Vec::new();
    let mut heights = Vec::new();
    let mut biome_counts = BTreeMap::<&'static str, u64>::new();
    let mut block_counts = BTreeMap::<u32, u64>::new();
    let mut stage = rustcraft_minecraft_b173::worldgen::V2GenerationMetrics::default();
    let mut canonical = blake3::Hasher::new();
    for x in -radius..=radius {
        for z in -radius..=radius {
            let position = ChunkPos { x, z };
            let started = Instant::now();
            let sections = if version == 2 {
                let (sections, metrics) = v2.generate_with_metrics(seed, position).unwrap();
                stage.climate += metrics.climate;
                stage.terrain += metrics.terrain;
                stage.surface += metrics.surface;
                stage.caves += metrics.caves;
                stage.ores += metrics.ores;
                stage.vegetation += metrics.vegetation;
                stage.structures += metrics.structures;
                stage.cave_blocks += metrics.cave_blocks;
                for (target, value) in stage.ore_blocks.iter_mut().zip(metrics.ore_blocks) {
                    *target += value;
                }
                stage.trees += metrics.trees;
                stage.lakes += metrics.lakes;
                sections
            } else {
                generator.generate(seed, position).unwrap()
            };
            generation_ms.push(started.elapsed().as_secs_f64() * 1000.0);
            canonical.update(&position.x.to_le_bytes());
            canonical.update(&position.z.to_le_bytes());
            for (section_y, chunk) in &sections {
                canonical.update(&section_y.to_le_bytes());
                for state in chunk.states() {
                    rustcraft_minecraft_b173::worldgen::update_semantic_state_hash(
                        &mut canonical,
                        *state,
                    );
                    *block_counts.entry(state.block.0).or_default() += 1;
                }
            }
            for local_z in 0..16_i32 {
                for local_x in 0..16_i32 {
                    let world_x = x * 16 + local_x;
                    let world_z = z * 16 + local_z;
                    heights.push(if version == 2 {
                        MinecraftOverworldGeneratorV2::terrain_height(seed, world_x, world_z)
                    } else {
                        report_surface_height(&sections, local_x, local_z)
                    });
                    let biome = if version == 2 {
                        MinecraftOverworldGeneratorV2::biome(seed, world_x, world_z).name()
                    } else {
                        "legacy_unclassified"
                    };
                    *biome_counts.entry(biome).or_default() += 1;
                }
            }
            columns.push((position, sections));
        }
    }
    heights.sort_unstable();
    generation_ms.sort_by(f64::total_cmp);
    let percentile = |values: &[f64], fraction: f64| {
        values[((values.len() as f64 * fraction).ceil() as usize)
            .saturating_sub(1)
            .min(values.len() - 1)]
    };
    let height_percentile = |fraction: f64| {
        heights[((heights.len() as f64 * fraction).ceil() as usize)
            .saturating_sub(1)
            .min(heights.len() - 1)]
    };
    let land = heights.iter().filter(|height| **height > 64).count();
    let mean_height =
        heights.iter().map(|height| f64::from(*height)).sum::<f64>() / heights.len() as f64;
    println!(
        "worldgen-report generator={}:v{} seed={} spawn_column=({}, {}) chunks={} sample_blocks={} semantic_hash={} height=min:{}/p05:{}/p50:{}/p95:{}/max:{}/mean:{:.2} land_fraction={:.3} ocean_fraction={:.3} generation_ms=p50:{:.3}/p95:{:.3}/max:{:.3}",
        OVERWORLD_GENERATOR_ID,
        version,
        seed,
        spawn_column.x,
        spawn_column.z,
        columns.len(),
        heights.len(),
        canonical.finalize().to_hex(),
        heights[0],
        height_percentile(0.05),
        height_percentile(0.50),
        height_percentile(0.95),
        heights[heights.len() - 1],
        mean_height,
        land as f64 / heights.len() as f64,
        1.0 - land as f64 / heights.len() as f64,
        percentile(&generation_ms, 0.50),
        percentile(&generation_ms, 0.95),
        generation_ms.last().copied().unwrap_or_default(),
    );
    println!("  biomes={biome_counts:?}");
    println!(
        "  blocks coal={} iron={} gold={} diamond={} logs={} leaves={} water={} sand={} gravel={}",
        block_counts
            .get(&rustcraft_minecraft_b173::blocks::COAL_ORE.id.0)
            .copied()
            .unwrap_or(0),
        block_counts
            .get(&rustcraft_minecraft_b173::blocks::IRON_ORE.id.0)
            .copied()
            .unwrap_or(0),
        block_counts
            .get(&rustcraft_minecraft_b173::blocks::GOLD_ORE.id.0)
            .copied()
            .unwrap_or(0),
        block_counts
            .get(&rustcraft_minecraft_b173::blocks::DIAMOND_ORE.id.0)
            .copied()
            .unwrap_or(0),
        block_counts
            .get(&rustcraft_minecraft_b173::blocks::LOG.id.0)
            .copied()
            .unwrap_or(0),
        block_counts
            .get(&rustcraft_minecraft_b173::blocks::LEAVES.id.0)
            .copied()
            .unwrap_or(0),
        block_counts
            .get(&rustcraft_minecraft_b173::blocks::WATER.id.0)
            .copied()
            .unwrap_or(0),
        block_counts
            .get(&rustcraft_minecraft_b173::blocks::SAND.id.0)
            .copied()
            .unwrap_or(0),
        block_counts
            .get(&rustcraft_minecraft_b173::blocks::GRAVEL.id.0)
            .copied()
            .unwrap_or(0),
    );
    if version == 2 {
        println!(
            "  stages_ms climate={:.3} terrain={:.3} surface={:.3} caves={:.3} ores={:.3} vegetation={:.3} structures={:.3} cave_blocks={} ore_blocks={:?} tree_origins={} lake_origins={}",
            stage.climate.as_secs_f64() * 1000.0,
            stage.terrain.as_secs_f64() * 1000.0,
            stage.surface.as_secs_f64() * 1000.0,
            stage.caves.as_secs_f64() * 1000.0,
            stage.ores.as_secs_f64() * 1000.0,
            stage.vegetation.as_secs_f64() * 1000.0,
            stage.structures.as_secs_f64() * 1000.0,
            stage.cave_blocks,
            stage.ore_blocks,
            stage.trees,
            stage.lakes,
        );
    }
    if emit_map {
        let mut map = String::new();
        map.push_str(&format!(
            "# {}:v{} seed={} one character per four blocks\n",
            OVERWORLD_GENERATOR_ID, version, seed
        ));
        for z in (-radius * 16..=(radius + 1) * 16 - 1).step_by(4) {
            for x in (-radius * 16..=(radius + 1) * 16 - 1).step_by(4) {
                let character = if version == 2 {
                    match MinecraftOverworldGeneratorV2::biome(seed, x, z) {
                        Biome::Ocean => '~',
                        Biome::Beach => '.',
                        Biome::Plains => ',',
                        Biome::Forest => 'F',
                        Biome::Desert => 'D',
                        Biome::Hills => '^',
                    }
                } else if report_v1_height(seed, x, z) <= 64 {
                    '~'
                } else {
                    '#'
                };
                map.push(character);
            }
            map.push('\n');
        }
        let path = std::path::PathBuf::from("target").join(format!(
            "worldgen-map-v{}-{}.txt",
            version,
            seed.to_string().replace('-', "neg")
        ));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, map).unwrap();
        println!("  map={}", path.display());
    }
}

fn report_surface_height(sections: &[(i32, rustcraft_engine_core::Chunk)], x: i32, z: i32) -> i32 {
    let air = rustcraft_minecraft_b173::blocks::AIR.id;
    let water = rustcraft_minecraft_b173::blocks::WATER.id;
    let leaves = rustcraft_minecraft_b173::blocks::LEAVES.id;
    let log = rustcraft_minecraft_b173::blocks::LOG.id;
    for (section_y, chunk) in sections.iter().rev() {
        for local_y in (0..16_u8).rev() {
            let block = chunk.state((x as u8, local_y, z as u8)).block;
            if block != air && block != water && block != leaves && block != log {
                return section_y * 16 + i32::from(local_y);
            }
        }
    }
    0
}

fn report_v1_height(seed: i64, x: i32, z: i32) -> i32 {
    rustcraft_minecraft_b173::worldgen::v1_terrain_height(seed, x, z)
}

fn run_persistence_bench() {
    use rustcraft_world::{ChunkGenerator, WorldStorage};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};
    let generator = rustcraft_minecraft_b173::worldgen::minecraft_overworld_v2();
    let resolver = rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver;
    let root = std::env::temp_dir().join(format!(
        "rustcraft-m4-persist-bench-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let storage = WorldStorage::open(&root, "bench").unwrap();
    let positions = [
        ChunkPos { x: -1, z: -1 },
        ChunkPos { x: 0, z: -1 },
        ChunkPos { x: -1, z: 0 },
        ChunkPos { x: 0, z: 0 },
    ];
    let mut raw_bytes = 0usize;
    let mut stored_bytes = 0u64;
    let mut encode_ms = 0.0;
    let mut write_ms = 0.0;
    let mut load_ms = 0.0;
    let mut compression_ms = 0.0;
    let mut zlib_chunks = 0_u32;
    for position in positions {
        let sections = generator.generate(731_173, position).unwrap();
        let started = Instant::now();
        let stored = WorldStorage::encode_runtime_chunk(position, sections, &resolver).unwrap();
        encode_ms += started.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        let encoded = storage.store_chunk_measured(&stored).unwrap();
        write_ms += started.elapsed().as_secs_f64() * 1000.0;
        raw_bytes += encoded.raw_payload_bytes;
        compression_ms += encoded.compression_ms;
        zlib_chunks += u32::from(encoded.compression_method == 1);
        stored_bytes += encoded.stored_file_bytes as u64;
        let started = Instant::now();
        let _ = storage.load_runtime_chunk(position, &resolver).unwrap();
        load_ms += started.elapsed().as_secs_f64() * 1000.0;
    }
    storage.flush().unwrap();
    println!(
        "persistence-bench compression=zlib-fast-if-smaller chunks={} compressed_chunks={} raw_payload_bytes={} stored_file_bytes={} ratio={:.3} semantic_palette_encode_ms={:.3} compression_cpu_ms={:.3} compression_plus_atomic_write_sync_ms={:.3} load_decompress_decode_ms={:.3}",
        positions.len(),
        zlib_chunks,
        raw_bytes,
        stored_bytes,
        stored_bytes as f64 / raw_bytes as f64,
        encode_ms,
        compression_ms,
        write_ms,
        load_ms
    );
    drop(storage);
    std::fs::remove_dir_all(root).unwrap();
}

fn drain_stream_lighting(simulation: &mut Simulation) -> (f64, f64) {
    use std::time::Instant;
    let mut total_ms = 0.0;
    let mut max_step_ms: f64 = 0.0;
    while simulation.lighting.has_integration_work() {
        let started = Instant::now();
        let _ = simulation.advance_column_lighting(4_096);
        let step_ms = started.elapsed().as_secs_f64() * 1000.0;
        total_ms += step_ms;
        max_step_ms = max_step_ms.max(step_ms);
        let _ = simulation.take_dirty_sections();
    }
    (total_ms, max_step_ms)
}

fn run_world_stream_bench() {
    use rustcraft_runtime::lighting::InitialLightingScheduler;
    use rustcraft_world::{
        ChunkGenerator, ChunkLoadScheduler, GenerationScheduler, PersistenceDirtyTracker,
        ResidencyPhase, SemanticBlockResolver, WorldResidency, WorldStorage,
    };
    use std::{
        collections::HashSet,
        sync::Arc,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    let seed = 731_173;
    let generator: Arc<dyn ChunkGenerator> =
        Arc::new(rustcraft_minecraft_b173::worldgen::minecraft_overworld_v2());
    let resolver: Arc<dyn SemanticBlockResolver> =
        Arc::new(rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver);
    let root = std::env::temp_dir().join(format!(
        "rustcraft-m4-stream-bench-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let storage = WorldStorage::open(&root, "stream").unwrap();
    let profile = rustcraft_minecraft_b173::compile_profile().unwrap();
    let mut bootstrap = RuntimeBootstrap::new(Default::default());
    bootstrap.register_module(&BlocksModule).unwrap();
    let mut simulation = Simulation::new(
        World::new(profile.default_state().block),
        bootstrap.registry,
        Vec3::new(0.0, 100.0, 0.0),
    );
    simulation.bind_content_profile(profile.clone()).unwrap();
    let light_work_start = simulation.lighting.work_counters();
    simulation.world.enforce_column_availability(true);
    let mut residency = WorldResidency::new(1, 1);
    // Radius one is a complete 3x3 Chebyshev square, so every stage must admit all nine critical
    // columns without letting an outer/speculative workload consume their bounded capacity.
    let mut loads = ChunkLoadScheduler::new(1, 9);
    let mut generations = GenerationScheduler::new(1, 9);
    let mut config = rustcraft_control::config::settings::engine(false);
    rustcraft_control::config::settings::load(
        &mut config,
        &std::env::args().collect::<Vec<_>>(),
        |k| std::env::var(k).ok(),
        rustcraft_control::config::user_path(),
    )
    .expect("stream benchmark configuration");
    let light_workers = config
        .effective(rustcraft_control::config::settings::LIGHT_WORKERS)
        .integer() as usize;
    let mut initial_lighting = InitialLightingScheduler::new(light_workers, 9);
    let initial_lighting_registry = Arc::new(simulation.registry.clone());
    let mut initial_lighting_submitted = 0usize;
    let mut initial_lighting_completed_total = 0usize;
    let mut initial_lighting_queue_wait_ms = 0.0;
    let mut initial_lighting_worker_ms = 0.0;
    let mut initial_light_direct_voxels = 0u64;
    let mut initial_light_emitters = 0u64;
    let mut initial_light_propagation_nodes = 0u64;
    let mut initial_light_queue_samples = Vec::new();
    let mut initial_light_service_samples = Vec::new();
    let mut max_initial_lighting_backlog = 0usize;
    let mut dirty = PersistenceDirtyTracker::default();
    let edited_columns = [
        (
            ChunkPos { x: -8, z: 0 },
            BlockPos {
                x: -127,
                y: 100,
                z: 1,
            },
        ),
        (
            ChunkPos { x: 4, z: 0 },
            BlockPos {
                x: 65,
                y: 100,
                z: 1,
            },
        ),
    ];
    let mut generated_edits = HashSet::new();
    let mut modified_edits = HashSet::new();
    let mut revisited_edits = HashSet::new();
    let mut load_hits = 0usize;
    let mut load_misses = 0usize;
    let mut generated_count = 0usize;
    let mut evictions = 0usize;
    let mut saved_before_evict = 0usize;
    let mut max_resident = 0usize;
    let mut max_pending = 0usize;
    let mut load_ms = 0.0;
    let mut generation_ms = 0.0;
    let mut publication_ms = 0.0;
    let mut publication_max_ms: f64 = 0.0;
    let mut max_lighting_queue = 0usize;
    let started = Instant::now();

    let mut path = (-8..=16).map(|x| ChunkPos { x, z: 0 }).collect::<Vec<_>>();
    path.extend((-5..=15).rev().map(|x| ChunkPos { x, z: 0 }));
    path.extend((-4..=16).map(|x| ChunkPos { x, z: 0 }));
    path.extend((-8..=15).rev().map(|x| ChunkPos { x, z: 0 }));

    for center in path.iter().copied() {
        let resident = simulation.world.column_positions().collect::<HashSet<_>>();
        let plan = residency.update(center, &resident, &HashSet::new());
        let requests = plan.requests;
        for request in requests.iter().copied() {
            residency.set_phase(request, ResidencyPhase::Loading);
            loads
                .submit(storage.clone(), request, resolver.clone())
                .expect("bounded stream benchmark load queue");
        }
        let load_metrics = loads.metrics();
        max_pending = max_pending.max(load_metrics.queued + load_metrics.in_flight);
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut loaded_done = 0usize;
        let mut generated_expected = 0usize;
        while loaded_done < requests.len() {
            for completion in loads.take_ready(requests.len()) {
                loaded_done += 1;
                load_ms += completion.load_ms;
                if !residency.is_current(completion.request) {
                    continue;
                }
                match completion.result.expect("stream chunk load succeeded") {
                    Some(column) => {
                        initial_lighting
                            .submit(rustcraft_runtime::lighting::InitialLightingRequest {
                                position: completion.request.position,
                                token: completion.request.token,
                                priority: residency.priority_key(completion.request.position),
                                default_block: simulation.world.empty_block(),
                                sections: column.sections,
                                registry: initial_lighting_registry.clone(),
                                persist_new: false,
                            })
                            .unwrap();
                        initial_lighting_submitted += 1;
                        max_initial_lighting_backlog =
                            max_initial_lighting_backlog.max(initial_lighting.outstanding());
                        load_hits += 1;
                    }
                    None => {
                        residency.set_phase(completion.request, ResidencyPhase::Generating);
                        generations
                            .request(
                                generator.clone(),
                                seed,
                                completion.request.position,
                                completion.request.token,
                            )
                            .expect("bounded stream benchmark generation queue");
                        let generation_metrics = generations.metrics();
                        let load_metrics = loads.metrics();
                        max_pending = max_pending.max(
                            load_metrics.queued
                                + load_metrics.in_flight
                                + generation_metrics.pending
                                + generation_metrics.in_flight,
                        );
                        generated_expected += 1;
                        load_misses += 1;
                    }
                }
            }
            if loaded_done < requests.len() {
                assert!(Instant::now() < deadline, "stream load timed out");
                std::thread::yield_now();
            }
        }
        let mut generated_done = 0usize;
        while generated_done < generated_expected {
            for result in generations.take_ready() {
                let request = rustcraft_world::ResidencyRequest {
                    position: result.position,
                    token: result.generation,
                };
                if !residency.is_current(request) {
                    continue;
                }
                let sections = result.sections.expect("stream generation succeeded");
                generation_ms += result.generation_ms;
                if edited_columns
                    .iter()
                    .any(|(position, _)| *position == result.position)
                {
                    generated_edits.insert(result.position);
                }
                initial_lighting
                    .submit(rustcraft_runtime::lighting::InitialLightingRequest {
                        position: result.position,
                        token: result.generation,
                        priority: residency.priority_key(result.position),
                        default_block: simulation.world.empty_block(),
                        sections,
                        registry: initial_lighting_registry.clone(),
                        persist_new: false,
                    })
                    .unwrap();
                initial_lighting_submitted += 1;
                max_initial_lighting_backlog =
                    max_initial_lighting_backlog.max(initial_lighting.outstanding());
                dirty.mark_dirty(result.position);
                generated_count += 1;
                generated_done += 1;
            }
            assert!(Instant::now() < deadline, "stream generation timed out");
            std::thread::yield_now();
        }

        let mut initial_lighting_completed = 0usize;
        while initial_lighting_completed < initial_lighting_submitted {
            let completed = initial_lighting.take_ready(8);
            initial_lighting_completed += completed.len();
            initial_lighting_completed_total += completed.len();
            for result in completed {
                assert!(
                    result.error.is_none(),
                    "initial lighting failed: {:?}",
                    result.error
                );
                initial_lighting_queue_wait_ms += result.queue_wait_ms;
                initial_lighting_worker_ms += result.worker_elapsed_ms;
                initial_light_queue_samples.push(result.queue_wait_ms);
                initial_light_service_samples.push(result.worker_elapsed_ms);
                initial_light_direct_voxels += result.work_counters.direct_voxels_scanned;
                initial_light_emitters += result.work_counters.emitters_found;
                initial_light_propagation_nodes += result.work_counters.propagation_queue_pops;
                let position = result.position;
                let token = result.token;
                if !residency.is_current(rustcraft_world::ResidencyRequest { position, token }) {
                    continue;
                }
                simulation
                    .publish_initial_lit_column(result, false)
                    .unwrap();
                max_lighting_queue =
                    max_lighting_queue.max(simulation.lighting.integration_columns().len());
                residency.published(position, token);
                if modified_edits.contains(&position) {
                    let (_, block) = edited_columns
                        .iter()
                        .find(|(edit_position, _)| *edit_position == position)
                        .expect("modified stream column has an edit definition");
                    assert_eq!(
                        simulation.world.state(*block).block,
                        STONE.id,
                        "edited block did not survive async residency reload"
                    );
                    revisited_edits.insert(position);
                }
            }
            if initial_lighting_completed < initial_lighting_submitted {
                assert!(Instant::now() < deadline, "initial lighting timed out");
                std::thread::yield_now();
            }
        }
        initial_lighting_submitted = 0;

        let (step_total, step_max) = drain_stream_lighting(&mut simulation);
        publication_ms += step_total;
        publication_max_ms = publication_max_ms.max(step_max);

        if generated_edits.contains(&center) && modified_edits.insert(center) {
            let (_, block) = edited_columns
                .iter()
                .find(|(position, _)| *position == center)
                .expect("generated stream column has an edit definition");
            simulation
                .world
                .set_state(*block, rustcraft_engine_core::BlockState::new(STONE.id));
            dirty.mark_dirty(center);
        }

        let resident = simulation.world.column_positions().collect::<HashSet<_>>();
        let eviction_plan = residency.update(center, &resident, &HashSet::new());
        for position in eviction_plan.evict {
            if dirty.is_dirty(position) {
                let token = dirty.begin_save(position).expect("dirty save token");
                let sections = simulation
                    .world
                    .section_positions()
                    .filter(|(candidate, _)| *candidate == position)
                    .filter_map(|(_, y)| {
                        simulation
                            .world
                            .section(position, y)
                            .cloned()
                            .map(|chunk| (y, chunk))
                    })
                    .collect::<Vec<_>>();
                let stored =
                    WorldStorage::encode_runtime_chunk(position, sections, resolver.as_ref())
                        .unwrap();
                storage.store_chunk(&stored).unwrap();
                dirty.complete_save(token, true);
                saved_before_evict += 1;
            }
            assert!(
                !dirty.is_dirty(position),
                "dirty column evicted before save"
            );
            let _ = simulation.remove_column_incremental_lighting(position);
            let (step_total, step_max) = drain_stream_lighting(&mut simulation);
            publication_ms += step_total;
            publication_max_ms = publication_max_ms.max(step_max);
            residency.evicted(position);
            evictions += 1;
        }
        max_resident = max_resident.max(simulation.world.chunk_count());
        let load_metrics = loads.metrics();
        let generation_metrics = generations.metrics();
        max_pending = max_pending.max(
            load_metrics.queued
                + load_metrics.in_flight
                + generation_metrics.pending
                + generation_metrics.in_flight,
        );
    }

    assert_eq!(generated_edits.len(), edited_columns.len());
    assert_eq!(modified_edits.len(), edited_columns.len());
    assert_eq!(revisited_edits.len(), edited_columns.len());
    for (position, block) in edited_columns {
        let persisted = storage
            .load_runtime_chunk(position, resolver.as_ref())
            .unwrap();
        let restored = persisted
            .iter()
            .find(|(y, _)| *y == block.y.div_euclid(16))
            .unwrap()
            .1
            .state((1, 4, 1));
        assert_eq!(
            restored.block, STONE.id,
            "edit did not survive eviction/reload"
        );
    }
    let generated_again = generator.generate(seed, ChunkPos { x: 4, z: 2 }).unwrap();
    let sample_chunk_state_hash = canonical_chunk_hash(&generated_again);
    let light_work_end = simulation.lighting.work_counters();
    let render_dirty_sections = simulation.take_dirty_sections().len();
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    storage.flush().unwrap();
    println!(
        "world-stream-bench profile={} centers={} generated={} loaded={} misses={} load_workers=1 generation_workers=1 initial_light_workers={} max_resident={} max_pending_inflight={} max_initial_light_backlog={} max_lighting_boundary_queue={} evicted={} save_before_evict={} load_ms={:.3} worker_generation_ms={:.3} initial_light_jobs={} initial_light_queue_wait_sum_ms={:.3} initial_light_queue_p50/p95/max_ms={:?} initial_light_worker_elapsed_sum_ms={:.3} initial_light_worker_p50/p95/max_ms={:?} initial_light_columns_per_sec={:.2} initial_light_direct_voxels={} initial_light_emitters={} initial_light_propagation_nodes={} publication_ms={:.3} publication_max_ms={:.3} boundary_light_scanned={} boundary={} boundary_qpush/pop={}/{} boundary_light_writes={} light_dirty_sections={} render_dirty_sections={} wall_ms={:.3} separated_revisit_edits={} negative_coordinates=ok generator=minecraft_b173:overworld:v2 sample_chunk_semantic_hash={}",
        if cfg!(debug_assertions) {
            "dev"
        } else {
            "release"
        },
        path.len(),
        generated_count,
        load_hits,
        load_misses,
        light_workers,
        max_resident,
        max_pending,
        max_initial_lighting_backlog,
        max_lighting_queue,
        evictions,
        saved_before_evict,
        load_ms,
        generation_ms,
        initial_lighting_completed_total,
        initial_lighting_queue_wait_ms,
        latency_summary(&mut initial_light_queue_samples),
        initial_lighting_worker_ms,
        latency_summary(&mut initial_light_service_samples),
        initial_lighting_completed_total as f64 / started.elapsed().as_secs_f64().max(0.001),
        initial_light_direct_voxels,
        initial_light_emitters,
        initial_light_propagation_nodes,
        publication_ms,
        publication_max_ms,
        light_work_end
            .direct_voxels_scanned
            .saturating_sub(light_work_start.direct_voxels_scanned),
        light_work_end
            .boundary_voxels_inspected
            .saturating_sub(light_work_start.boundary_voxels_inspected),
        light_work_end
            .propagation_queue_pushes
            .saturating_sub(light_work_start.propagation_queue_pushes),
        light_work_end
            .propagation_queue_pops
            .saturating_sub(light_work_start.propagation_queue_pops),
        light_work_end
            .light_writes
            .saturating_sub(light_work_start.light_writes),
        light_work_end
            .dirty_section_insertions
            .saturating_sub(light_work_start.dirty_section_insertions),
        render_dirty_sections,
        elapsed,
        revisited_edits.len(),
        sample_chunk_state_hash,
    );
    drop(loads);
    drop(generations);
    drop(storage);
    std::fs::remove_dir_all(root).unwrap();
}

fn latency_summary(samples: &mut [f64]) -> Option<[f64; 3]> {
    if samples.is_empty() {
        return None;
    }
    samples.sort_by(f64::total_cmp);
    let percentile = |p: f64| {
        let index = ((samples.len() as f64 * p).ceil() as usize)
            .saturating_sub(1)
            .min(samples.len() - 1);
        samples[index]
    };
    Some([percentile(0.5), percentile(0.95), *samples.last().unwrap()])
}

fn run_world_roundtrip() {
    use rustcraft_world::{ChunkGenerator, GenerationScheduler, WorldMetadata, WorldStorage};
    use std::{
        sync::Arc,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    let profile = rustcraft_minecraft_b173::compile_profile().unwrap();
    let generator: Arc<dyn ChunkGenerator> =
        Arc::new(rustcraft_minecraft_b173::worldgen::minecraft_overworld_v2());
    let root = std::env::temp_dir().join(format!(
        "rustcraft-m4-roundtrip-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let storage = WorldStorage::open(&root, "known_seed").unwrap();
    let metadata = WorldMetadata {
        seed: 731_173,
        game_id: profile.id.as_str().to_owned(),
        profile_fingerprint: profile_fingerprint(&profile),
        persistence_schema_version: rustcraft_world::PERSISTED_STATE_SCHEMA_VERSION,
        generator_id: generator.id().to_owned(),
        generator_version: generator.version(),
    };
    storage.store_metadata(&metadata).unwrap();

    let positions = [ChunkPos { x: -1, z: 0 }, ChunkPos { x: 0, z: 0 }];
    let mut scheduler = GenerationScheduler::new(2, 2);
    for (generation, pos) in positions.into_iter().enumerate() {
        scheduler
            .request(generator.clone(), metadata.seed, pos, generation as u64 + 1)
            .unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut generated = Vec::new();
    while generated.len() < positions.len() && Instant::now() < deadline {
        generated.extend(scheduler.take_ready());
        std::thread::yield_now();
    }
    assert_eq!(
        generated.len(),
        positions.len(),
        "generation workers did not finish"
    );
    let resolver = rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver;
    let mut world = World::new(BlockId(0));
    for column in generated {
        world
            .publish_column(column.position, column.sections.unwrap())
            .unwrap();
    }
    let edited = BlockPos { x: 2, y: 110, z: 3 };
    let state = rustcraft_engine_core::BlockState {
        block: STONE.id,
        variant: 23,
    };
    world.set_state(edited, state);
    let center_pos = ChunkPos { x: 0, z: 0 };
    let section_data = world
        .section_positions()
        .filter(|(position, _)| *position == center_pos)
        .filter_map(|(_, y)| {
            world
                .section(center_pos, y)
                .cloned()
                .map(|chunk| (y, chunk))
        })
        .collect::<Vec<_>>();
    storage
        .store_chunk(
            &WorldStorage::encode_runtime_chunk(center_pos, section_data, &resolver).unwrap(),
        )
        .unwrap();
    storage
        .store_chunk(
            &WorldStorage::encode_runtime_chunk(
                ChunkPos { x: -1, z: 0 },
                world
                    .section_positions()
                    .filter(|(position, _)| *position == ChunkPos { x: -1, z: 0 })
                    .filter_map(|(_, y)| {
                        world
                            .section(ChunkPos { x: -1, z: 0 }, y)
                            .cloned()
                            .map(|chunk| (y, chunk))
                    }),
                &resolver,
            )
            .unwrap(),
        )
        .unwrap();
    drop(storage);

    let reopened = WorldStorage::open(&root, "known_seed").unwrap();
    assert_eq!(reopened.load_metadata().unwrap(), metadata);
    let loaded = reopened.load_runtime_chunk(center_pos, &resolver).unwrap();
    let mut restored = World::new(BlockId(0));
    restored.publish_column(center_pos, loaded).unwrap();
    assert_eq!(
        restored.state(edited),
        state,
        "persisted player edit was lost"
    );
    assert!(reopened.chunk_exists(ChunkPos { x: -1, z: 0 }));

    // A saved chunk is loaded, not regenerated over its persisted edits. An unpersisted neighbor
    // remains a pure function of the same seed/coordinates.
    let neighbor_a = generator
        .generate(metadata.seed, ChunkPos { x: 1, z: 0 })
        .unwrap();
    let neighbor_b = generator
        .generate(metadata.seed, ChunkPos { x: 1, z: 0 })
        .unwrap();
    assert_eq!(
        canonical_chunk_hash(&neighbor_a),
        canonical_chunk_hash(&neighbor_b)
    );
    drop(reopened);
    std::fs::remove_dir_all(&root).unwrap();
    println!(
        "world-roundtrip: generated=2 workers=2 negative_coordinates=ok semantic_palette=ok edit_survived=ok neighboring_generation=deterministic"
    );
}

fn profile_fingerprint(profile: &rustcraft_game_api::CompiledGameProfile) -> String {
    profile.semantic_fingerprint().to_string()
}

fn canonical_chunk_hash(sections: &[(i32, rustcraft_engine_core::Chunk)]) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    for (y, chunk) in sections {
        hasher.update(&y.to_le_bytes());
        for state in chunk.states() {
            rustcraft_minecraft_b173::worldgen::update_semantic_state_hash(&mut hasher, *state);
        }
    }
    hasher.finalize()
}

fn run_survival() {
    let blocks = BlocksModule;
    let mut bootstrap = RuntimeBootstrap::new(Default::default());
    bootstrap.register_module(&blocks).unwrap();
    let profile = rustcraft_minecraft_b173::compile_profile().unwrap();
    let mut world = World::new(profile.default_state().block);
    world.set(
        BlockPos { x: 0, y: 0, z: 0 },
        rustcraft_minecraft_b173::blocks::GRASS.id,
    );
    world.set(
        BlockPos { x: 1, y: 1, z: 0 },
        rustcraft_minecraft_b173::blocks::LOG.id,
    );
    let mut sim = Simulation::new(world, bootstrap.registry, Vec3::new(1.5, 1.0, 1.0));
    sim.set_mode(GameMode::Survival);
    assert!(sim.break_block(BlockPos { x: 1, y: 1, z: 0 }));
    for _ in 0..30 {
        sim.step(AgentIntent::default(), 0.05);
    }
    assert_eq!(sim.items.len(), 0, "drop should be collected");
    let log = rustcraft_minecraft_b173::blocks::LOG.item.unwrap();
    sim.crafting_grid[0] = Some(rustcraft_runtime::inventory::ItemStack {
        item: log,
        count: 1,
        damage: 0,
    });
    assert!(sim.take_crafting_output());
    let planks = rustcraft_minecraft_b173::blocks::PLANKS.item.unwrap();
    assert!(
        sim.inventory
            .slots()
            .iter()
            .any(|s| s.is_some_and(|s| s.item == planks && s.count >= 4))
    );
    println!(
        "survival: drop_pickup=ok craft=log_to_planks inventory_slots={}",
        sim.inventory.occupied()
    );
}

fn run_smoke() {
    let payload = b"minecraft_b173:first-party:blocks:v1";
    let manifest = ContentManifest {
        packages: vec![PackageDescriptor {
            id: PackageId::parse("minecraft_b173:blocks").unwrap(),
            version: PackageVersion::new(1, 0, 0),
            kind: PackageKind::Data,
            hash: ContentHash::from_content_bytes(payload),
            dependencies: Vec::new(),
            targets: vec![PackageTarget::Server, PackageTarget::Bot],
        }],
    };
    assert!(manifest.validate_bytes(&PackageId::parse("minecraft_b173:blocks").unwrap(), payload));
    assert_eq!(manifest.for_target(PackageTarget::Bot).len(), 1);
    let mut bootstrap = RuntimeBootstrap::new(manifest);
    let blocks = BlocksModule;
    let profile = rustcraft_minecraft_b173::compile_profile().unwrap();
    let flat = rustcraft_minecraft_b173::flat_world_module(&profile);
    bootstrap
        .register_module(&blocks)
        .expect("block module registration");
    bootstrap
        .register_module(&flat)
        .expect("flat-world registration");
    let mut world = World::new(profile.default_state().block);
    flat.generate(&mut world, -16, 16, -16, 16);
    assert_eq!(world.get(BlockPos { x: 0, y: 0, z: 0 }), GRASS.id);
    let module_count = bootstrap.module_count();
    let mut simulation = Simulation::new(world, bootstrap.registry, Vec3::new(0.5, 3.0, 0.5));
    simulation
        .inventory
        .insert(STONE.item.unwrap(), 64, &simulation.registry);
    for _ in 0..100 {
        simulation.step(AgentIntent::default(), 0.02);
    }
    simulation.player.pitch = 0.9;
    let target = simulation.target().expect("ground in reach").block;
    let intents = [
        AgentIntent::default(),
        AgentIntent {
            movement: MoveIntent::default(),
            break_block: Some(target),
            ..Default::default()
        },
        AgentIntent {
            use_action: true,
            ..Default::default()
        },
    ];
    let mut bot = ScriptedBot::new(intents.to_vec());
    let before = simulation.observe(3);
    assert_eq!(before.api_version, rustcraft_bot_api::BOT_API_VERSION);
    assert!(!before.nearby_blocks.is_empty());
    run_controller(&mut simulation, &mut bot, 1, 0.02);
    run_controller(&mut simulation, &mut bot, 1, 0.02);
    assert_eq!(simulation.world.get(target), simulation.world.empty_block());
    let adjacent = simulation.target().expect("new ground target").adjacent;
    run_controller(&mut simulation, &mut bot, 1, 0.02);
    assert_eq!(simulation.world.get(adjacent), STONE.id);
    assert_eq!(simulation.inventory.held().unwrap().count, 63);
    assert_eq!(
        simulation.observe(1).inventory[0].as_ref().unwrap().count,
        63
    );
    println!(
        "smoke: modules={} chunks={} player_y={:.3} observations={} break/place=ok",
        module_count,
        simulation.world.chunk_count(),
        simulation.player.position.y,
        before.nearby_blocks.len()
    );
}

fn run_dx_scenario(path: &str) -> Result<(), String> {
    let mut registry = rustcraft_control::engine_registry();
    rustcraft_minecraft_b173::control::register_commands(&mut registry)?;
    let mut tools =
        rustcraft_scripting_rhai::DevTools::new(std::path::Path::new("scripts"), registry)?;
    let blocks = rustcraft_minecraft_b173::blocks::BlocksModule;
    let mut bootstrap = RuntimeBootstrap::new(ContentManifest { packages: vec![] });
    bootstrap
        .register_module(&blocks)
        .map_err(|e| format!("{e:?}"))?;
    let profile = rustcraft_minecraft_b173::compile_profile().map_err(|e| format!("{e:?}"))?;
    let mut world = World::new(profile.default_state().block);
    rustcraft_minecraft_b173::flat_world_module(&profile).generate(&mut world, -16, 16, -16, 16);
    let mut simulation = Simulation::new(world, bootstrap.registry, Vec3::new(0.5, 3., 0.5));
    let mut config = rustcraft_control::config::settings::engine(false);
    rustcraft_control::config::settings::load(
        &mut config,
        &std::env::args().collect::<Vec<_>>(),
        |k| std::env::var(k).ok(),
        rustcraft_control::config::user_path(),
    )?;
    let mut state = rustcraft_control::ControlState {
        config,
        leased: true,
        ..Default::default()
    };
    state.sync_config();
    let mut residency = rustcraft_world::WorldResidency::new(
        state
            .config
            .effective(rustcraft_control::config::settings::LOAD_RADIUS)
            .integer() as i32,
        state
            .config
            .effective(rustcraft_control::config::settings::RETAIN_RADIUS)
            .integer() as i32
            - state
                .config
                .effective(rustcraft_control::config::settings::LOAD_RADIUS)
                .integer() as i32,
    );
    {
        let mut host = rustcraft_minecraft_b173::control::MinecraftHost {
            simulation: &mut simulation,
            state: &mut state,
        };
        tools.start(path, &mut host)?;
    }
    loop {
        let mut host = rustcraft_minecraft_b173::control::MinecraftHost {
            simulation: &mut simulation,
            state: &mut state,
        };
        if let Some(bundle) = tools.advance(&mut host, false)? {
            println!("DX_RESULT {}", bundle.join("result.json").display());
            let result = &tools.scenario.as_ref().unwrap().result;
            return if result.status == "pass" {
                Ok(())
            } else {
                Err(format!("scenario {}: {:?}", result.status, result.error))
            };
        }
        while let Some(name) = state.captures.pop_front() {
            use rustcraft_control::Host;
            let snapshot = rustcraft_minecraft_b173::control::MinecraftHost {
                simulation: &mut simulation,
                state: &mut state,
            }
            .snapshot();
            let directory = std::path::PathBuf::from("target/captures").join(format!(
                "{name}-{}",
                tools.scenario.as_ref().unwrap().result.run_id
            ));
            tools.write_snapshot(directory, snapshot)?;
        }
        for _ in 0..state.fixed.take_ticks(1) {
            state.config.apply(
                rustcraft_control::config::Policy::NextTick,
                simulation.time,
                |c| {
                    residency
                        .set_radii(
                            c.effective(rustcraft_control::config::settings::LOAD_RADIUS)
                                .integer() as i32,
                            c.effective(rustcraft_control::config::settings::RETAIN_RADIUS)
                                .integer() as i32,
                        )
                        .map_err(str::to_owned)
                },
            )?;
            state.sync_config();
            simulation.step(state.intent.clone(), 0.05);
        }
    }
}

/// Bounded native/headless acceptance; no graphical dependency or world-save mutation.
fn run_config_smoke(arguments: &[String]) -> Result<(), String> {
    use rustcraft_control::{
        Action, Context, Host, Source,
        config::{self, Policy, settings as keys},
    };
    let mut c = config::settings::engine(false);
    config::settings::load(
        &mut c,
        arguments,
        |k| std::env::var(k).ok(),
        config::user_path(),
    )?;
    let base = c.effective(keys::LOAD_RADIUS).integer();
    let blocks = rustcraft_minecraft_b173::blocks::BlocksModule;
    let mut boot = RuntimeBootstrap::new(ContentManifest { packages: vec![] });
    boot.register_module(&blocks)
        .map_err(|e| format!("{e:?}"))?;
    let mut simulation = Simulation::new(
        World::new(rustcraft_minecraft_b173::blocks::AIR.id),
        boot.registry,
        Vec3::new(-0.5, 3., -0.5),
    );
    let mut state = rustcraft_control::ControlState {
        config: c,
        ..Default::default()
    };
    state.sync_config();
    let mut residency = rustcraft_world::WorldResidency::new(
        base as i32,
        state.config.effective(keys::RETAIN_RADIUS).integer() as i32 - base as i32,
    );
    let context = Context::developer(Source::ServerAdmin);
    for (load, retain) in [(6, 7), (3, 4), (12, 16), (3, 3)] {
        let mut host = rustcraft_minecraft_b173::control::MinecraftHost {
            simulation: &mut simulation,
            state: &mut state,
        };
        rustcraft_control::execute(
            &mut host,
            &context,
            &Action::ConfigSet(vec![
                (keys::LOAD_RADIUS.into(), load.to_string()),
                (keys::RETAIN_RADIUS.into(), retain.to_string()),
            ]),
        )?;
        state.config.apply(Policy::NextTick, simulation.time, |c| {
            residency
                .set_radii(
                    c.effective(keys::LOAD_RADIUS).integer() as i32,
                    c.effective(keys::RETAIN_RADIUS).integer() as i32,
                )
                .map_err(str::to_owned)
        })?;
        assert_eq!(residency.load_radius(), load);
        assert_eq!(residency.retain_radius(), retain);
        let center = ChunkPos { x: -1, z: -1 };
        let resident = std::collections::HashSet::from([center]);
        let plan = residency.update(center, &resident, &std::collections::HashSet::new());
        assert!(!plan.evict.contains(&center));
        assert_eq!(
            residency.desired_column_count(),
            ((load * 2 + 1) * (load * 2 + 1)) as usize
        );
    }
    let cadence = state.config.effective(keys::DIAGNOSTIC_MS).integer();
    {
        let mut host = rustcraft_minecraft_b173::control::MinecraftHost {
            simulation: &mut simulation,
            state: &mut state,
        };
        rustcraft_control::execute(
            &mut host,
            &context,
            &Action::ConfigSet(vec![(keys::DIAGNOSTIC_MS.into(), "50".into())]),
        )?;
        assert_eq!(host.state.diagnostics.cadence.as_millis(), 50);
        rustcraft_control::execute(
            &mut host,
            &context,
            &Action::ConfigReset(keys::DIAGNOSTIC_MS.into()),
        )?;
        assert_eq!(host.state.diagnostics.cadence.as_millis(), cadence as u128);
    }
    let before = state.config.effective(keys::LOAD_RADIUS).clone();
    let mut host = rustcraft_minecraft_b173::control::MinecraftHost {
        simulation: &mut simulation,
        state: &mut state,
    };
    assert!(
        rustcraft_control::execute(
            &mut host,
            &context,
            &Action::ConfigSet(vec![(keys::LOAD_RADIUS.into(), "13".into())])
        )
        .is_err()
    );
    assert_eq!(host.state.config.effective(keys::LOAD_RADIUS), &before);
    // Reset both runtime layers atomically, preserving file/env/CLI precedence.
    state.config.request(&[
        (keys::LOAD_RADIUS.into(), None),
        (keys::RETAIN_RADIUS.into(), None),
    ])?;
    state.config.apply(Policy::NextTick, 0, |c| {
        residency
            .set_radii(
                c.effective(keys::LOAD_RADIUS).integer() as i32,
                c.effective(keys::RETAIN_RADIUS).integer() as i32,
            )
            .map_err(str::to_owned)
    })?;
    assert_eq!(state.config.effective(keys::LOAD_RADIUS).integer(), base);
    let host = rustcraft_minecraft_b173::control::MinecraftHost {
        simulation: &mut simulation,
        state: &mut state,
    };
    let s = host.snapshot();
    assert_eq!(s.config["settings"][keys::LOAD_RADIUS]["effective"], base);
    println!(
        "C1_HEADLESS pass settings={} native/control/readback={} reset=precedence invalid=preserved negative_coordinates=ok",
        state.config.len(),
        base
    );
    Ok(())
}
