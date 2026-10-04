//! Explicit, bounded diagnostic campaign. Checked Safe-to-Safe Control hops avoid terrain-route
//! accidents; production streaming/extraction/meshing/upload/removal remain unchanged.
use super::{ClientApp, devtools::ClientHost};
use rustcraft_control::{Action, Context, Host, Source, diagnostics::Domain};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
pub(super) struct Campaign {
    started: Instant,
    progress: Instant,
    initialized: bool,
    targets: Vec<[f32; 2]>,
    cursor: usize,
    samples: Vec<Value>,
    hops: usize,
    last: Value,
    last_turn: Instant,
    draining: bool,
    finishing: bool,
    capture: Value,
    providers: Value,
    events: Value,
}
impl Campaign {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            progress: Instant::now(),
            initialized: false,
            targets: vec![],
            cursor: 0,
            samples: vec![],
            hops: 0,
            last: Value::Null,
            last_turn: Instant::now() - Duration::from_secs(1),
            draining: false,
            finishing: false,
            capture: Value::Null,
            providers: Value::Null,
            events: Value::Null,
        }
    }
    fn turn(&mut self, app: &mut ClientApp) -> Result<bool, String> {
        if self.last_turn.elapsed() < Duration::from_millis(100) {
            return Ok(false);
        }
        self.last_turn = Instant::now();
        let Some(s) = app
            .simulation
            .as_ref()
            .filter(|_| app.player_control_enabled)
        else {
            return Ok(false);
        };
        let position = s.player.position;
        let mut ctx = Context::developer(Source::Scenario);
        ctx.capabilities.insert("render.capture".into());
        if !self.initialized {
            let span = (2 * app.residency.retain_radius() + 2) as f32 * 16.;
            self.targets.push([position.x, position.z]);
            for _ in 0..5 {
                self.targets.extend([
                    [position.x + span, position.z],
                    [position.x + span, position.z + span],
                    [position.x, position.z + span],
                    [position.x, position.z],
                ]);
            }
            for i in 2..10 {
                self.targets
                    .push([position.x + span * i as f32, position.z - span]);
            }
            self.targets.push([position.x, position.z]); // revisit origin after unique exploration
            rustcraft_control::execute(&mut ClientHost { app }, &ctx, &Action::Pause)?;
            app.control_state.page = "memory".into();
            app.debug = true;
            self.initialized = true;
            self.progress = Instant::now();
        }
        if self.started.elapsed() > Duration::from_secs(1800)
            || self.progress.elapsed() > Duration::from_secs(300)
        {
            self.last = app.lifetime_ledger();
            return Err(format!(
                "bounded campaign timeout at phase {} hop {}",
                self.cursor, self.hops
            ));
        }
        if self.finishing {
            if let Some(job) = app.dx_capture_job {
                match app
                    .devtools
                    .as_ref()
                    .ok_or("devtools unavailable")?
                    .jobs
                    .status(job)?
                {
                    rustcraft_control::JobStatus::Complete(value) => {
                        self.capture = value.clone();
                        return Ok(true);
                    }
                    rustcraft_control::JobStatus::Failed(error) => {
                        return Err(format!("capture failed: {error}"));
                    }
                    rustcraft_control::JobStatus::Cancelled => {
                        return Err("capture cancelled".into());
                    }
                    _ => {}
                }
            }
            return Ok(false);
        }
        if self.draining {
            app.control_state.diagnostics.invalidate(Domain::Residency);
        }
        let mut host = ClientHost { app };
        host.prepare_diagnostics(&[Domain::Residency]);
        let snap = host.snapshot();
        let ledger = &snap.residency;
        if self.draining {
            if ledger["idle"] != true
                || host
                    .app
                    .simulation
                    .as_ref()
                    .is_some_and(|s| s.lighting.has_integration_work())
            {
                return Ok(false);
            }
            self.samples.push(json!({"phase":self.cursor,"fully_drained":true,"hop":self.hops,"location":[position.x,position.z],"ledger":ledger}));
            self.providers = host.app.control_state.diagnostics.metrics();
            self.events =
                serde_json::to_value(host.app.devtools.as_ref().map(|t| t.events.entries()))
                    .map_err(|e| e.to_string())?;
            validate(&self.samples, host.app.residency.retain_radius())?;
            rustcraft_control::execute(&mut host, &ctx, &Action::Capture("rsm1_final".into()))?;
            self.finishing = true;
            self.progress = Instant::now();
            return Ok(false);
        }
        let target = self.targets[self.cursor];
        let dx = target[0] - position.x;
        let dz = target[1] - position.z;
        if dx.abs() < 0.05 && dz.abs() < 0.05 {
            if ledger["idle"] != true || ledger["player_chunk"] != snap.player["chunk"] {
                return Ok(false);
            }
            self.samples.push(json!({"phase":self.cursor,"hop":self.hops,"location":[position.x,position.z],"ledger":ledger}));
            println!(
                "RSM1_SAMPLE phase={} hop={} columns={} render={} metadata={} gpu_bytes={} rss={}",
                self.cursor,
                self.hops,
                ledger["world"]["resident_columns"],
                ledger["render"]["sections"],
                ledger["meshing"]["generations"],
                ledger["gpu"]["logical_bytes"],
                ledger["process"]["rss_bytes"]
            );
            self.cursor += 1;
            self.progress = Instant::now();
            if self.cursor == self.targets.len() {
                self.draining = true;
                return Ok(false);
            }
        } else {
            let next = [
                position.x + dx.clamp(-16., 16.),
                96.,
                position.z + dz.clamp(-16., 16.),
            ];
            let column = rustcraft_engine_core::ChunkPos {
                x: (next[0].floor() as i32).div_euclid(16),
                z: (next[2].floor() as i32).div_euclid(16),
            };
            if !host
                .app
                .simulation
                .as_ref()
                .is_some_and(|s| s.world.column_available(column))
                || !host.app.render_ready_columns.contains(&column)
            {
                return Ok(false);
            }
            rustcraft_control::execute(&mut host, &ctx, &Action::Teleport(next))?;
            host.app
                .control_state
                .diagnostics
                .invalidate(Domain::Residency);
            self.hops += 1;
            self.progress = Instant::now();
        }
        Ok(false)
    }
    fn record(&self, status: &str, error: Option<String>) -> Result<(), String> {
        let dir = std::path::Path::new("target/rsm1").join(format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("samples.json");
        std::fs::write(&path,serde_json::to_vec_pretty(&json!({"status":status,"error":error,"capture":self.capture,"providers":self.providers,"recent_events":self.events,"last_observation":self.last,"samples":self.samples,"hops":self.hops,"elapsed_ms":self.started.elapsed().as_millis()})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        println!("RSM1_RESULT {}", path.display());
        Ok(())
    }
}
/// Structural acceptance follows configured retention and actual current ownership, not RSS.
fn validate(samples: &[Value], retain: i32) -> Result<(), String> {
    if samples.len() != 31 {
        return Err("incomplete repeated/unique/revisit/drain campaign".into());
    }
    for sample in samples {
        let l = &sample["ledger"];
        let n = |path: &[&str]| {
            let mut v = l;
            for p in path {
                v = &v[*p];
            }
            v.as_u64().unwrap_or(u64::MAX)
        };
        let columns = n(&["world", "resident_columns"]);
        let bound = ((2 * i64::from(retain) + 1).pow(2) as u64)
            .saturating_add(n(&["world", "lighting_pinned"]));
        let logical = n(&["gpu", "logical_bytes"]);
        let capacity = n(&["gpu", "capacity_bytes"]);
        let sections = n(&["render", "sections"]);
        let pages = n(&["gpu", "mesh_pages"]);
        if logical
            > sections.saturating_mul(rustcraft_render::meshing::MAX_RESULT_LOGICAL_BYTES as u64)
            || capacity
                > logical
                    .saturating_mul(4)
                    .saturating_add(pages.saturating_mul(512))
            || columns > bound
            || n(&["world", "orphan_light_sections"]) != 0
            || n(&["render", "sections"]) != n(&["world", "resident_sections"])
            || n(&["gpu", "sections"]) != n(&["render", "sections"])
            || n(&["meshing", "generations"]) != n(&["render", "sections"])
            || [
                "pending",
                "inflight_submitted_unconsumed",
                "ready",
                "completed_unconsumed",
                "live_job_snapshots",
            ]
            .iter()
            .any(|p| n(&["meshing", p]) != 0)
        {
            return Err(format!(
                "ownership envelope failed at phase {}",
                sample["phase"]
            ));
        }
    }
    let last = &samples.last().unwrap()["ledger"];
    if last["lighting_lifetime"]["queued_cleanup"] != 0
        || last["lighting_lifetime"]["queued_integration"] != 0
        || last["lighting_lifetime"]["active_column"] != Value::Null
    {
        return Err("lighting retirement did not fully drain".into());
    }
    Ok(())
}
impl ClientApp {
    pub(super) fn service_rsm1_campaign(&mut self) {
        let Some(mut campaign) = self.rsm1_campaign.take() else {
            return;
        };
        match campaign.turn(self) {
            Ok(false) => self.rsm1_campaign = Some(campaign),
            Ok(true) => {
                if let Err(e) = campaign.record("complete", None) {
                    self.rsm1_failure = Some(e);
                }
                self.dx_exit_pending = true;
            }
            Err(error) => {
                campaign.last = self.lifetime_ledger();
                campaign.events =
                    serde_json::to_value(self.devtools.as_ref().map(|t| t.events.entries()))
                        .unwrap_or(Value::Null);
                eprintln!("RSM1_FAIL {error}");
                self.rsm1_failure = Some(error.clone());
                if let Err(e) = campaign.record("fail", Some(error)) {
                    eprintln!("RSM1 evidence failure: {e}");
                }
                self.dx_exit_pending = true;
            }
        }
    }
}
