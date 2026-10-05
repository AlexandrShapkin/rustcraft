//! Explicit graphical experiment: production fixed ticks, mouse ingestion, render and present.
use super::{ClientApp, devtools::ClientHost, presentation::Ledger};
use rustcraft_control::{Action, Context, Source};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
const PHASES: [&str; 6] = ["stationary", "pan", "walk", "walk_pan", "stop", "streaming"];
pub(super) struct Campaign {
    start: Instant,
    phase_start: Option<Instant>,
    last: Instant,
    phase: usize,
    origin: [f32; 3],
    samples: Vec<Value>,
    initialized: bool,
    checks: u8,
    check_tick: u64,
    check_started: Instant,
}
impl Campaign {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            phase_start: None,
            last: Instant::now(),
            phase: 0,
            origin: [0.; 3],
            samples: vec![],
            initialized: false,
            checks: 0,
            check_tick: 0,
            check_started: Instant::now(),
        }
    }
    fn turn(&mut self, app: &mut ClientApp) -> Result<bool, String> {
        if self.start.elapsed() > Duration::from_secs(420) {
            return Err("P1 experiment timed out".into());
        }
        if !app.player_control_enabled {
            return Ok(false);
        }
        let ctx = Context::developer(Source::Scenario);
        if !self.initialized {
            let p = app
                .simulation
                .as_ref()
                .ok_or("simulation unavailable")?
                .player
                .position;
            self.origin = [p.x.floor(), 96., p.z.floor()];
            // Disposable controlled walking pad. Semantic package block identity through Control.
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
            app.simulation.as_mut().unwrap().player.yaw = 0.;
            app.simulation.as_mut().unwrap().player.pitch = 0.;
            app.debug = false;
            app.controller.captured = true;
            self.initialized = true;
            return Ok(false);
        }
        if self.phase == PHASES.len() {
            return self.checks(app, &ctx);
        }
        if self.phase_start.is_none() {
            let l = app.lifetime_ledger();
            if l["core_ready"] != true
                || !app.mesh_scheduler.is_idle()
                || !app.snapshot_dirty_sections.is_empty()
                || !app.mesh_dirty_sections.is_empty()
            {
                return Ok(false);
            }
            println!(
                "P1_START settled_core={} frontier={}",
                l["core_ready"], l["lighting_frontier_columns"]
            );
            self.begin_phase(app);
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        app.controller.captured = true;
        app.controller.forward = if matches!(self.phase, 2 | 3 | 5) {
            1.
        } else {
            0.
        };
        app.controller.jump = self.phase == 5;
        if self.phase == 2 {
            let s = app.simulation.as_mut().unwrap();
            // Turn at the pad edge; ordinary authority still owns all movement/collision.
            if (s.player.position.z - self.origin[2]).abs() > 12. {
                s.player.yaw = if s.player.position.z > self.origin[2] {
                    std::f32::consts::PI
                } else {
                    0.
                };
            }
        }
        if matches!(self.phase, 1 | 3) {
            app.ingest_mouse((-dt * 0.65 / 0.002, 0.), now);
        }
        if self.phase_start.unwrap().elapsed() >= Duration::from_secs(20) {
            let mut sample = app.presentation_observation();
            sample["phase"] = json!(PHASES[self.phase]);
            sample["seconds"] = json!(self.phase_start.unwrap().elapsed().as_secs_f64());
            sample["authoritative_tick"] = json!(app.simulation.as_ref().unwrap().time);
            sample["configuration"] = app.control_state.config.snapshot();
            self.samples.push(sample);
            println!(
                "P1_PHASE {} {}",
                PHASES[self.phase],
                self.samples.last().unwrap()
            );
            self.phase += 1;
            if self.phase == PHASES.len() {
                app.controller.forward = 0.;
                app.controller.look = Default::default();
                self.check_started = Instant::now();
                return Ok(false);
            }
            self.begin_phase(app);
        }
        Ok(false)
    }
    fn checks(&mut self, app: &mut ClientApp, ctx: &Context) -> Result<bool, String> {
        if self.checks == 0 {
            rustcraft_control::execute(&mut ClientHost { app }, ctx, &Action::Pause)?;
            self.check_tick = app.simulation.as_ref().unwrap().time;
            self.check_started = Instant::now();
            self.checks = 1;
            return Ok(false);
        }
        if self.check_started.elapsed() < Duration::from_millis(500) {
            return Ok(false);
        }
        match self.checks {
            1 => {
                if app.simulation.as_ref().unwrap().time != self.check_tick {
                    return Err("pause advanced authority".into());
                }
                let pose =
                    super::presentation::Transform::from_sim(app.simulation.as_ref().unwrap());
                if app
                    .view_state
                    .sample(pose, 0.9, Default::default(), true)
                    .position
                    != pose.position
                {
                    return Err("paused visual drift".into());
                }
                rustcraft_control::execute(&mut ClientHost { app }, ctx, &Action::Step(1))?;
                self.checks = 2;
            }
            2 => {
                if app.simulation.as_ref().unwrap().time != self.check_tick + 1 {
                    return Err("single step was not exact".into());
                }
                rustcraft_control::execute(&mut ClientHost { app }, ctx, &Action::Resume)?;
                self.checks = 3;
            }
            3 => {
                if app.simulation.as_ref().unwrap().time <= self.check_tick + 1 {
                    return Err("resume did not advance".into());
                }
                rustcraft_control::execute(
                    &mut ClientHost { app },
                    ctx,
                    &Action::Teleport(self.origin),
                )?;
                let pose =
                    super::presentation::Transform::from_sim(app.simulation.as_ref().unwrap());
                if app
                    .view_state
                    .sample(pose, 0.01, Default::default(), false)
                    .position
                    != pose.position
                {
                    return Err("teleport interpolated old position".into());
                }
                app.control_state.page = "presentation".into();
                app.debug = true;
                self.checks = 4;
            }
            4 => {
                let mut capture_ctx = ctx.clone();
                capture_ctx.capabilities.insert("render.capture".into());
                rustcraft_control::execute(
                    &mut ClientHost { app },
                    &capture_ctx,
                    &Action::Capture("p1_final".into()),
                )?;
                self.checks = 5;
            }
            _ => {
                if let Some(job) = app.dx_capture_job {
                    match app.devtools.as_ref().unwrap().jobs.status(job)? {
                        rustcraft_control::JobStatus::Complete(_) => {
                            self.validate()?;
                            return Ok(true);
                        }
                        rustcraft_control::JobStatus::Failed(e) => return Err(e.clone()),
                        rustcraft_control::JobStatus::Cancelled => {
                            return Err("P1 capture cancelled".into());
                        }
                        _ => {}
                    }
                }
            }
        }
        self.check_started = Instant::now();
        Ok(false)
    }
    fn begin_phase(&mut self, app: &mut ClientApp) {
        let _ = app.presentation_observation();
        let old = std::mem::take(&mut app.presentation_timing);
        app.presentation_timing = Ledger::recording(old.monitor, old.target_ms);
        self.phase_start = Some(Instant::now());
        self.last = Instant::now();
    }
    fn validate(&self) -> Result<(), String> {
        for name in ["pan", "walk", "walk_pan"] {
            let v = self
                .samples
                .iter()
                .find(|v| v["phase"] == name)
                .ok_or("missing motion phase")?;
            let frames = v["frames"].as_u64().ok_or("missing frame count")?;
            let duplicates = v[if name == "walk" {
                "presentation_position_duplicates"
            } else {
                "camera_duplicates"
            }]
            .as_u64()
            .ok_or("missing duplicate count")?;
            // In this continuously driven fixture, every render turn has a distinct transform;
            // permit two boundary/rest frames. Authority may still repeat between ticks.
            if frames < 400 || duplicates > 2 {
                return Err(format!(
                    "{name}: insufficient >20FPS evidence or duplicate shown transforms: {frames}/{duplicates}"
                ));
            }
        }
        Ok(())
    }
    fn record(&self, status: &str, error: Option<String>) -> Result<(), String> {
        let dir = std::path::Path::new("target/p1");
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.json", std::process::id()));
        std::fs::write(&path,serde_json::to_vec_pretty(&json!({"status":status,"error":error,"input":"synthetic mouse events through native ingestion; ordinary LocalHumanController fixed-tick intent","surface":"640x360 graphical surface","pause_step_teleport_checks":self.checks,"phases":self.samples})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        println!("P1_RESULT {}", path.display());
        Ok(())
    }
}
impl ClientApp {
    pub(super) fn service_p1_campaign(&mut self) {
        let Some(mut c) = self.p1_campaign.take() else {
            return;
        };
        match c.turn(self) {
            Ok(false) => self.p1_campaign = Some(c),
            Ok(true) => {
                if let Err(e) = c.record("complete", None) {
                    self.p1_failure = Some(e);
                }
                self.dx_exit_pending = true;
            }
            Err(e) => {
                let _ = c.record("fail", Some(e.clone()));
                self.p1_failure = Some(e);
                self.dx_exit_pending = true;
            }
        }
    }
}
