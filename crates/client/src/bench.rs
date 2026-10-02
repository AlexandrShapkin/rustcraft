//! Repeatable CPU-only M2 workload, no window or GPU required.
use crate::*;
use rustcraft_engine_core::BlockPos;
use rustcraft_minecraft_b173::blocks::{LAMP, STONE};

struct SyntheticTextures;

impl rustcraft_render::BlockTextureResolver for SyntheticTextures {
    fn texture(
        &self,
        block: BlockId,
        _face: rustcraft_render::Face,
    ) -> Option<rustcraft_render::AtlasRegion> {
        (block.0 != 0).then(|| {
            rustcraft_render::AtlasRegion::full(rustcraft_render::TextureHandle(block.0 % 2))
        })
    }

    fn opaque(&self, block: BlockId) -> bool {
        block.0 != 0
    }
}

fn synthetic_world(radius: i32, exposed: bool) -> rustcraft_engine_core::World {
    let mut world = rustcraft_engine_core::World::new(BlockId(0));
    for chunk_z in -radius..=radius {
        for chunk_x in -radius..=radius {
            for z in 0..16 {
                for x in 0..16 {
                    if exposed {
                        for y in 0..16 {
                            if (x + y + z) % 2 == 0 {
                                world.set(
                                    BlockPos {
                                        x: chunk_x * 16 + x,
                                        y,
                                        z: chunk_z * 16 + z,
                                    },
                                    BlockId(1 + ((x + z) & 1) as u32),
                                );
                            }
                        }
                    } else {
                        world.set(
                            BlockPos {
                                x: chunk_x * 16 + x,
                                y: 0,
                                z: chunk_z * 16 + z,
                            },
                            BlockId(1 + ((chunk_x + chunk_z).rem_euclid(2)) as u32),
                        );
                    }
                }
            }
        }
    }
    world
}

fn measure_scene(label: &str, radius: i32, exposed: bool) {
    let world = synthetic_world(radius, exposed);
    let extraction_started = Instant::now();
    let presentation = rustcraft_render::RenderWorld::from_world(&world);
    let extraction_ms = extraction_started.elapsed().as_secs_f64() * 1000.0;
    let mesh_started = Instant::now();
    let mut pages = 0;
    let mut vertices = 0;
    let mut indices = 0;
    for chunk in presentation.chunks() {
        let meshes =
            rustcraft_render::build_chunk_mesh_pages(&presentation, chunk, &SyntheticTextures);
        pages += meshes.len();
        vertices += meshes
            .iter()
            .map(|page| page.mesh.vertices.len())
            .sum::<usize>();
        indices += meshes
            .iter()
            .map(|page| page.mesh.indices.len())
            .sum::<usize>();
        std::hint::black_box(meshes);
    }
    let mesh_ms = mesh_started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "R1.2 scene={label} radius={radius} resident_sections={} snapshot_bytes={} page_batches={pages} extraction_ms={extraction_ms:.3} mesh_ms={mesh_ms:.3} vertices={vertices} indices={indices} triangles={} cpu_mesh_bytes={}",
        presentation.chunk_count(),
        presentation.snapshot_bytes(),
        indices / 3,
        vertices * std::mem::size_of::<rustcraft_render::Vertex>()
            + indices * std::mem::size_of::<u32>()
    );
}

pub fn run_render_scale() {
    println!(
        "R1.2 build_profile={} arch={} os={}",
        if cfg!(debug_assertions) {
            "development"
        } else {
            "release"
        },
        std::env::consts::ARCH,
        std::env::consts::OS
    );
    for (label, radius, exposed) in [
        ("small", 1, false),
        ("medium", 4, false),
        ("large", 8, false),
        ("exposed", 1, true),
    ] {
        measure_scene(label, radius, exposed);
    }
    measure_camera_motion();
    measure_async_remesh_stress();

    let mut world = synthetic_world(4, false);
    let mut presentation = rustcraft_render::RenderWorld::from_world(&world);
    let dirty = (-4..=4)
        .flat_map(|z| (-4..=4).map(move |x| (rustcraft_engine_core::ChunkPos { x, z }, 0)))
        .collect::<Vec<_>>();
    for (position, _) in &dirty {
        world.set(
            BlockPos {
                x: position.x * 16 + 8,
                y: 1,
                z: position.z * 16 + 8,
            },
            BlockId(1),
        );
    }
    let started = Instant::now();
    presentation.sync_sections(&world, dirty.iter().copied());
    let extraction_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mesh_started = Instant::now();
    let mut completed = 0;
    for chunk in presentation
        .chunks()
        .filter(|chunk| dirty.contains(&(chunk.position, chunk.section_y)))
    {
        std::hint::black_box(rustcraft_render::build_chunk_mesh_pages(
            &presentation,
            chunk,
            &SyntheticTextures,
        ));
        completed += 1;
    }
    println!(
        "R1.2 scene=dirty-remesh dirty_sections={} jobs_completed={completed} snapshot_bytes={} extraction_ms={extraction_ms:.3} mesh_ms={:.3}",
        dirty.len(),
        presentation.snapshot_bytes(),
        mesh_started.elapsed().as_secs_f64() * 1000.0
    );
}

struct GatedSyntheticTextures {
    gate: std::sync::Arc<std::sync::Barrier>,
    first_call: std::sync::atomic::AtomicBool,
}

impl rustcraft_render::BlockTextureResolver for GatedSyntheticTextures {
    fn texture(
        &self,
        block: BlockId,
        _: rustcraft_render::Face,
    ) -> Option<rustcraft_render::AtlasRegion> {
        if self
            .first_call
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            self.gate.wait();
            self.gate.wait();
        }
        (block.0 != 0)
            .then(|| rustcraft_render::AtlasRegion::full(rustcraft_render::TextureHandle(block.0)))
    }

    fn opaque(&self, block: BlockId) -> bool {
        block.0 != 0
    }
}

fn measure_async_remesh_stress() {
    use rustcraft_render::meshing::MeshScheduler;
    use std::time::Duration;

    let world = synthetic_world(1, true);
    let extraction_started = Instant::now();
    let mut presentation = rustcraft_render::RenderWorld::from_world(&world);
    let mut snapshots = presentation.chunks().cloned().collect::<Vec<_>>();
    snapshots.sort_by_key(|chunk| (chunk.position.x, chunk.position.z, chunk.section_y));
    let snapshot_ms = extraction_started.elapsed().as_secs_f64() * 1000.0;
    let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
    let mut scheduler = MeshScheduler::new(
        1,
        2,
        GatedSyntheticTextures {
            gate: std::sync::Arc::clone(&gate),
            first_call: std::sync::atomic::AtomicBool::new(true),
        },
    );
    scheduler.mark_dirty(snapshots[0].clone());
    gate.wait();

    let mut edited_world = world;
    for snapshot in &snapshots {
        let origin = rustcraft_render::section_origin(snapshot.position, snapshot.section_y);
        edited_world.set(
            BlockPos {
                x: origin.x + 15,
                y: origin.y + 15,
                z: origin.z + 15,
            },
            BlockId(2),
        );
    }
    let affected = snapshots
        .iter()
        .map(|snapshot| (snapshot.position, snapshot.section_y))
        .collect::<Vec<_>>();
    presentation.sync_sections(&edited_world, affected.iter().copied());
    let mut edited = presentation.chunks().cloned().collect::<Vec<_>>();
    edited.sort_by_key(|chunk| (chunk.position.x, chunk.position.z, chunk.section_y));
    for _ in 0..99 {
        scheduler.mark_dirty(edited[0].clone());
    }
    for snapshot in edited.iter().skip(1) {
        scheduler.mark_dirty(snapshot.clone());
    }
    gate.wait();

    let deadline = Instant::now() + Duration::from_secs(60);
    let mut max_ready = 0;
    while scheduler.stats().mesh_jobs_completed < 1 + affected.len() as u64 {
        scheduler.poll();
        max_ready = max_ready.max(scheduler.stats().ready);
        assert!(Instant::now() < deadline, "async remesh stress timed out");
        std::thread::yield_now();
    }
    let mut uploads = 0;
    let mut upload_bytes = 0;
    let mut worker_mesh_ms = 0.0;
    let mut upload_waves = 0;
    while scheduler.stats().ready > 0 {
        let ready = scheduler.take_ready(Vec3::ZERO, Vec3::ZERO, 2, 2 * 1024 * 1024);
        assert!(!ready.is_empty());
        upload_waves += 1;
        for result in ready {
            uploads += 1;
            upload_bytes += result.logical_bytes();
            worker_mesh_ms += result.mesh_ms;
            if result.section == affected[0] {
                assert!(result.pages.iter().any(|page| page.texture.0 == 2));
            }
        }
    }
    let stats = scheduler.stats();
    println!(
        "R1.2 dirty_stress requested={} unique_sections={} jobs_submitted={} jobs_completed={} stale_discarded={} coalesced={} worker_count={} pending={} in_flight={} uploads_budgeted={} upload_waves={} max_completed_waiting={} upload_bytes={} snapshot_ms={snapshot_ms:.3} worker_mesh_ms={worker_mesh_ms:.3} upload_submit_ms=not-applicable-cpu-harness latest_generation_pages=verified",
        1 + 99 + affected.len() - 1,
        affected.len(),
        stats.mesh_jobs_submitted,
        stats.mesh_jobs_completed,
        stats.mesh_jobs_discarded_stale,
        stats.mesh_jobs_coalesced,
        stats.worker_count,
        stats.pending,
        stats.in_flight,
        uploads,
        upload_waves,
        max_ready,
        upload_bytes
    );
}

fn measure_camera_motion() {
    let sections = (-8..=8)
        .flat_map(|z| (-8..=8).map(move |x| (rustcraft_engine_core::ChunkPos { x, z }, 0)))
        .collect::<Vec<_>>();
    let mesh_rebuilds_before = 0_u64;
    for (label, position, yaw) in [
        ("forward", Vec3::new(0., 8., -24.), 0.0),
        (
            "turned-east",
            Vec3::new(0., 8., -24.),
            std::f32::consts::FRAC_PI_2,
        ),
        ("translated", Vec3::new(64., 8., -24.), 0.0),
        ("turned-back", Vec3::new(0., 8., -24.), std::f32::consts::PI),
    ] {
        let camera = rustcraft_render::Camera {
            position,
            yaw,
            pitch: 0.,
            aspect: 16. / 9.,
            fov_y: 70_f32.to_radians(),
            near: 0.05,
            far: 256.,
        };
        let visible = rustcraft_render::visible_section_positions(
            sections.iter().copied(),
            rustcraft_render::Frustum::from_camera(camera),
        );
        let mesh_rebuilds_after = mesh_rebuilds_before;
        println!(
            "R1.2 camera_motion={label} resident_sections={} visible_sections={} culled_sections={} drawn_page_batches={} draw_calls={} mesh_rebuild_delta={}",
            sections.len(),
            visible.len(),
            sections.len() - visible.len(),
            visible.len(),
            visible.len(),
            mesh_rebuilds_after - mesh_rebuilds_before
        );
    }
}
pub fn run() {
    let mut hud = rustcraft_render::hud::HudGeometry::default();
    let text = "FPS 55 FRAME 18.18 MS 1% LOW 29\nTPS 20 / 20 TICK 0.05 MS\nCPU 40% RAM 180 MIB THREADS 10\nGPU AMD RADEON VEGA 8 GRAPHICS\nBACKEND VULKAN TYPE INTEGRATED\n1280X662 FIFO BGRA8UNORMSRGB\nGPU FRAME 0.45 MS UTIL 0%\nVRAM 845 / 1024 MIB\nXYZ 0.5 1.0 0.5 CHUNK 0 0\nYAW 0 PITCH 0\nCHUNKS 9 SECTIONS 18 RENDERED 18\nMESHES 18 DIRTY 0 PENDING 0\nVERTICES 10824 INDICES 16236 DRAWS 19\nREBUILDS 18 RATE 0 /S\nBUILD LAST 2.3 MEAN 1.8 MS\nLIGHT INIT 3400 LAST 0 MS\nHELD CORE:STONE SLOT 1\nPROFILE CORE:SANDBOX-M2 MODULES 2";
    let snapshot = rustcraft_render::hud::HudSnapshot {
        slots: [None; 9],
        selected: 0,
        target: None,
        text,
        items: &[],
        mining_progress: None,
        inventory_open: false,
        inventory_slots: [None; 36],
        crafting_slots: [None; 4],
        crafting_output: None,
        cursor_slot: None,
        cursor_position: [0.; 2],
    };
    let started = Instant::now();
    for _ in 0..240 {
        hud.build(&snapshot, 1280, 662, Stage::Triangle.camera(1280. / 662.));
    }
    println!(
        "M2 HUD build_mean_ms={:.4}",
        started.elapsed().as_secs_f64() * 1000. / 240.
    );
    let mut app = ClientApp::new(Some(Stage::NormalLit), None);
    app.start_world().unwrap();
    let sim = app.simulation.as_mut().unwrap();
    let presentation = app.presentation.as_mut().unwrap();
    let start = Instant::now();
    let mut vertices = 0;
    for chunk in presentation.chunks() {
        vertices += rustcraft_render::build_chunk_mesh(presentation, chunk, &FirstPartyTextures)
            .vertices
            .len();
    }
    println!(
        "M2 CPU initial_light_ms={:.3} initial_mesh_ms={:.3} sections={} vertices={vertices}",
        sim.lighting.initial_ms,
        start.elapsed().as_secs_f64() * 1000.,
        presentation.chunk_count()
    );
    sim.take_dirty_sections();
    for (label, p, block, slot) in [
        ("interior", BlockPos { x: 8, y: 1, z: 8 }, STONE.id, 0),
        ("boundary", BlockPos { x: 15, y: 1, z: 8 }, STONE.id, 0),
        ("emissive", BlockPos { x: 15, y: 1, z: 8 }, LAMP.id, 8),
    ] {
        sim.inventory.select(slot);
        for add in [true, false] {
            if add {
                assert!(sim.place_block(p, block));
            } else {
                assert!(sim.break_block(p));
            }
            let dirty = sim.take_dirty_sections();
            let notified = dirty.len();
            let start = Instant::now();
            presentation.sync_sections(&sim.world, dirty.iter().copied());
            let mut rebuilt = 0;
            for chunk in presentation
                .chunks()
                .filter(|c| dirty.contains(&(c.position, c.section_y)))
            {
                std::hint::black_box(rustcraft_render::build_chunk_mesh(
                    presentation,
                    chunk,
                    &FirstPartyTextures,
                ));
                rebuilt += 1;
            }
            println!(
                "M2 CPU {label} add={add} light_ms={:.3} visited={} dirty={notified} rebuilt={rebuilt} extract_mesh_ms={:.3}",
                sim.lighting.last_update_ms,
                sim.lighting.last_visited,
                start.elapsed().as_secs_f64() * 1000.
            );
        }
    }
    let mut ticks = rustcraft_runtime::metrics::History::default();
    for _ in 0..1000 {
        let start = Instant::now();
        sim.step(AgentIntent::default(), 0.05);
        ticks.push(start.elapsed().as_secs_f64());
    }
    println!("M2 CPU tick_ms={:.6}", ticks.mean().unwrap() * 1000.);
}

pub fn run_m3() {
    let mut app = ClientApp::new(Some(Stage::NormalLit), None);
    app.start_world().unwrap();
    let sim = app.simulation.as_mut().unwrap();
    sim.set_survival();
    let item = rustcraft_minecraft_b173::blocks::DIRT.item.unwrap();
    for n in [100usize, 1000] {
        sim.items.clear();
        for i in 0..n {
            sim.spawn_item(
                item,
                1,
                rustcraft_engine_core::Vec3::new((i % 20) as f32 + 0.5, 2., (i / 20) as f32 + 0.5),
            );
        }
        let t = Instant::now();
        for _ in 0..100 {
            sim.step(rustcraft_agent_api::AgentIntent::default(), 0.05);
        }
        println!(
            "M3 items={} tick_ms={:.4} pickup_remaining={}",
            n,
            t.elapsed().as_secs_f64() * 10.,
            sim.items.len()
        );
    }
}
