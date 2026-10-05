//! Bounded application-side timing. Instants and transform scalars only, never frame resources.
use rustcraft_engine_core::Vec3;
use serde_json::{Value, json};
use std::{collections::VecDeque, time::Instant};
const CAP: usize = 512;
#[derive(Default)]
pub(super) struct Samples(VecDeque<f64>);
impl Samples {
    pub fn push(&mut self, v: f64) {
        if !v.is_finite() || v < 0. {
            return;
        }
        if self.0.len() == CAP {
            self.0.pop_front();
        }
        self.0.push_back(v);
    }
    pub fn summary(&self, target: Option<f64>) -> Value {
        if self.0.is_empty() {
            return json!([0]);
        }
        let mut values = self.0.iter().copied().collect::<Vec<_>>();
        values.sort_by(f64::total_cmp);
        let n = values.len();
        let mean = values.iter().sum::<f64>() / n as f64;
        let jitter = (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64).sqrt();
        let percentile = |p: f64| values[((n - 1) as f64 * p).ceil() as usize];
        let mut buckets = [0u64; 5];
        if let Some(t) = target {
            for v in &values {
                let r = v / t;
                let i = if r < 0.75 {
                    0
                } else if r < 1.25 {
                    1
                } else if r < 1.75 {
                    2
                } else if r < 2.5 {
                    3
                } else {
                    4
                };
                buckets[i] += 1;
            }
        }
        json!([
            n,
            mean,
            percentile(0.5),
            percentile(0.95),
            percentile(0.99),
            values[n - 1],
            jitter,
            target.map(|t| values.iter().filter(|v| **v > 1.5 * t).count()),
            target.map(|t| values.iter().filter(|v| **v > 2.5 * t).count()),
            target.map(|_| buckets)
        ])
    }
}
#[derive(Default, Clone, Copy)]
pub(super) struct Transform {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}
impl Transform {
    pub fn from_sim(s: &rustcraft_runtime::Simulation) -> Self {
        Self {
            position: s.player.position,
            yaw: s.player.yaw,
            pitch: s.player.pitch,
        }
    }
}
pub(super) fn yaw_delta(a: f32, b: f32) -> f32 {
    (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}
fn same_position(a: Transform, b: Transform) -> bool {
    a.position.x == b.position.x && a.position.y == b.position.y && a.position.z == b.position.z
}
#[derive(Default)]
pub(super) struct Ledger {
    pub enabled: bool,
    pub monitor: Value,
    pub target_ms: Option<f64>,
    pub render: Samples,
    pub request: Samples,
    pub redraw: Samples,
    pub acquire: Samples,
    pub prepare: Samples,
    pub submit: Samples,
    pub present: Samples,
    pub total: Samples,
    pub age: Samples,
    pub alpha: Samples,
    pub displacement: Samples,
    pub yaw_step: Samples,
    pub input_camera: Samples,
    pub input_authority: Samples,
    pub tick_gap: Samples,
    pub tick_work: Samples,
    last_render: Option<Instant>,
    last_request: Option<Instant>,
    last_redraw: Option<Instant>,
    last_submit: Option<Instant>,
    last_present: Option<Instant>,
    last_tick: Option<Instant>,
    tick_start: Option<Instant>,
    input_camera_at: Option<Instant>,
    input_tick_at: Option<Instant>,
    previous: Option<(Transform, Transform)>,
    pub authoritative_tick_used: Option<u64>,
    pub frames: u64,
    pub ticks: u64,
    pub auth_position_duplicates: u64,
    pub position_duplicates: u64,
    pub camera_duplicates: u64,
    pub resets: u64,
}
impl Ledger {
    pub fn recording(monitor: Value, target_ms: Option<f64>) -> Self {
        Self {
            enabled: true,
            monitor,
            target_ms,
            ..Default::default()
        }
    }
    fn interval(slot: &mut Option<Instant>, samples: &mut Samples, now: Instant) {
        if let Some(old) = slot.replace(now) {
            samples.push(now.duration_since(old).as_secs_f64() * 1000.);
        }
    }
    pub fn reset_cadence(&mut self) {
        self.last_render = None;
        self.last_request = None;
        self.last_redraw = None;
        self.last_submit = None;
        self.last_present = None;
        self.previous = None;
        self.last_tick = None;
        self.tick_start = None;
        self.input_camera_at = None;
        self.input_tick_at = None;
        self.resets += 1;
    }
    pub fn set_target(&mut self, target: Option<f64>) {
        if self.target_ms != target {
            self.reset_cadence();
            self.render = Default::default();
            self.request = Default::default();
            self.redraw = Default::default();
            self.submit = Default::default();
            self.present = Default::default();
            self.target_ms = target;
        }
    }
    pub fn request(&mut self, now: Instant) {
        if self.enabled {
            Self::interval(&mut self.last_request, &mut self.request, now);
        }
    }
    pub fn redraw(&mut self, now: Instant) {
        if self.enabled {
            Self::interval(&mut self.last_redraw, &mut self.redraw, now);
        }
    }
    pub fn input(&mut self, now: Instant) {
        if self.enabled {
            self.input_camera_at.get_or_insert(now);
            self.input_tick_at.get_or_insert(now);
        }
    }
    pub fn tick_start(&mut self, now: Instant) {
        if self.enabled {
            self.tick_start = Some(now);
        }
    }
    pub fn tick_end(&mut self, now: Instant, look: bool) {
        if self.enabled {
            Self::interval(&mut self.last_tick, &mut self.tick_gap, now);
            self.ticks += 1;
            if let Some(start) = self.tick_start.take() {
                self.tick_work
                    .push(now.duration_since(start).as_secs_f64() * 1000.);
            }
            if look && let Some(input) = self.input_tick_at.take() {
                self.input_authority
                    .push(now.duration_since(input).as_secs_f64() * 1000.);
            }
        }
    }
    pub fn frame(&mut self, now: Instant, auth: Transform, shown: Transform, alpha: f32) {
        if !self.enabled {
            return;
        }
        Self::interval(&mut self.last_render, &mut self.render, now);
        self.frames += 1;
        self.alpha.push(f64::from(alpha));
        if let Some(t) = self.last_tick {
            self.age.push(now.duration_since(t).as_secs_f64() * 1000.);
        }
        if let Some((old_auth, old)) = self.previous {
            if same_position(auth, old_auth) {
                self.auth_position_duplicates += 1;
            }
            if same_position(shown, old) {
                self.position_duplicates += 1;
            }
            let dy = yaw_delta(old.yaw, shown.yaw);
            let dp = shown.pitch - old.pitch;
            if same_position(shown, old) && dy == 0. && dp == 0. {
                self.camera_duplicates += 1;
            }
            self.displacement.push(f64::from({
                let d = shown.position - old.position;
                (d.x * d.x + d.y * d.y + d.z * d.z).sqrt()
            }));
            self.yaw_step.push(f64::from(dy.abs()));
            if (dy != 0. || dp != 0.)
                && let Some(input) = self.input_camera_at.take()
            {
                self.input_camera
                    .push(now.duration_since(input).as_secs_f64() * 1000.);
            }
        }
        self.previous = Some((auth, shown));
    }
    pub fn gpu_frame(&mut self, f: rustcraft_render::ApplicationFrameTiming) {
        if !self.enabled || f.present_at.is_none() {
            return;
        }
        self.acquire.push(f.acquire_ms);
        self.prepare.push(f.prepare_ms);
        self.total.push(f.total_ms);
        if let Some(t) = f.submit_at {
            Self::interval(&mut self.last_submit, &mut self.submit, t);
        }
        if let Some(t) = f.present_at {
            Self::interval(&mut self.last_present, &mut self.present, t);
        }
    }
    pub fn snapshot(&self) -> Value {
        if !self.enabled {
            return json!({"unavailable":"enable devtools or explicit P1 acceptance to record application timing"});
        }
        let t = self.target_ms;
        json!({"statistics_columns":["count","mean","p50","p95","p99","max","stddev","cadence_misses_gt_1.5_refresh","long_gt_2.5_refresh","refresh_multiple_buckets"],"clock":"std::time::Instant; monotonic application timestamps","physical_scanout":"unavailable","vrr":"unavailable","monitor":self.monitor,"target_ms":t,
            "render_ms":self.render.summary(t),"request_ms":self.request.summary(t),"redraw_ms":self.redraw.summary(t),"acquire_ms":self.acquire.summary(None),"prepare_ms":self.prepare.summary(None),"submit_ms":self.submit.summary(t),"present_call_ms":self.present.summary(t),"frame_cpu_ms":self.total.summary(t),
            "state_age_ms":self.age.summary(None),"alpha":self.alpha.summary(None),"position_delta":self.displacement.summary(None),"yaw_delta":self.yaw_step.summary(None),"input_event_to_camera_ms":self.input_camera.summary(None),"input_event_to_authority_ms":self.input_authority.summary(None),"tick_interval_ms":self.tick_gap.summary(None),"tick_work_ms":self.tick_work.summary(None),
            "authoritative_transform":self.previous.map(|(a,_)|json!({"position":[a.position.x,a.position.y,a.position.z],"yaw":a.yaw,"pitch":a.pitch})),"presentation_transform":self.previous.map(|(_,p)|json!({"position":[p.position.x,p.position.y,p.position.z],"yaw":p.yaw,"pitch":p.pitch})),"authoritative_tick_used":self.authoritative_tick_used,"frames":self.frames,"ticks":self.ticks,"authoritative_position_duplicates":self.auth_position_duplicates,"presentation_position_duplicates":self.position_duplicates,"camera_duplicates":self.camera_duplicates,"history_limit":CAP,"cadence_resets":self.resets})
    }
}
impl super::ClientApp {
    pub(super) fn ingest_mouse(&mut self, delta: (f64, f64), at: Instant) {
        if self.controller.captured && !self.dev_focus() {
            self.controller.look.x += delta.0 as f32;
            self.controller.look.y += delta.1 as f32;
            self.presentation_timing.input(at);
        }
    }
    pub(super) fn presentation_observation(&mut self) -> Value {
        let w = self.window.as_ref();
        let monitor = w.and_then(|w| w.current_monitor());
        let refresh = monitor
            .as_ref()
            .and_then(|m| m.refresh_rate_millihertz())
            .filter(|r| *r > 0);
        self.presentation_timing
            .set_target(refresh.map(|r| 1_000_000. / f64::from(r)));
        self.presentation_timing.monitor = json!({"name":monitor.and_then(|m|m.name()),"reported_refresh_millihertz":refresh,"refresh_provider":"winit configured monitor metadata, NOT scanout","scale_factor":w.map(|w|w.scale_factor()),"fullscreen":w.map(|w|w.fullscreen().is_some()),"platform":if cfg!(target_os="windows"){"Windows"}else if std::env::var_os("WAYLAND_DISPLAY").is_some(){"Wayland; compositor identity unavailable"}else{"platform compositor unavailable"}});
        let mut v = self.presentation_timing.snapshot();
        if let Some(r) = self.renderer.as_ref() {
            v["surface"] = json!(r.surface_description());
            v["present_mode"] = json!(format!("{:?}", r.config_present_mode()));
            v["supported_modes"] = json!(
                r.supported_present_modes
                    .iter()
                    .map(|m| format!("{m:?}"))
                    .collect::<Vec<_>>()
            );
            v["adapter"] = json!(r.adapter_info.name);
            v["backend"] = json!(format!("{:?}", r.adapter_info.backend));
        }
        v["fixed_dt_ms"] = json!(50);
        v["accumulator_alpha"] = json!(self.clock.alpha());
        v
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scalar_history_is_bounded_and_refresh_relative() {
        let mut s = Samples::default();
        for i in 0..2000 {
            s.push(f64::from(i));
        }
        assert_eq!(s.0.len(), CAP);
        let mut cadence = Samples::default();
        for ms in [6.94, 6.94, 13.88] {
            cadence.push(ms);
        }
        assert_eq!(cadence.summary(Some(6.94))[7], 1);
        assert_eq!(cadence.summary(Some(16.667))[7], 0);
        assert_eq!(cadence.summary(None)[7], Value::Null);
    }
    #[test]
    fn p1_recorder_overhead_and_display_rebase() {
        let mut recorder = Ledger::default();
        let pose = Transform::default();
        for enabled in [false, true] {
            recorder.enabled = enabled;
            let mut times = vec![];
            for _ in 0..10000 {
                let now = Instant::now();
                let start = Instant::now();
                recorder.frame(now, pose, pose, 0.5);
                recorder.request(now);
                times.push(start.elapsed().as_nanos());
            }
            times.sort_unstable();
            println!(
                "P1_RECORDER_COST enabled={enabled} mean_ns={} p50_ns={} p95_ns={} p99_ns={} max_ns={}",
                times.iter().sum::<u128>() / 10000,
                times[5000],
                times[9500],
                times[9900],
                times[9999]
            );
        }
        recorder.set_target(Some(16.667));
        assert!(recorder.render.0.is_empty());
        let now = Instant::now();
        recorder.request(now);
        recorder.reset_cadence();
        recorder.request(now + std::time::Duration::from_secs(10));
        assert!(recorder.request.0.is_empty());
        recorder.set_target(Some(6.944));
        assert_eq!(recorder.target_ms, Some(6.944));
        assert!(recorder.request.0.is_empty());
    }
    #[test]
    fn inactive_recorder_does_no_collection() {
        let mut l = Ledger::default();
        let t = Transform::default();
        for _ in 0..10000 {
            l.frame(Instant::now(), t, t, 0.5);
            l.request(Instant::now());
        }
        assert_eq!(l.frames, 0);
        assert!(l.render.0.is_empty());
    }
}

/// Client-only visual samples. No ownership of simulation/world data and no write-back path.
#[derive(Default)]
pub(super) struct ViewState {
    previous: Option<Transform>,
    current: Option<Transform>,
}
impl ViewState {
    pub fn reset(&mut self, pose: Transform) {
        self.previous = Some(pose);
        self.current = Some(pose);
    }
    pub fn tick(&mut self, before: Transform, after: Transform, paused: bool) {
        if paused {
            self.reset(after);
        } else {
            self.previous = Some(before);
            self.current = Some(after);
        }
    }
    pub fn sample(
        &mut self,
        authority: Transform,
        alpha: f32,
        pending: Vec3,
        paused: bool,
    ) -> Transform {
        // Detect out-of-tick reposition (load/Control/other composition owner).
        if self.current.is_none_or(|p| !same_position(p, authority)) || paused {
            self.reset(authority);
        }
        let previous = self.previous.unwrap_or(authority);
        let alpha = if alpha.is_finite() {
            alpha.clamp(0., 1.)
        } else {
            0.
        };
        Transform {
            position: previous.position + (authority.position - previous.position) * alpha,
            yaw: authority.yaw - pending.x * 0.002,
            pitch: (authority.pitch + pending.y * 0.002).clamp(-1.5, 1.5),
        }
    }
}
impl super::ClientApp {
    pub(super) fn rebase_presentation(&mut self) {
        if let Some(s) = self.simulation.as_ref() {
            self.view_state.reset(Transform::from_sim(s));
        }
        self.presentation_timing.reset_cadence();
    }
}
#[cfg(test)]
mod view_tests {
    use super::*;
    fn pose(x: f32, yaw: f32) -> Transform {
        Transform {
            position: Vec3::new(x, 0., 0.),
            yaw,
            pitch: 0.,
        }
    }
    #[test]
    fn translation_is_read_only_and_continuous_between_ticks() {
        let mut v = ViewState::default();
        v.tick(pose(0., 0.), pose(1., 0.), false);
        for i in 0..=10 {
            let p = v.sample(pose(1., 0.), i as f32 / 10., Vec3::ZERO, false);
            assert!((p.position.x - i as f32 / 10.).abs() < 1e-6);
        }
        assert_eq!(v.current.unwrap().position.x, 1.);
    }
    #[test]
    fn pending_mouse_is_applied_exactly_once_and_rebased_without_jump() {
        let mut v = ViewState::default();
        let old = pose(0., 0.);
        v.reset(old);
        let pending = Vec3::new(20. + 30. + 10., 25., 0.);
        let preview = v.sample(old, 0.7, pending, false);
        let mut controller = super::super::LocalHumanController {
            look: pending,
            ..Default::default()
        };
        use rustcraft_agent_api::Controller;
        let intent = controller.next_intent();
        assert_eq!(intent.look_delta.x, 60.);
        let mut sim = rustcraft_runtime::Simulation::new(
            rustcraft_engine_core::World::new(rustcraft_engine_core::BlockId(0)),
            rustcraft_mod_api::BlockRegistry::default(),
            Vec3::ZERO,
        );
        sim.step(intent, 0.05);
        let new = Transform::from_sim(&sim);
        v.tick(old, new, false);
        let rendered = v.sample(new, 0., controller.look, false);
        assert_eq!(preview.yaw, rendered.yaw);
        assert_eq!(preview.pitch, rendered.pitch);
        assert_eq!(controller.next_intent().look_delta.x, 0.);
    }
    #[test]
    fn wrap_pitch_discontinuity_and_paused_step_are_explicit() {
        assert!(
            (yaw_delta(359f32.to_radians(), 1f32.to_radians()) - 2f32.to_radians()).abs() < 1e-5
        );
        let mut v = ViewState::default();
        v.tick(pose(0., 0.), pose(1., 0.), false);
        assert_eq!(
            v.sample(pose(9000., 0.), 0.1, Vec3::ZERO, false).position.x,
            9000.
        );
        v.tick(pose(9000., 0.), pose(9001., 0.), true);
        for a in [0., 0.3, 0.9] {
            assert_eq!(
                v.sample(pose(9001., 0.), a, Vec3::ZERO, true).position.x,
                9001.
            );
        }
        assert_eq!(
            v.sample(pose(9001., 0.), 0., Vec3::new(0., 10000., 0.), false)
                .pitch,
            1.5
        );
    }
    #[test]
    fn catch_up_keeps_final_two_ticks_and_hitch_reset_snaps() {
        let mut v = ViewState::default();
        for i in 0..5 {
            v.tick(pose(i as f32, 0.), pose((i + 1) as f32, 0.), false);
        }
        assert_eq!(
            v.sample(pose(5., 0.), 0.5, Vec3::ZERO, false).position.x,
            4.5
        );
        v.reset(pose(5., 0.));
        assert_eq!(
            v.sample(pose(5., 0.), 0.01, Vec3::ZERO, false).position.x,
            5.
        );
    }
}
