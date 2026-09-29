//! Periodic read-only presentation snapshot. HUD code never queries simulation or OS state.
use crate::gpu_metrics;
use rustcraft_render::Renderer;
use rustcraft_runtime::{
    Simulation,
    metrics::{Metrics, ProcessSnapshot},
};

pub struct DebugMetricsSnapshot {
    pub fps: Option<f64>,
    pub frame_ms: Option<f64>,
    pub low: Option<f64>,
    pub tps: Option<f64>,
    pub tick_ms: Option<f64>,
    pub last_tick_ms: Option<f64>,
    pub process: ProcessSnapshot,
    pub gpu_ms: Option<f64>,
    pub gpu_util: Option<f64>,
    pub vram_used: Option<f64>,
    pub vram_total: Option<f64>,
    pub text: String,
}
fn value(v: Option<f64>) -> String {
    v.map_or_else(|| "N/A".into(), |v| format!("{v:.2}"))
}
impl DebugMetricsSnapshot {
    pub fn collect(
        sim: &Simulation,
        renderer: &Renderer,
        metrics: &Metrics,
        process: &ProcessSnapshot,
        gpu: Option<&gpu_metrics::Snapshot>,
    ) -> Self {
        let mut s = Self {
            fps: metrics.frames.fps(),
            frame_ms: metrics.frames.mean().map(|v| v * 1000.),
            low: metrics.frames.low(),
            tps: metrics.tps,
            tick_ms: metrics.ticks.mean().map(|v| v * 1000.),
            last_tick_ms: metrics.ticks.last().map(|v| v * 1000.),
            process: process.clone(),
            gpu_ms: renderer.gpu_ms(),
            gpu_util: gpu.and_then(|g| g.utilization),
            vram_used: gpu.and_then(|g| g.used_mib),
            vram_total: gpu.and_then(|g| g.total_mib),
            text: String::new(),
        };
        let p = sim.player.position;
        let held = sim
            .inventory
            .held()
            .and_then(|s| sim.registry.item(s.item))
            .map_or("EMPTY", |i| i.name);
        s.text = format!(
            "RUSTCRAFT M3 LOCAL - UPTIME {:.0} S\nFPS {} FRAME {} MS 1% LOW {}\nTPS {} / 20 TICK {} MS LAST {}\nSTEPS {} CATCHUP {}\nPROCESS CPU {}% RSS {} MIB THREADS {} CPUS {}\nGPU {}\nBACKEND {:?} TYPE {:?}\nPRESENT {:?} (VSYNC) {}\nGPU FRAME {} MS\nDEVICE GPU BUSY {}%\nDEVICE VRAM {} / {} MIB\nXYZ {:.2} {:.2} {:.2} CHUNK {} {} SECTION {}\nYAW {:.2} PITCH {:.2}\nCHUNKS {} SECTIONS {} RENDERED {}\nMESHES {} DIRTY {} PENDING {}\nENTITIES {} ITEMS {}\nINVENTORY {}/36\nMINING {}\nVERTICES {} INDICES {} DRAWS {}\nREBUILDS {} RATE {} /S\nBUILD LAST {} MEAN {} MS\nLIGHT INIT {:.2} LAST {:.2} MS\nHELD {} DURABILITY {} SLOT {}\nPROFILE CORE:SANDBOX-M3 MODULES 2",
            metrics.uptime.elapsed().as_secs_f64(),
            value(s.fps),
            value(s.frame_ms),
            value(s.low),
            value(s.tps),
            value(s.tick_ms),
            value(s.last_tick_ms),
            metrics.steps,
            metrics.catch_up,
            value(s.process.cpu),
            value(s.process.rss_mib),
            s.process
                .threads
                .map_or_else(|| "N/A".into(), |v| v.to_string()),
            s.process
                .logical_cpus
                .map_or_else(|| "N/A".into(), |v| v.to_string()),
            renderer.adapter_info.name,
            renderer.adapter_info.backend,
            renderer.adapter_info.device_type,
            renderer.config_present_mode(),
            renderer.surface_description(),
            value(s.gpu_ms),
            value(s.gpu_util),
            value(s.vram_used),
            value(s.vram_total),
            p.x,
            p.y,
            p.z,
            (p.x.floor() as i32).div_euclid(16),
            (p.z.floor() as i32).div_euclid(16),
            (p.y.floor() as i32).div_euclid(16),
            sim.player.yaw,
            sim.player.pitch,
            sim.world.chunk_count(),
            sim.world.section_positions().count(),
            renderer.rendered_sections(),
            renderer.mesh_count(),
            sim.dirty_section_count(),
            sim.dirty_section_count(),
            sim.items.len(),
            sim.items.len(),
            sim.inventory.occupied(),
            sim.mining
                .map_or_else(|| "N/A".into(), |m| format!("{:.0}%", m.progress * 100.)),
            renderer.vertices,
            renderer.indices,
            renderer.draw_calls(),
            renderer.mesh_rebuilds,
            value(metrics.rebuilds_per_second),
            value(metrics.meshes.last().map(|v| v * 1000.)),
            value(metrics.meshes.mean().map(|v| v * 1000.)),
            sim.lighting.initial_ms,
            sim.lighting.last_update_ms,
            held,
            sim.inventory
                .held()
                .map_or_else(|| "N/A".into(), |s| s.damage.to_string()),
            sim.inventory.selected() + 1
        );
        s
    }
}
