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
    if std::env::args().any(|argument| argument == "--version") {
        println!("{}", rustcraft_build_info::identity());
        return;
    }
    rustcraft_minecraft_b173::validate_package()
        .expect("minecraft_b173 must register through the public Game API");
    let smoke = std::env::args().any(|arg| arg == "--smoke");
    if std::env::args().any(|arg| arg == "--world-roundtrip") {
        run_world_roundtrip();
    } else if std::env::args().any(|arg| arg == "--worldgen-bench") {
        run_worldgen_bench();
    } else if std::env::args().any(|arg| arg == "--persistence-bench") {
        run_persistence_bench();
    } else if smoke {
        run_smoke();
    } else if std::env::args().any(|arg| arg == "--survival") {
        run_survival();
    } else {
        println!("rustcraft server bootstrap; run with --smoke for the headless scenario");
    }
}

fn run_worldgen_bench() {
    use rustcraft_world::{ChunkGenerator, GenerationScheduler};
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    let generator: Arc<dyn ChunkGenerator> =
        Arc::new(rustcraft_minecraft_b173::worldgen::minecraft_overworld());
    let positions = (-2..2)
        .flat_map(|z| (-2..2).map(move |x| ChunkPos { x, z }))
        .collect::<Vec<_>>();
    let mut scheduler = GenerationScheduler::new(4, 4);
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
        "worldgen benchmark timed out"
    );
    generated.sort_by_key(|column| (column.position.x, column.position.z));
    let mut hash = blake3::Hasher::new();
    let mut section_count = 0usize;
    let mut estimated_bytes = 0usize;
    for column in &generated {
        hash.update(&column.position.x.to_le_bytes());
        hash.update(&column.position.z.to_le_bytes());
        let mut sections = column.sections.as_ref().unwrap().iter().collect::<Vec<_>>();
        sections.sort_by_key(|(y, _)| *y);
        for (y, chunk) in sections {
            section_count += 1;
            estimated_bytes += std::mem::size_of_val(chunk.states());
            hash.update(&y.to_le_bytes());
            for state in chunk.states() {
                hash.update(&state.block.0.to_le_bytes());
                hash.update(&state.variant.to_le_bytes());
            }
        }
    }
    let elapsed = started.elapsed().as_secs_f64();
    let worker_ms = scheduler.metrics().generation_total_ms;
    println!(
        "worldgen-bench profile={} workers=4 seed=731173 chunks={} sections={} wall_ms={:.3} worker_generation_ms={:.3} chunks_per_s={:.2} estimated_authoritative_bytes={} hash={}",
        if cfg!(debug_assertions) {
            "dev"
        } else {
            "release"
        },
        positions.len(),
        section_count,
        elapsed * 1000.0,
        worker_ms,
        positions.len() as f64 / elapsed,
        estimated_bytes,
        hash.finalize().to_hex()
    );
}

fn run_persistence_bench() {
    use rustcraft_world::{ChunkGenerator, WorldStorage};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};
    let generator = rustcraft_minecraft_b173::worldgen::minecraft_overworld();
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

fn run_world_roundtrip() {
    use rustcraft_world::{ChunkGenerator, GenerationScheduler, WorldMetadata, WorldStorage};
    use std::{
        sync::Arc,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    let profile = rustcraft_minecraft_b173::compile_profile().unwrap();
    let generator: Arc<dyn ChunkGenerator> =
        Arc::new(rustcraft_minecraft_b173::worldgen::minecraft_overworld());
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
            hasher.update(&state.block.0.to_le_bytes());
            hasher.update(&state.variant.to_le_bytes());
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
