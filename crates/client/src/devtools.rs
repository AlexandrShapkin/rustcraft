//! Generic developer presentation and platform control composition.
use super::*;
use rustcraft_engine_core::ChunkPos;

impl ClientApp {
    pub(super) fn legacy_debug_visible(&self) -> bool {
        self.debug
            && (self.devtools.is_none()
                || (!self.dev_focus()
                    && (self.control_state.page.is_empty()
                        || self.control_state.page == "overview")))
    }
    fn developer_text(&self, tools: &rustcraft_scripting_rhai::DevTools) -> String {
        let mut text = tools.text(&self.control_state);
        if self.debug
            && !tools.console_open
            && !self.control_state.selector.open
            && (self.control_state.page.is_empty() || self.control_state.page == "overview")
        {
            text.push('\n');
            text.push_str(&self.debug_overlay_text);
        }
        rustcraft_control::diagnostics::bounded_text(&text)
    }
    pub(super) fn dev_focus(&self) -> bool {
        self.devtools.as_ref().is_some_and(|d| d.console_open) || self.control_state.selector.open
    }
    fn dev_focus_transition(&mut self) {
        if self.dev_focus() {
            if let Some(window) = self.window.as_ref() {
                self.controller.release(window);
            }
            self.controller = LocalHumanController::default();
        } else if !self.inventory_open && !self.controller.captured {
            self.controller = LocalHumanController::default();
            if let Some(window) = self.window.as_ref() {
                self.controller.capture(window);
            }
        }
        self.dx_text.clear();
    }
    pub(super) fn dev_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        let PhysicalKey::Code(code) = event.physical_key else {
            return false;
        };
        if event.state != ElementState::Pressed {
            return self.dev_focus();
        }
        if code == KeyCode::F4 && !event.repeat {
            self.devtools
                .as_mut()
                .unwrap()
                .input(rustcraft_scripting_rhai::ConsoleInput::Close);
            use rustcraft_control::diagnostics::DebugInput;
            let input = if self.control_state.selector.open {
                DebugInput::Close
            } else {
                DebugInput::Open
            };
            let _ = self.control_state.selector_input(input);
            self.dev_focus_transition();
            return true;
        }
        if self.control_state.selector.open && code != KeyCode::Backquote && code != KeyCode::F10 {
            use rustcraft_control::diagnostics::{DebugInput, Domain};
            let input = match code {
                KeyCode::Escape => Some(DebugInput::Close),
                KeyCode::ArrowDown => Some(DebugInput::Next),
                KeyCode::ArrowUp => Some(DebugInput::Previous),
                KeyCode::Tab => Some(DebugInput::Tab),
                KeyCode::Enter => Some(DebugInput::Activate),
                KeyCode::KeyH => Some(DebugInput::Help),
                KeyCode::KeyC => Some(DebugInput::TargetChunk),
                KeyCode::KeyE => Some(DebugInput::NextEntity),
                _ => None,
            };
            if let Some(input) = input {
                if input == DebugInput::NextEntity {
                    use rustcraft_control::Host;
                    ClientHost { app: self }.prepare_diagnostics(&[Domain::Entities]);
                }
                if let Err(error) = self.control_state.selector_input(input) {
                    self.devtools.as_mut().unwrap().print(&error);
                }
                self.dev_focus_transition();
            }
            return true;
        }
        if code == KeyCode::Backquote && !event.repeat {
            self.control_state.selector.open = false;
            let tools = self.devtools.as_mut().unwrap();
            tools.input(rustcraft_scripting_rhai::ConsoleInput::Toggle);
            if tools.console_open
                && let Some(window) = self.window.as_ref()
            {
                self.controller.release(window);
                self.controller = LocalHumanController::default();
            }
            self.dev_focus_transition();
            return true;
        }
        if code == KeyCode::F3 && !event.repeat && !self.dev_focus() {
            if !self.control_state.page.is_empty() && self.control_state.page != "overview" {
                self.debug = false;
            }
            self.control_state.page = "overview".into();
            self.dx_text.clear();
        }
        if code == KeyCode::F10 && !event.repeat {
            if let (Some(mut tools), Some(simulation)) =
                (self.devtools.take(), self.simulation.as_mut())
            {
                tools.abort(&mut rustcraft_minecraft_b173::control::MinecraftHost {
                    simulation,
                    state: &mut self.control_state,
                });
                self.devtools = Some(tools);
            }
            return true;
        }
        if !self.devtools.as_ref().is_some_and(|d| d.console_open) {
            return false;
        }
        let mut tools = self.devtools.take().unwrap();
        match code {
            KeyCode::Escape => tools.input(rustcraft_scripting_rhai::ConsoleInput::Close),
            KeyCode::Enter => {
                if self.simulation.is_some() {
                    let _ = tools.submit(&mut ClientHost { app: self }, false);
                }
            }
            KeyCode::Backspace => tools.input(rustcraft_scripting_rhai::ConsoleInput::Backspace),
            KeyCode::Delete => tools.input(rustcraft_scripting_rhai::ConsoleInput::Delete),
            KeyCode::ArrowLeft => tools.input(rustcraft_scripting_rhai::ConsoleInput::Left),
            KeyCode::ArrowRight => tools.input(rustcraft_scripting_rhai::ConsoleInput::Right),
            KeyCode::ArrowUp => tools.input(rustcraft_scripting_rhai::ConsoleInput::HistoryUp),
            KeyCode::ArrowDown => tools.input(rustcraft_scripting_rhai::ConsoleInput::HistoryDown),
            KeyCode::Home => tools.input(rustcraft_scripting_rhai::ConsoleInput::Home),
            KeyCode::End => tools.input(rustcraft_scripting_rhai::ConsoleInput::End),
            KeyCode::PageUp => tools.input(rustcraft_scripting_rhai::ConsoleInput::ScrollUp),
            KeyCode::PageDown => tools.input(rustcraft_scripting_rhai::ConsoleInput::ScrollDown),
            KeyCode::Tab => tools.input(rustcraft_scripting_rhai::ConsoleInput::Complete),
            _ => {
                if let Some(text) = event.text.as_ref()
                    && text.chars().all(|c| !c.is_control())
                {
                    tools.input(rustcraft_scripting_rhai::ConsoleInput::Insert(
                        text.to_string(),
                    ));
                }
            }
        }
        self.dx_text = self.developer_text(&tools);
        self.devtools = Some(tools);
        self.dev_focus_transition();
        true
    }
    pub(super) fn service_devtools(&mut self) {
        if self.devtools.is_none() || !self.player_control_enabled {
            return;
        }
        let dx_started = Instant::now();
        let was_leased = self.control_state.leased;
        let was_focus = self.dev_focus();
        let mut tools = self.devtools.take().unwrap();
        if let Some(result) = self.renderer.as_mut().and_then(|r| r.poll_capture()) {
            match result {
                Ok(frame) => {
                    match tools.write_capture(frame.path, frame.width, frame.height, frame.rgba) {
                        Ok(id) => self.dx_capture_job = Some(id),
                        Err(error) => {
                            self.dx_capture_failure_recorded = true;
                            self.dx_capture_job =
                                tools.record_capture_failure(&error).ok().flatten();
                        }
                    }
                }
                Err(error) => {
                    self.dx_capture_failure_recorded = true;
                    self.dx_capture_job = tools.record_capture_failure(&error).ok().flatten();
                }
            }
        }
        if self.dux_fixture {
            self.dux_fixture = false;
            if let Some(s) = self.simulation.as_mut() {
                let p = s.player.position + Vec3::new(2., 1., 2.);
                s.spawn_item(rustcraft_minecraft_b173::blocks::DIRT.item.unwrap(), 1, p);
                self.control_state.selected_entity =
                    s.items.last().map(|e| format!("{:032x}", e.id.0));
            }
        }
        self.control_state.diagnostics.managed = true;
        let refresh_debug = self.dx_snapshot_at.elapsed() > Duration::from_millis(250);
        if refresh_debug {
            self.dx_snapshot_at = Instant::now();
        }
        if let Some(sim) = self.simulation.as_ref() {
            self.control_state.domains.tick = sim.time;
            self.control_state.domains.player = serde_json::json!({"position":[sim.player.position.x,sim.player.position.y,sim.player.position.z]});
        }
        if self.simulation.is_some() {
            let scenario_path = self.scenario_path.take();
            let abort = self
                .dx_abort_frame
                .is_some_and(|frame| tools.frame >= frame);
            let mut host = ClientHost { app: self };
            if let Some(path) = scenario_path
                && let Err(error) = tools.start(&path, &mut host)
            {
                tools.print(&error);
                eprintln!("DX scenario start failed: {error}");
                host.app.dx_exit_pending = true;
            }
            if abort && tools.scenario.as_ref().is_some_and(|s| s.active) {
                tools.abort(&mut host);
                host.app.dx_abort_frame = None;
            }
            match tools.advance(&mut host, true) {
                Ok(Some(bundle)) => {
                    println!("DX_RESULT {}", bundle.join("result.json").display());
                    host.app.control_state.fixed.pause();
                    host.app.dx_capture = Some(bundle.join("frame.png"));
                    host.app.dx_exit_pending = true;
                }
                Err(error) => {
                    tools.print(&error);
                    eprintln!("DX tooling error {error}; partial bundle may exist");
                    if let Some(s) = tools.scenario.as_mut() {
                        s.result.status = "error".into();
                        s.result.error = Some(error);
                    }
                    host.app.dx_exit_pending = true;
                }
                _ => {}
            }
        }
        let demand = self.control_state.diagnostic_demand();
        {
            tools.prepare(&demand, &mut ClientHost { app: self });
        }
        if self.control_state.fixed.paused {
            if let Some(simulation) = self.simulation.as_mut() {
                let dirty = simulation.take_persistence_dirty_chunks();
                let sections = simulation.take_dirty_sections();
                self.snapshot_dirty_sections.extend(sections);
                self.service_world_saves(dirty);
            }
            self.service_player_autosave();
            self.service_world_state_autosave();
        }
        self.control_state.leased = tools.scenario.as_ref().is_some_and(|s| s.active);
        if was_leased != self.control_state.leased {
            let captured = self.controller.captured;
            self.controller = LocalHumanController {
                captured,
                ..Default::default()
            };
        }
        if !self.control_state.leased {
            self.control_state.intent = Default::default();
        }
        if self.dx_capture.is_none()
            && !self.renderer.as_ref().is_some_and(|r| r.capture_pending())
            && let Some(name) = self.control_state.captures.pop_front()
        {
            let directory = PathBuf::from("target/captures").join(format!(
                "{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            if self.simulation.is_some() {
                use rustcraft_control::Host;
                tools.prepare(
                    &rustcraft_control::diagnostics::Domain::ALL,
                    &mut ClientHost { app: self },
                );
                let snapshot = ClientHost { app: self }.snapshot();
                if let Err(error) = tools.write_snapshot(directory.clone(), snapshot) {
                    tools.print(&error);
                }
                self.dx_capture = Some(directory.join("frame.png"));
            }
        }
        if self.control_state.overlays.is_empty() {
            self.dx_boxes.clear();
            self.dx_colors.clear();
            self.control_state.domains.overlay_geometry =
                serde_json::json!({"count":0,"boxes":[],"cap":64});
        }
        if refresh_debug || tools.console_open || self.dx_text.is_empty() {
            self.dx_text = self.developer_text(&tools);
        }
        if self.control_state.selector.open {
            tools.input(rustcraft_scripting_rhai::ConsoleInput::Close);
        }
        self.devtools = Some(tools);
        if was_focus != self.dev_focus() {
            self.dev_focus_transition();
        }
        self.dx_update_us = dx_started.elapsed().as_micros();
        self.dx_update_max_us = self.dx_update_max_us.max(self.dx_update_us);
    }
    fn collect_debug_geometry(&mut self) {
        self.dx_boxes.clear();
        self.dx_colors.clear();
        if let Some(sim) = self.simulation.as_ref() {
            use rustcraft_engine_core::Aabb;
            if self.control_state.overlays.contains("collision") {
                self.dx_boxes.push(sim.player.bounds());
                self.dx_colors.push([1., 0.8, 0.1]);
            }
            if self.control_state.overlays.contains("entities") {
                for item in self
                    .control_state
                    .domains
                    .entities
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(12)
                {
                    let Some(p) = diagnostic_position(item) else {
                        continue;
                    };
                    self.dx_boxes.push(Aabb::new(
                        Vec3::new(p.x - 0.15, p.y, p.z - 0.15),
                        Vec3::new(p.x + 0.15, p.y + 0.3, p.z + 0.15),
                    ));
                    self.dx_colors.push([0.9, 0.3, 1.]);
                    let x = p.x.floor().div_euclid(16.) * 16.;
                    let z = p.z.floor().div_euclid(16.) * 16.;
                    self.dx_boxes.push(Aabb::new(
                        Vec3::new(x, p.y, z),
                        Vec3::new(x + 16., p.y + 0.1, z + 16.),
                    ));
                    self.dx_colors.push([0.4, 0.5, 1.]);
                }
            }
            if self.control_state.overlays.contains("streaming") {
                let center = ChunkPos {
                    x: (sim.player.position.x.floor() as i32).div_euclid(16),
                    z: (sim.player.position.z.floor() as i32).div_euclid(16),
                };
                for dx in -2..=2 {
                    for dz in -2..=2 {
                        let chunk = ChunkPos {
                            x: center.x + dx,
                            z: center.z + dz,
                        };
                        let color = if self.render_ready_columns.contains(&chunk) {
                            [0.1, 1., 0.3]
                        } else if sim.world.column_available(chunk) {
                            [1., 0.9, 0.1]
                        } else if self.residency.is_desired(chunk) {
                            [1., 0.2, 0.1]
                        } else if self.residency.is_retained_by_radius(chunk) {
                            [0.2, 0.5, 1.]
                        } else {
                            [0.4, 0.4, 0.4]
                        };
                        let x = chunk.x as f32 * 16.;
                        let z = chunk.z as f32 * 16.;
                        self.dx_boxes.push(Aabb::new(
                            Vec3::new(x, sim.player.position.y, z),
                            Vec3::new(x + 16., sim.player.position.y + 0.1, z + 16.),
                        ));
                        self.dx_colors.push(color);
                    }
                }
            }
            if self.control_state.overlays.contains("chunks") {
                let value = &self.control_state.domains.chunk;
                if let (Some(x), Some(z), Some(y)) = (
                    value["chunk"][0].as_f64(),
                    value["chunk"][1].as_f64(),
                    value["section_y"].as_f64(),
                ) {
                    let p = Vec3::new(x as f32 * 16., y as f32 * 16., z as f32 * 16.);
                    self.dx_boxes
                        .push(Aabb::new(p, p + Vec3::new(16., 16., 16.)));
                    self.dx_colors.push([0., 1., 1.]);
                }
            }
            if self.control_state.overlays.contains("selected_entity")
                && let Some(p) = diagnostic_position(&self.control_state.domains.entity)
            {
                self.dx_boxes.push(Aabb::new(
                    Vec3::new(p.x - 0.15, p.y, p.z - 0.15),
                    Vec3::new(p.x + 0.15, p.y + 0.3, p.z + 0.15),
                ));
                self.dx_colors.push([1., 0.5, 0.]);
                let x = p.x.floor().div_euclid(16.) * 16.;
                let z = p.z.floor().div_euclid(16.) * 16.;
                self.dx_boxes.push(Aabb::new(
                    Vec3::new(x, p.y, z),
                    Vec3::new(x + 16., p.y + 0.1, z + 16.),
                ));
                self.dx_colors.push([0.3, 0.5, 1.]);
            }
            if self.control_state.overlays.contains("target")
                && let Some(hit) = sim.target()
            {
                self.dx_colors.push([1., 1., 1.]);
                let p = hit.block;
                self.dx_boxes.push(Aabb::new(
                    Vec3::new(p.x as f32, p.y as f32, p.z as f32),
                    Vec3::new(p.x as f32 + 1., p.y as f32 + 1., p.z as f32 + 1.),
                ));
            }
        }
    }
}

/// Repeatable diagnostic through the real service and surface loop, using equal event-turn counts.
#[derive(Default)]
pub(super) struct Probe {
    phase: usize,
    turns: usize,
    samples: Vec<u128>,
    reports: Vec<serde_json::Value>,
    parked: Option<rustcraft_scripting_rhai::DevTools>,
    before: std::collections::BTreeMap<rustcraft_control::diagnostics::Domain, u64>,
}
impl ClientApp {
    pub(super) fn service_devtools_measured(&mut self, event_loop: &ActiveEventLoop) {
        let Some(mut probe) = self.dx_probe.take() else {
            self.service_devtools();
            return;
        };
        if !self.player_control_enabled {
            self.dx_probe = Some(probe);
            return;
        }
        if probe.turns == 0 {
            self.control_state.fixed.pause();
            probe.before = self
                .control_state
                .diagnostics
                .samples
                .iter()
                .map(|(d, s)| (*d, s.collections))
                .collect();
            match probe.phase {
                0 => probe.parked = self.devtools.take(),
                1 => {
                    self.devtools = probe.parked.take();
                    self.control_state.page.clear();
                    self.control_state.overlays.clear();
                }
                2 => self.control_state.page = "lighting".into(),
                3 => self.control_state.page = "renderer".into(),
                4 => {
                    self.control_state.page.clear();
                    self.control_state
                        .overlays
                        .extend(["streaming".into(), "collision".into()]);
                }
                5 => {
                    self.control_state.overlays.clear();
                    self.devtools.as_mut().unwrap().scenario = Some(
                        rustcraft_control::Scenario::new(
                            "overhead".into(),
                            vec![rustcraft_control::Step::WaitFrames(100000)],
                            rustcraft_control::Context::developer(
                                rustcraft_control::Source::Scenario,
                            ),
                            1,
                            "diagnostic".into(),
                        )
                        .unwrap(),
                    );
                }
                _ => unreachable!(),
            }
        }
        let started = Instant::now();
        self.service_devtools();
        let us = started.elapsed().as_nanos() as f64 / 1000.;
        if probe.turns >= 20 {
            probe.samples.push((us * 1000.) as u128);
        }
        probe.turns += 1;
        if probe.turns == 240 {
            probe.samples.sort_unstable();
            let quantile =
                |p: usize| probe.samples[(probe.samples.len() - 1) * p / 100] as f64 / 1000.;
            let mean =
                probe.samples.iter().sum::<u128>() as f64 / probe.samples.len() as f64 / 1000.;
            let report = serde_json::json!({"mode":(["disabled",
                "overview",
                "low_page",
                "high_page",
                "overlay",
                "scenario"][probe.phase]),
                "event_turns":probe.turns,
                "samples":probe.samples.len(),
                "mean_us":mean,
                "p50_us":quantile(50),
                "p95_us":quantile(95),
                "p99_us":quantile(99),
                "max_us":quantile(100),
                "render_frames":self.devtools.as_ref().map(|d|d.frame),
                "collections":self.control_state.diagnostics.samples.iter().map(|(d,s)|(d.key(),s.collections-probe.before.get(d).copied().unwrap_or(0))).collect::<std::collections::BTreeMap<_,_>>(),
                "geometry_count":self.dx_boxes.len(),
                "cache_domains":self.control_state.diagnostics.samples.len()});
            eprintln!("DX_OVERHEAD {report}");
            probe.reports.push(report);
            probe.samples.clear();
            probe.turns = 0;
            probe.phase += 1;
            if probe.phase == 6 {
                if let Some(tools) = self.devtools.as_mut() {
                    tools.scenario = None;
                }
                self.control_state.intent = Default::default();
                self.control_state.leased = false;
                std::fs::create_dir_all("target").unwrap();
                rustcraft_scripting_rhai::write_json(
                    std::path::Path::new("target/dx-overhead.json"),
                    &probe.reports,
                )
                .unwrap();
                event_loop.exit();
                return;
            }
        }
        self.dx_probe = Some(probe);
    }
}

/// Graphical composition requests existing subsystem observations into the shared Control cache.
/// Neither presentation nor scripting owns a second world/entity scanner.
struct ClientHost<'a> {
    app: &'a mut ClientApp,
}
impl rustcraft_control::Host for ClientHost<'_> {
    fn snapshot(&self) -> rustcraft_control::Snapshot {
        let mut s = self.app.simulation.as_ref().map_or_else(
            || self.app.control_state.domains.clone(),
            |sim| {
                rustcraft_minecraft_b173::control::MinecraftHost::live_snapshot(
                    sim,
                    &self.app.control_state,
                )
            },
        );
        s.debug = self.app.control_state.debug_metadata();
        s.debug["entity_coverage"] =
            self.app.control_state.domains.debug["entity_coverage"].clone();
        s.debug["overlay_enabled"] = serde_json::json!(
            self.app
                .control_state
                .overlays
                .iter()
                .map(|n| (n.clone(), true))
                .collect::<std::collections::BTreeMap<_, _>>()
        );
        s.debug["geometry_count"] = serde_json::json!(self.app.dx_boxes.len());
        s.debug["registered_views"] =
            serde_json::json!(self.app.control_state.diagnostics.registry.len());
        s.debug["selected_entity_present"] = serde_json::json!(s.entity["position"].is_array());
        s.debug["legacy_overview_text"] = serde_json::json!(self.app.debug_overlay_text);
        s.debug["selector_input_focus"] = serde_json::json!(self.app.control_state.selector.open);
        s
    }
    fn diagnostic_due(&self, domain: rustcraft_control::diagnostics::Domain) -> bool {
        self.app
            .control_state
            .diagnostics
            .samples
            .get(&domain)
            .and_then(|s| s.at)
            .is_none_or(|at| at.elapsed() >= rustcraft_control::diagnostics::Diagnostics::CADENCE)
    }
    fn prepare_diagnostics(&mut self, domains: &[rustcraft_control::diagnostics::Domain]) {
        use rustcraft_control::diagnostics::Domain;
        for &d in domains {
            if d == Domain::Scripts || !self.app.control_state.diagnostics.due(d, Instant::now()) {
                continue;
            }
            let started = Instant::now();
            let a = &mut self.app;
            let v = match d {
                Domain::World | Domain::Entities | Domain::Entity => {
                    if let Some(simulation) = a.simulation.as_mut() {
                        let mut value = rustcraft_minecraft_b173::control::MinecraftHost {
                            simulation,
                            state: &mut a.control_state,
                        }
                        .collect_diagnostic(d);
                        if d == Domain::World {
                            value["generator"] = serde_json::json!(
                                a.stream_generator
                                    .as_ref()
                                    .map(|g| serde_json::json!({"id":g.id(),
                            "version":g.version()}))
                            );
                        }
                        value
                    } else {
                        serde_json::json!({"unavailable":"simulation not open"})
                    }
                }
                Domain::Streaming => {
                    let load = a.load_scheduler.metrics();
                    let generation = a.generation_scheduler.metrics();
                    serde_json::json!({"player_chunk":a.simulation.as_ref().map(|s|[(s.player.position.x.floor() as i32).div_euclid(16),(s.player.position.z.floor() as i32).div_euclid(16)]),
                        "desired":a.residency.desired_column_count(),
                        "desired_radius":a.residency.load_radius(),
                        "retained_radius":a.residency.retain_radius(),
                        "resident":a.simulation.as_ref().map(|s|s.world.column_positions().count()),
                        "pending":a.residency.pending_column_count(),
                        "safe":a.simulation.as_ref().map(|s|s.world.safe_column_positions().count()),
                        "visible":a.render_ready_columns.len(),
                        "idle":a.residency.pending_column_count()==0,
                        "load":{"queued":load.queued,
                        "inflight":load.in_flight,
                        "failures":load.failed},
                        "generation":{"queued":generation.pending,
                        "inflight":generation.in_flight},
                        "frontier":a.frontier_summary()})
                }
                Domain::Lighting => a.simulation.as_ref().map_or(
                    serde_json::json!({"unavailable":"simulation not open"}),
                    |s| {
                        let l = s.lighting.work_counters();
                        serde_json::json!({"backlog":s.lighting.has_integration_work(),
                    "initial_pending":a.initial_lighting_scheduler.pending(),
                    "columns_started":l.columns_started,
                    "columns_completed":l.columns_completed,
                    "queue_pushes":l.propagation_queue_pushes,
                    "queue_pops":l.propagation_queue_pops})
                    },
                ),
                Domain::Meshing => {
                    let m = a.mesh_scheduler.stats();
                    serde_json::json!({"idle":a.mesh_scheduler.is_idle(),
                    "pending":m.pending,
                    "inflight":m.in_flight,
                    "ready":m.ready,
                    "failed":m.mesh_jobs_failed,
                    "stale":m.mesh_jobs_discarded_stale,
                    "workers":m.worker_count,
                    "ready_bytes":m.ready_cpu_bytes,
                    "pending_snapshot_bytes":m.pending_snapshot_bytes,
                    "inflight_snapshot_bytes":m.in_flight_snapshot_bytes,
                    "generation_entries":a.mesh_scheduler.generation_entry_count()})
                }
                Domain::Renderer => a.renderer.as_ref().map_or(
                    serde_json::json!({"unavailable":"graphical surface not open"}),
                    |r| {
                        serde_json::json!({"adapter":r.adapter_info.name,
                    "backend":format!("{:?}",r.adapter_info.backend),
                    "surface":r.surface_description(),
                    "present_mode":format!("{:?}",r.config_present_mode()),
                    "draw_calls":r.draw_calls(),
                    "meshes":r.mesh_count(),
                    "logical_gpu_bytes":r.gpu_mesh_logical_bytes(),
                    "capacity_gpu_bytes":r.gpu_mesh_allocated_bytes(),
                    "capture_pending":r.capture_pending(),
                    "timing":"application diagnostics; physical scanout unavailable"})
                    },
                ),
                Domain::Persistence => {
                    let saves = a.save_scheduler.metrics();
                    let player = a.player_save_scheduler.metrics();
                    let world = a.world_state_save_scheduler.metrics();
                    serde_json::json!({"dirty":a.persistence_dirty.dirty_count(),
                    "dirty_subset_cap":16,"dirty_subset_truncated":a.persistence_dirty.dirty_count()>16,"dirty_subset":a.persistence_dirty.diagnostic_entries(16).map(|(p,g,s)|serde_json::json!({"chunk":[p.x,p.z],
                    "generation":g,
                    "saving":s})).collect::<Vec<_>>(),
                    "queued":saves.queued,
                    "inflight":saves.in_flight,
                    "failures":saves.failed,
                    "player_revision":a.player_revision,
                    "player_persisted":a.player_persisted_revision,
                    "player_dirty":a.player_dirty,
                    "player_inflight":player.in_flight,
                    "player_failures":player.failures,
                    "world_revision":a.world_state_revision,
                    "world_persisted":a.world_state_persisted_revision,
                    "world_dirty":a.world_state_dirty,
                    "world_inflight":world.in_flight,
                    "world_failures":world.failures})
                }
                Domain::Chunk => a.chunk_diagnostic(),
                Domain::Overlays => a.overlay_diagnostic(),
                Domain::Scripts => unreachable!(),
            };
            if d == Domain::Entities {
                a.control_state.domains.debug["entity_coverage"] = serde_json::json!({"cap":64,
                    "active":a.simulation.as_ref().map(|s|s.items.len()),
                    "truncated":a.simulation.as_ref().is_some_and(|s|s.items.len()>64)});
            }
            d.set(&mut a.control_state.domains, v);
            a.control_state.diagnostics.collected(d, started);
        }
    }
    fn publish_diagnostic(
        &mut self,
        domain: rustcraft_control::diagnostics::Domain,
        value: serde_json::Value,
        started: Instant,
    ) {
        if self
            .app
            .control_state
            .diagnostics
            .due(domain, Instant::now())
        {
            domain.set(&mut self.app.control_state.domains, value);
            self.app
                .control_state
                .diagnostics
                .collected(domain, started);
        }
    }
    fn block(&self, p: [i32; 3]) -> rustcraft_control::ControlResult<String> {
        let s = self
            .app
            .simulation
            .as_ref()
            .ok_or("simulation unavailable")?;
        let p = rustcraft_engine_core::BlockPos {
            x: p[0],
            y: p[1],
            z: p[2],
        };
        if !s
            .world
            .column_available(rustcraft_engine_core::split_block(p).0)
        {
            return Err("column unavailable".into());
        }
        s.registry
            .get(s.world.get(p))
            .map(|d| d.name.to_owned())
            .ok_or("unregistered block".into())
    }
    fn apply(
        &mut self,
        action: &rustcraft_control::Action,
    ) -> rustcraft_control::ControlResult<serde_json::Value> {
        let simulation = self
            .app
            .simulation
            .as_mut()
            .ok_or("simulation unavailable")?;
        let result = rustcraft_minecraft_b173::control::MinecraftHost {
            simulation,
            state: &mut self.app.control_state,
        }
        .apply(action);
        if result.is_ok()
            && matches!(
                action,
                rustcraft_control::Action::DebugUi(_)
                    | rustcraft_control::Action::DebugPage(_)
                    | rustcraft_control::Action::EntityPage(_)
                    | rustcraft_control::Action::Overlay { .. }
                    | rustcraft_control::Action::InspectChunk { .. }
                    | rustcraft_control::Action::InspectEntity(_)
            )
        {
            self.app.dx_text.clear();
        }
        result
    }
    fn intent(
        &mut self,
        intent: rustcraft_control::AgentIntent,
    ) -> rustcraft_control::ControlResult<()> {
        self.app.control_state.intent = intent;
        Ok(())
    }
}
impl ClientApp {
    fn chunk_diagnostic(&self) -> serde_json::Value {
        let Some(s) = self.simulation.as_ref() else {
            return serde_json::json!({"unavailable":"simulation not open"});
        };
        let [x, z, y] = self.control_state.selected_chunk.unwrap_or_else(|| {
            let p = s
                .target()
                .map(|h| h.block)
                .unwrap_or(rustcraft_engine_core::BlockPos {
                    x: s.player.position.x.floor() as i32,
                    y: s.player.position.y.floor() as i32,
                    z: s.player.position.z.floor() as i32,
                });
            [p.x.div_euclid(16), p.z.div_euclid(16), p.y.div_euclid(16)]
        });
        let chunk = ChunkPos { x, z };
        let resident = s.world.column_positions().any(|p| p == chunk);
        let safe = s.world.column_available(chunk);
        let visible = self.render_ready_columns.contains(&chunk);
        serde_json::json!({"chunk":[x,z],
            "section_y":y,
            "resident":resident,
            "desired":self.residency.is_desired(chunk),
            "retained_radius":self.residency.is_retained_by_radius(chunk),
            "safe":safe,
            "visible":visible,
            "residency_phase":format!("{:?}",self.residency.phase(chunk)),
            "lighting":"column-specific convergence unavailable; Safe gates interaction",
            "lighting_pending_global":s.lighting.has_integration_work(),
            "column_mesh_stage":format!("{:?}",self.mesh_scheduler.column_stage(chunk)),
            "section_mesh_stage":format!("{:?}",self.mesh_scheduler.section_stage((chunk,y))),
            "section_gpu_pages":self.renderer.as_ref().and_then(|r|r.section_mesh_pages(chunk,y)),
            "renderer_available":self.renderer.is_some(),
            "mesh_generation":self.mesh_scheduler.current_generation((chunk,y)),
            "dirty":self.persistence_dirty.is_dirty(chunk),
            "save_pin":self.persistence_dirty.is_saving(chunk),
            "active_entities_subset":s.items.iter().take(4096).filter(|e|(e.position.x.floor() as i32).div_euclid(16)==x&&(e.position.z.floor() as i32).div_euclid(16)==z).count(),
            "entity_coverage_truncated":s.items.len()>4096,
            "why":if !resident{"not resident; inspect desired/load state"}else if !safe{"resident but unsafe; neighborhood/lighting/render frontier not released"}else if !visible{"safe column without current ready presentation"}else{"current presentation ready"}})
    }
    fn overlay_diagnostic(&mut self) -> serde_json::Value {
        // Existing bounded debug geometry collection is invoked only for active overlay demand.
        self.collect_debug_geometry();
        serde_json::json!({"boxes":self.dx_boxes.iter().zip(&self.dx_colors).map(|(b,c)|serde_json::json!({"min":[b.min.x,b.min.y,b.min.z],
            "max":[b.max.x,b.max.y,b.max.z],
            "color":c})).collect::<Vec<_>>(),
            "count":self.dx_boxes.len(),
            "cap":64})
    }
}

fn diagnostic_position(v: &serde_json::Value) -> Option<Vec3> {
    let p = v["position"].as_array()?;
    Some(Vec3::new(
        p.first()?.as_f64()? as f32,
        p.get(1)?.as_f64()? as f32,
        p.get(2)?.as_f64()? as f32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_control::{
        Action, Host,
        diagnostics::{DebugInput, Domain},
    };
    fn app() -> ClientApp {
        use rustcraft_mod_api::GameplayModule;
        let mut registry = rustcraft_mod_api::BlockRegistry::default();
        rustcraft_minecraft_b173::blocks::BlocksModule
            .register(&mut registry)
            .unwrap();
        let mut a = ClientApp::new(None, None);
        a.simulation = Some(Simulation::new(
            rustcraft_engine_core::World::new(rustcraft_minecraft_b173::blocks::AIR.id),
            registry,
            Vec3::new(0.5, 3., 0.5),
        ));
        a.control_state.diagnostics.managed = true;
        a
    }
    #[test]
    fn shared_observation_is_reused_and_inactive_demand_stops_collection() {
        let mut a = app();
        let mut host = ClientHost { app: &mut a };
        host.prepare_diagnostics(&[Domain::World]);
        let first = host.snapshot().world;
        host.prepare_diagnostics(&[Domain::World]);
        assert_eq!(host.snapshot().world, first);
        assert_eq!(
            host.app.control_state.diagnostics.samples[&Domain::World].collections,
            1
        );
        assert!(host.app.control_state.diagnostic_demand().is_empty());
        host.prepare_diagnostics(&[]);
        assert_eq!(
            host.app.control_state.diagnostics.samples[&Domain::World].collections,
            1
        );
        host.app.control_state.page = "world".into();
        assert!(
            host.app
                .control_state
                .diagnostic_text()
                .contains("block_query_radius: 1")
        );
        let mut tools = rustcraft_scripting_rhai::DevTools::new(
            std::path::Path::new("../../scripts"),
            rustcraft_control::engine_registry(),
        );
        // Tests execute from the crate root; source-root fallback supports cargo invocations.
        if tools.is_err() {
            tools = rustcraft_scripting_rhai::DevTools::new(
                std::path::Path::new("scripts"),
                rustcraft_control::engine_registry(),
            );
        }
        tools
            .unwrap()
            .evaluate("assert_eq(world().block_query_radius, 1)", &mut host, false)
            .unwrap();
        assert_eq!(
            host.app.control_state.diagnostics.samples[&Domain::World].collections,
            1
        );
        let sim = host.app.simulation.as_mut().unwrap();
        sim.spawn_item(
            rustcraft_minecraft_b173::blocks::DIRT.item.unwrap(),
            1,
            Vec3::new(2., 3., 2.),
        );
        let id = format!("{:032x}", sim.items[0].id.0);
        let mut tools = rustcraft_scripting_rhai::DevTools::new(
            std::path::Path::new("../../scripts"),
            rustcraft_control::engine_registry(),
        )
        .unwrap();
        tools
            .evaluate(
                &format!("assert_true(entity(\"{id}\").age == 0.0)"),
                &mut host,
                false,
            )
            .unwrap();
        assert_eq!(
            host.app.control_state.diagnostics.samples[&Domain::Entities].collections,
            1
        );
    }
    #[test]
    fn selected_identity_and_transient_geometry_are_bounded() {
        let mut a = app();
        let s = a.simulation.as_mut().unwrap();
        s.spawn_item(
            rustcraft_minecraft_b173::blocks::DIRT.item.unwrap(),
            1,
            Vec3::new(2., 3., 2.),
        );
        let id = format!("{:032x}", s.items[0].id.0);
        a.control_state
            .view_action(&Action::InspectEntity(id.clone()))
            .unwrap();
        for _ in 0..24 {
            a.control_state
                .view_action(&Action::DebugUi(DebugInput::Open))
                .unwrap();
            a.control_state
                .view_action(&Action::Overlay {
                    name: "selected_entity".into(),
                    enabled: true,
                })
                .unwrap();
            let demand = a.control_state.diagnostic_demand();
            ClientHost { app: &mut a }.prepare_diagnostics(&demand);
            assert_eq!(a.control_state.domains.entity["id"], id);
            assert_eq!(a.dx_boxes.len(), 2);
            a.control_state
                .view_action(&Action::Overlay {
                    name: "selected_entity".into(),
                    enabled: false,
                })
                .unwrap();
            a.collect_debug_geometry();
            assert!(a.dx_boxes.is_empty());
            a.control_state
                .view_action(&Action::DebugUi(DebugInput::Close))
                .unwrap();
        }
        a.simulation.as_mut().unwrap().items.clear();
        a.control_state.diagnostics.invalidate(Domain::Entity);
        ClientHost { app: &mut a }.prepare_diagnostics(&[Domain::Entity]);
        assert_eq!(a.control_state.domains.entity["id"], id);
        assert!(a.control_state.domains.entity["position"].is_null());
        assert!(
            a.control_state.domains.entity["status"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
        assert_eq!(a.control_state.diagnostics.registry.len(), 17);
        assert!(a.control_state.diagnostics.samples.len() <= 11);
    }
    #[test]
    fn focus_transition_clears_held_input_and_allows_fresh_input() {
        let mut a = app();
        for _ in 0..24 {
            a.controller.key(KeyCode::KeyW, ElementState::Pressed);
            a.controller.look = Vec3::new(10., 2., 0.);
            a.controller.break_held = true;
            a.control_state.selector_input(DebugInput::Open).unwrap();
            a.dev_focus_transition();
            assert!(a.dev_focus());
            assert_eq!(a.controller.forward, 0.);
            assert_eq!(a.controller.look, Vec3::ZERO);
            assert!(!a.controller.break_held);
            a.control_state.selector_input(DebugInput::Close).unwrap();
            a.dev_focus_transition();
            assert!(!a.dev_focus());
            assert_eq!(a.controller.forward, 0.);
            a.controller.key(KeyCode::KeyW, ElementState::Pressed);
            assert_eq!(a.controller.forward, 1.);
            a.controller.key(KeyCode::KeyW, ElementState::Released);
            assert_eq!(a.controller.forward, 0.);
        }
    }
    #[test]
    fn f3_preserves_cached_metrics_and_hidden_overview_stops_legacy_collection() {
        let mut a = app();
        a.debug = true;
        a.debug_overlay_text = "FPS 60 TICK 2MS".into();
        a.devtools = Some(
            rustcraft_scripting_rhai::DevTools::new(
                std::path::Path::new("../../scripts"),
                rustcraft_control::engine_registry(),
            )
            .unwrap(),
        );
        assert!(a.legacy_debug_visible());
        assert!(
            a.developer_text(a.devtools.as_ref().unwrap())
                .contains("FPS 60")
        );
        assert_eq!(
            ClientHost { app: &mut a }.snapshot().debug["legacy_overview_text"],
            "FPS 60 TICK 2MS"
        );
        a.control_state.page = "world".into();
        assert!(!a.legacy_debug_visible());
        a.control_state.page = "overview".into();
        a.control_state.selector.open = true;
        assert!(!a.legacy_debug_visible());
        a.control_state.selector.open = false;
        a.devtools.as_mut().unwrap().console_open = true;
        assert!(!a.legacy_debug_visible());
    }
    #[test]
    fn absent_render_provider_and_disabled_startup_are_explicit() {
        let mut a = app();
        assert!(a.devtools.is_none());
        assert!(!a.dev_focus());
        assert!(a.control_state.diagnostics.samples.is_empty());
        ClientHost { app: &mut a }.prepare_diagnostics(&[Domain::Renderer]);
        assert!(
            a.control_state.domains.renderer["unavailable"]
                .as_str()
                .unwrap()
                .contains("surface")
        );
    }
}
