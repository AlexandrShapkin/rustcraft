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
    } else if std::env::args().any(|arg| arg == "--world-stream-bench") {
        run_world_stream_bench();
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
        Arc::new(rustcraft_minecraft_b173::worldgen::minecraft_overworld());
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
    let light_work_start = simulation.lighting.work_counters();
    simulation.world.enforce_column_availability(true);
    let mut residency = WorldResidency::new(1, 1);
    // Radius one is a complete 3x3 Chebyshev square, so every stage must admit all nine critical
    // columns without letting an outer/speculative workload consume their bounded capacity.
    let mut loads = ChunkLoadScheduler::new(1, 9);
    let mut generations = GenerationScheduler::new(1, 9);
    let light_workers = std::env::var("RUSTCRAFT_LIGHT_WORKERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, 4);
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
                    Some(sections) => {
                        initial_lighting
                            .submit(rustcraft_runtime::lighting::InitialLightingRequest {
                                position: completion.request.position,
                                token: completion.request.token,
                                priority: residency.priority_key(completion.request.position),
                                default_block: simulation.world.empty_block(),
                                sections,
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
        "world-stream-bench profile={} centers={} generated={} loaded={} misses={} load_workers=1 generation_workers=1 initial_light_workers={} max_resident={} max_pending_inflight={} max_initial_light_backlog={} max_lighting_boundary_queue={} evicted={} save_before_evict={} load_ms={:.3} worker_generation_ms={:.3} initial_light_jobs={} initial_light_queue_wait_sum_ms={:.3} initial_light_queue_p50/p95/max_ms={:?} initial_light_worker_elapsed_sum_ms={:.3} initial_light_worker_p50/p95/max_ms={:?} initial_light_columns_per_sec={:.2} initial_light_direct_voxels={} initial_light_emitters={} initial_light_propagation_nodes={} publication_ms={:.3} publication_max_ms={:.3} boundary_light_scanned={} boundary={} boundary_qpush/pop={}/{} boundary_light_writes={} light_dirty_sections={} render_dirty_sections={} wall_ms={:.3} separated_revisit_edits={} negative_coordinates=ok sample_chunk_state_hash={}",
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
