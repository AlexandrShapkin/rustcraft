//! Bounded field harness around the normal client, not a separate simulation/input path.
use super::{ClientApp, devtools::ClientHost, presentation::Ledger};
use rustcraft_control::{Action, Context, Source};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
const PHASES: [&str; 6] = [
    "stationary",
    "pan",
    "walk",
    "walk_pan",
    "fast_pan",
    "walk_fast_pan",
];
// Normal runtime sensitivity is 0.002 radians/count (runtime::apply_intent).
// 1500 counts/s represents a roughly 172-degree human turn in one second.
const FAST_MOUSE_COUNTS_PER_SECOND: f64 = 1500.;
const CAP: usize = 32768;

fn output() -> PathBuf {
    std::env::var_os("RUSTCRAFT_F1_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/f1"))
}
fn write(name: &str, value: &Value) -> Result<(), String> {
    std::fs::create_dir_all(output()).map_err(|e| e.to_string())?;
    std::fs::write(
        output().join(name),
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
pub(super) fn write_failure(error: &str) -> Result<(), String> {
    write(
        "summary.json",
        &json!({"schema_version":1,"status":"failed", "error":error}),
    )
}
fn adapter(info: &wgpu::AdapterInfo) -> Value {
    json!({"name":info.name,"vendor":info.vendor,"device":info.device,
        "device_type":format!("{:?}",info.device_type),"backend":format!("{:?}",info.backend),
        "driver":info.driver,"driver_info":info.driver_info})
}
fn representative(info: &wgpu::AdapterInfo) -> Result<(), String> {
    if info.vendor != 0x1002
        || info.backend != wgpu::Backend::Vulkan
        || !format!("{} {}", info.driver, info.driver_info)
            .to_lowercase()
            .contains("radv")
    {
        return Err(format!(
            "F1 requires AMD/RADV/Vulkan; actual adapter: {}",
            adapter(info)
        ));
    }
    Ok(())
}
pub(super) fn probe() -> Result<(), String> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });
    let result = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .map_err(|e| format!("no Vulkan adapter: {e}"));
    match result {
        Ok(a) => {
            let info = a.get_info();
            write("adapter.json", &adapter(&info))?;
            if let Err(error) = representative(&info) {
                write_failure(&error)?;
                return Err(error);
            }
            println!("F1_ADAPTER {}", adapter(&info));
            Ok(())
        }
        Err(error) => {
            write_failure(&error)?;
            Err(error)
        }
    }
}

pub(super) struct Trace {
    pub enabled: bool,
    origin: Instant,
    last_frame: Option<Instant>,
    frames: Vec<(f64, f64, f64)>,
    workers: Vec<Value>,
    main: Vec<Value>,
    budgets: Vec<Value>,
    dropped: usize,
}
impl Default for Trace {
    fn default() -> Self {
        Self {
            enabled: false,
            origin: Instant::now(),
            last_frame: None,
            frames: vec![],
            workers: vec![],
            main: vec![],
            budgets: vec![],
            dropped: 0,
        }
    }
}
impl Trace {
    fn ms(&self, at: Instant) -> f64 {
        at.saturating_duration_since(self.origin).as_secs_f64() * 1000.
    }
    pub fn frame(&mut self, end: Instant, cpu_ms: f64) {
        if !self.enabled {
            return;
        }
        if let Some(start) = self.last_frame.replace(end) {
            if self.frames.len() < CAP {
                self.frames.push((self.ms(start), self.ms(end), cpu_ms));
            } else {
                self.dropped += 1;
            }
        }
    }
    pub fn worker(
        &mut self,
        kind: &str,
        start: Instant,
        end: Instant,
        publication_ms: f64,
        success: bool,
    ) {
        if !self.enabled {
            return;
        }
        if self.workers.len() < CAP {
            self.workers.push(json!({"kind":kind,
            "start_ms":self.ms(start),"end_ms":self.ms(end),"publication_ms":publication_ms,"success":success}));
        } else {
            self.dropped += 1;
        }
    }
    pub fn main_service(&mut self, kind: &str, start: Instant, end: Instant) {
        if !self.enabled {
            return;
        }
        if self.main.len() < CAP {
            self.main.push(json!({"kind":kind,
            "start_ms":self.ms(start),"end_ms":self.ms(end)}));
        } else {
            self.dropped += 1;
        }
    }
    pub fn fixed_budget(&mut self, at: Instant, due: u64, executed: u32, dropped_seconds: f64) {
        if !self.enabled || (due == 0 && executed == 0 && dropped_seconds == 0.) {
            return;
        }
        if self.budgets.len() < CAP {
            self.budgets.push(json!({"at_ms":self.ms(at),
            "due":due,"executed":executed,"dropped_seconds":dropped_seconds}));
        } else {
            self.dropped += 1;
        }
    }
    fn correlation(&self, start: f64, end: f64, long_ms: f64) -> Value {
        let overlaps = |a: f64, b: f64, e: &Value| {
            a < e["end_ms"].as_f64().unwrap() && b > e["start_ms"].as_f64().unwrap()
        };
        let long: Vec<_> = self
            .frames
            .iter()
            .filter(|(a, b, _)| *a < end && *b > start && b - a > long_ms)
            .collect();
        let main_cost: Vec<f64> = self
            .main
            .iter()
            .filter(|e| overlaps(start, end, e))
            .map(|e| e["end_ms"].as_f64().unwrap() - e["start_ms"].as_f64().unwrap())
            .collect();
        let main_in_long: Vec<f64> = long
            .iter()
            .map(|&&(a, b, _)| {
                self.main
                    .iter()
                    .filter(|e| overlaps(a, b, e))
                    .map(|e| {
                        b.min(e["end_ms"].as_f64().unwrap())
                            - a.max(e["start_ms"].as_f64().unwrap())
                    })
                    .sum()
            })
            .collect();
        json!({"long_frame_threshold_ms":long_ms,"long_frame_count":long.len(),
            "long_frames_overlapping_workers":long.iter().filter(|&&(a,b,_)|
                self.workers.iter().any(|e|overlaps(*a,*b,e))).count(),
            "long_frames_overlapping_main_persistence":long.iter().filter(|&&(a,b,_)|
                self.main.iter().any(|e|overlaps(*a,*b,e))).count(),
            "main_persistence_service_ms":distribution(main_cost),
            "main_persistence_time_inside_long_frames_ms":distribution(main_in_long),
            "interpretation":"worker overlap is not evidence of main-thread blocking; main service cost is measured separately"})
    }
    fn summary(&self, long_ms: f64) -> Value {
        let long: Vec<_> = self
            .frames
            .iter()
            .filter(|(a, b, _)| b - a > long_ms)
            .collect();
        let overlap = |a: f64, b: f64, e: &Value| {
            a < e["end_ms"].as_f64().unwrap() && b > e["start_ms"].as_f64().unwrap()
        };
        let events: Vec<_> = long.iter().map(|&&(a,b,cpu)| json!({"start_ms":a,"end_ms":b,
            "frame_cpu_ms":cpu,"worker_overlap":self.workers.iter().filter(|e|overlap(a,b,e)).collect::<Vec<_>>(),
            "main_persistence_overlap":self.main.iter().filter(|e|overlap(a,b,e)).collect::<Vec<_>>()})).collect();
        let publication: Vec<f64> = self
            .workers
            .iter()
            .filter(|e| e["kind"] == "player" && e["success"] == true)
            .map(|e| e["publication_ms"].as_f64().unwrap())
            .collect();
        json!({"schema_version":1,"clock":"std::time::Instant, milliseconds since client construction",
            "long_frame_threshold_ms":long_ms,"long_frames":events,"frame_intervals":self.frames,
            "frame_interval_columns":["start_ms","end_ms","renderer_cpu_ms"],
            "worker_checkpoints":self.workers,"main_persistence_services":self.main,"fixed_step_budgets":self.budgets,
            "player_publication_ms":distribution(publication),"dropped_trace_events":self.dropped,
            "publication_semantics":"sync_ms/durability_ms include file creation, write-excluded publication, replacement, directory sync and cleanup; not isolated fsync",
            "interpretation":"worker overlap is correlation only; main-thread service duration is separately measured; neither proves physical display stall"})
    }
}
fn distribution(mut values: Vec<f64>) -> Value {
    if values.is_empty() {
        return json!({"count":0});
    }
    values.sort_by(f64::total_cmp);
    let percentile = |p: f64| values[((values.len() - 1) as f64 * p).ceil() as usize];
    json!({"count":values.len(),"p50":percentile(0.5),"p95":percentile(0.95),
        "p99":percentile(0.99),"max":values.last(),"mean":values.iter().sum::<f64>()/values.len() as f64})
}

pub(super) struct Campaign {
    interactive: bool,
    start: Instant,
    phase_start: Option<Instant>,
    last: Instant,
    phase: usize,
    origin: [f32; 3],
    initialized: bool,
    samples: Vec<Value>,
    counters: (u64, u64, f64),
}
impl Campaign {
    pub fn new(interactive: bool) -> Self {
        Self {
            interactive,
            start: Instant::now(),
            phase_start: None,
            last: Instant::now(),
            phase: 0,
            origin: [0.; 3],
            initialized: false,
            samples: vec![],
            counters: (0, 0, 0.),
        }
    }
    fn turn(&mut self, app: &mut ClientApp) -> Result<bool, String> {
        if self.start.elapsed() > Duration::from_secs(420) {
            return Err("F1 workload timed out".into());
        }
        if let Some(r) = &app.renderer {
            representative(&r.adapter_info)?;
        }
        if !app.player_control_enabled {
            return Ok(false);
        }
        if !self.initialized && !self.interactive {
            let p = app
                .simulation
                .as_ref()
                .ok_or("simulation unavailable")?
                .player
                .position;
            self.origin = [p.x.floor(), 96., p.z.floor()];
            let ctx = Context::developer(Source::Scenario);
            for x in -16..=16 {
                for z in -16..=16 {
                    rustcraft_control::execute(
                        &mut ClientHost { app },
                        &ctx,
                        &Action::SetBlock {
                            position: [self.origin[0] as i32 + x, 95, self.origin[2] as i32 + z],
                            key: "minecraft_b173:stone".into(),
                            variant: 0,
                        },
                    )?;
                }
            }
            rustcraft_control::execute(
                &mut ClientHost { app },
                &ctx,
                &Action::Teleport(self.origin),
            )?;
            let sim = app.simulation.as_mut().unwrap();
            sim.player.yaw = 0.;
            sim.player.pitch = 0.;
            app.debug = false;
            self.initialized = true;
            return Ok(false);
        }
        if self.phase_start.is_none() {
            if app.lifetime_ledger()["core_ready"] != true
                || !app.mesh_scheduler.is_idle()
                || !app.snapshot_dirty_sections.is_empty()
                || !app.mesh_dirty_sections.is_empty()
            {
                return Ok(false);
            }
            if self.interactive && !app.controller.captured {
                return Ok(false);
            }
            let _ = app.presentation_observation();
            let old = std::mem::take(&mut app.presentation_timing);
            app.presentation_timing = Ledger::recording(old.monitor, old.target_ms);
            self.counters = (
                app.responsiveness.executed_ticks,
                app.responsiveness.dropped_ticks,
                app.responsiveness.dropped_seconds,
            );
            self.phase_start = Some(Instant::now());
            self.last = Instant::now();
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        if !self.interactive {
            app.controller.captured = true;
            app.controller.forward = if matches!(self.phase, 2 | 3 | 5) {
                1.
            } else {
                0.
            };
            if self.phase == 2 {
                let sim = app.simulation.as_mut().unwrap();
                if (sim.player.position.z - self.origin[2]).abs() > 12. {
                    sim.player.yaw = if sim.player.position.z > self.origin[2] {
                        std::f32::consts::PI
                    } else {
                        0.
                    };
                }
            }
            if matches!(self.phase, 1 | 3) {
                app.ingest_mouse((-dt * 0.65 / 0.002, 0.), now);
            }
            if matches!(self.phase, 4 | 5) {
                app.ingest_mouse((-dt * FAST_MOUSE_COUNTS_PER_SECOND, 0.), now);
            }
        }
        if self.phase_start.unwrap().elapsed()
            < Duration::from_secs(if self.interactive { 45 } else { 30 })
        {
            return Ok(false);
        }
        let mut sample = app.presentation_observation();
        sample["phase"] = json!(if self.interactive {
            "interactive"
        } else {
            PHASES[self.phase]
        });
        sample["mouse_counts_per_second"] = if self.interactive {
            Value::Null
        } else {
            json!(match self.phase {
                1 | 3 => 325.,
                4 | 5 => FAST_MOUSE_COUNTS_PER_SECOND,
                _ => 0.,
            })
        };
        sample["mouse_sensitivity_radians_per_count"] = json!(0.002);
        sample["start_ms"] = json!(app.f1_trace.ms(self.phase_start.unwrap()));
        sample["end_ms"] = json!(app.f1_trace.ms(now));
        sample["configuration"] = app.control_state.config.snapshot();
        sample["fixed_step"] = json!({"executed":app.responsiveness.executed_ticks-self.counters.0,
            "dropped":app.responsiveness.dropped_ticks-self.counters.1,
            "dropped_seconds":app.responsiveness.dropped_seconds-self.counters.2,
            "last_catch_up":app.metrics.catch_up});
        sample["window_pixels"] = json!(
            app.window
                .as_ref()
                .map(|w| [w.inner_size().width, w.inner_size().height])
        );
        self.samples.push(sample);
        self.phase += 1;
        self.phase_start = None;
        if !self.interactive && self.phase < PHASES.len() {
            return Ok(false);
        }
        app.controller.forward = 0.;
        app.controller.look = Default::default();
        let r = app.renderer.as_ref().ok_or("renderer unavailable")?;
        let threshold = app
            .presentation_timing
            .target_ms
            .map_or(33.333, |ms| ms * 2.5);
        for sample in &mut self.samples {
            sample["checkpoint_frame_correlation"] = app.f1_trace.correlation(
                sample["start_ms"].as_f64().unwrap(),
                sample["end_ms"].as_f64().unwrap(),
                threshold,
            );
        }
        let trace = app.f1_trace.summary(threshold);
        write("timeline.json", &trace)?;
        if app.f1_trace.dropped != 0 {
            return Err("F1 trace capacity exceeded".into());
        }
        if !self.interactive
            && trace["player_publication_ms"]["count"]
                .as_u64()
                .unwrap_or(0)
                < 20
        {
            return Err(
                "F1 requires at least 20 successful player checkpoints for tail analysis".into(),
            );
        }
        write(
            "summary.json",
            &json!({"schema_version":1,"status":"measured","acceptance":"pending review of hardware evidence; not physical-display acceptance",
            "mode":if self.interactive {"interactive"} else {"automated"},
            "adapter":adapter(&r.adapter_info),"build_profile":if cfg!(debug_assertions){"dev"}else{"release"},
            "build_identity":rustcraft_build_info::identity(),"world_seed":app.world_seed,"generator_version":app.stream_generator.as_ref().map(|g|g.version()),"principal":app.session.principal,"role":app.session.role,
            "input":if self.interactive {"owner-controlled normal DeviceEvent mouse and WindowEvent keyboard; subjective acceptance requires owner observation"} else {"harness drives normal LocalHumanController and native mouse ingestion; production winit payload router tested separately without desktop injection"},
            "phases":self.samples,"timeline":"timeline.json","player_publication_ms":trace["player_publication_ms"],
            "long_frame_count":trace["long_frames"].as_array().unwrap().len(),
            "physical_scanout":"unavailable","input_to_photon":"unavailable"}),
        )?;
        println!(
            "F1_RESULT {} measured",
            output().join("summary.json").display()
        );
        Ok(true)
    }
}
impl ClientApp {
    pub(super) fn service_f1_campaign(&mut self) {
        let Some(mut campaign) = self.f1_campaign.take() else {
            return;
        };
        match campaign.turn(self) {
            Ok(false) => self.f1_campaign = Some(campaign),
            Ok(true) => self.dx_exit_pending = true,
            Err(error) => {
                let _ = write_failure(&error);
                self.f1_failure = Some(error);
                self.dx_exit_pending = true;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fast_mouse_delta_uses_normal_ingestion_and_controller() {
        use super::super::Controller;
        let mut app = ClientApp::new(None, None);
        app.controller.captured = true;
        app.ingest_mouse((-FAST_MOUSE_COUNTS_PER_SECOND / 20., 0.), Instant::now());
        let intent = app.controller.next_intent();
        assert_eq!(intent.look_delta.x, -75.);
        assert_eq!(intent.movement.forward, 0.);
        assert_eq!(app.controller.next_intent().look_delta.x, 0.);
        assert!(Campaign::new(true).interactive);
        assert_eq!(&PHASES[..4], &["stationary", "pan", "walk", "walk_pan"]);
    }
    #[test]
    fn worker_overlap_is_separate_from_main_thread_work() {
        let mut t = Trace {
            enabled: true,
            ..Default::default()
        };
        let at = t.origin;
        t.frame(at, 1.);
        t.frame(at + Duration::from_millis(40), 2.);
        t.worker(
            "player",
            at + Duration::from_millis(5),
            at + Duration::from_millis(35),
            30.,
            true,
        );
        t.main_service(
            "service_player_autosave",
            at + Duration::from_millis(39),
            at + Duration::from_millis(40),
        );
        let v = t.summary(33.);
        assert_eq!(
            v["long_frames"][0]["worker_overlap"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            v["long_frames"][0]["main_persistence_overlap"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(v["player_publication_ms"]["p99"], 30.);
        assert_eq!(v["dropped_trace_events"], 0);
        let correlation = t.correlation(0., 40., 33.);
        assert_eq!(correlation["long_frames_overlapping_workers"], 1);
        assert_eq!(correlation["main_persistence_service_ms"]["max"], 1.);
        assert_eq!(
            correlation["main_persistence_time_inside_long_frames_ms"]["max"],
            1.
        );
    }
    #[test]
    fn percentiles_and_empty_observations_are_explicit() {
        assert_eq!(distribution(vec![])["count"], 0);
        let d = distribution(vec![3., 1., 2.]);
        assert_eq!(d["p50"], 2.);
        assert_eq!(d["max"], 3.);
    }
}
