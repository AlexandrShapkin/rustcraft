//! Repeatable CPU-only M2 workload, no window or GPU required.
use crate::*;
use rustcraft_engine_core::BlockPos;
use rustcraft_minecraft_b173::blocks::{LAMP, STONE};
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
