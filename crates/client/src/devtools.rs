//! Generic developer presentation and platform control composition.
use super::*;
use rustcraft_engine_core::ChunkPos;

impl ClientApp {
    pub(super) fn dev_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        let PhysicalKey::Code(code) = event.physical_key else {
            return false;
        };
        if event.state != ElementState::Pressed {
            return self.devtools.as_ref().is_some_and(|d| d.console_open);
        }
        if code == KeyCode::Backquote && !event.repeat {
            let tools = self.devtools.as_mut().unwrap();
            tools.input(rustcraft_scripting_rhai::ConsoleInput::Toggle);
            if tools.console_open
                && let Some(window) = self.window.as_ref()
            {
                self.controller.release(window);
                self.controller = LocalHumanController::default();
            }
            return true;
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
                if let Some(simulation) = self.simulation.as_mut() {
                    let _ = tools.submit(
                        &mut rustcraft_minecraft_b173::control::MinecraftHost {
                            simulation,
                            state: &mut self.control_state,
                        },
                        false,
                    );
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
        self.dx_text = tools.text(&self.control_state);
        self.devtools = Some(tools);
        true
    }
    pub(super) fn service_devtools(&mut self) {
        if self.devtools.is_none() || !self.player_control_enabled {
            return;
        }
        let dx_started = Instant::now();
        let was_leased = self.control_state.leased;
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
        let refresh_debug = self.dx_snapshot_at.elapsed() > Duration::from_millis(250);
        if refresh_debug {
            self.dx_snapshot_at = Instant::now();
            let load = self.load_scheduler.metrics();
            let generation = self.generation_scheduler.metrics();
            let saves = self.save_scheduler.metrics();
            let player = self.player_save_scheduler.metrics();
            let global = self.world_state_save_scheduler.metrics();
            self.control_state.domains.streaming = serde_json::json!({"player_chunk":self.simulation.as_ref().map(|s|[(s.player.position.x.floor() as i32).div_euclid(16),(s.player.position.z.floor() as i32).div_euclid(16)]),"desired":self.residency.desired_column_count(),"desired_radius":self.residency.load_radius(),"resident":self.simulation.as_ref().map(|s|s.world.column_positions().count()),"retained_radius":self.residency.retain_radius(),"pending":self.residency.pending_column_count(),"safe":self.simulation.as_ref().map_or(0,|s|s.world.safe_column_positions().count()),"visible":self.render_ready_columns.len(),"idle":self.residency.pending_column_count()==0,"load":{"queued":load.queued,"inflight":load.in_flight,"failures":load.failed},"generation":{"queued":generation.pending,"inflight":generation.in_flight},"frontier":self.frontier_summary(),"oldest_critical_ms":self.stream_stage_fairness.iter().map(|stage|stage.max_oldest_age_ms).fold(0.0,f64::max)});
            self.control_state.domains.persistence = serde_json::json!({"columns":{"dirty":self.persistence_dirty.dirty_count(),"dirty_subset":self.persistence_dirty.diagnostic_entries(16).map(|(p,g,s)|serde_json::json!({"chunk":[p.x,p.z],"dirty_generation":g,"saving":s})).collect::<Vec<_>>(),"queued":saves.queued,"inflight":saves.in_flight,"failures":saves.failed},"player":{"revision":self.player_revision,"persisted":self.player_persisted_revision,"dirty":self.player_dirty,"inflight":player.in_flight,"failures":player.failures},"world":{"revision":self.world_state_revision,"persisted":self.world_state_persisted_revision,"dirty":self.world_state_dirty,"inflight":global.in_flight,"failures":global.failures}});
            self.control_state.domains.world = serde_json::json!({"generator":self.stream_generator.as_ref().map(|g|serde_json::json!({"id":g.id(),"version":g.version()}))});
            let mesh = self.mesh_scheduler.stats();
            self.control_state.domains.meshing = serde_json::json!({"idle":self.mesh_scheduler.is_idle(),"pending":mesh.pending,"inflight":mesh.in_flight,"ready":mesh.ready,"failed":mesh.mesh_jobs_failed,"workers":mesh.worker_count,"ready_bytes":mesh.ready_cpu_bytes});
            self.control_state.domains.renderer=self.renderer.as_ref().map_or(serde_json::Value::Null,|r|serde_json::json!({"draw_calls":r.draw_calls(),"mesh_rebuilds":r.mesh_rebuilds}));
            self.control_state.domains.scripts = serde_json::json!({"repl":tools.repl.diagnostic,"loaded":tools.loaded.values().map(|s|serde_json::json!({"path":s.path,"generation":s.generation,"error":s.last_error})).collect::<Vec<_>>(),"jobs":tools.jobs.snapshot(),"compile_pending":tools.compiling_scenario,"step":tools.scenario.as_ref().map(|s|s.cursor),"scenario":tools.scenario.as_ref().map(|s|&s.result),"limits":{"operations":50000,"deadline_ms":10},"update_us":self.dx_update_us,"update_max_us":self.dx_update_max_us,"frames":tools.frame});
            if let Some(simulation) = self.simulation.as_mut() {
                use rustcraft_control::Host;
                let snapshot = rustcraft_minecraft_b173::control::MinecraftHost {
                    simulation,
                    state: &mut self.control_state,
                }
                .snapshot();
                self.control_state.domains.player = snapshot.player;
                self.control_state.domains.entities = snapshot.entities;
                self.control_state.domains.world = snapshot.world;
                self.control_state.domains.lighting = {
                    let light = simulation.lighting.work_counters();
                    serde_json::json!({"backlog":simulation.lighting.has_integration_work(),"initial_pending":self.initial_lighting_scheduler.pending(),"columns_started":light.columns_started,"columns_completed":light.columns_completed,"queue_pushes":light.propagation_queue_pushes,"queue_pops":light.propagation_queue_pops})
                };
            }
        }
        if let Some(simulation) = self.simulation.as_mut() {
            let mut host = rustcraft_minecraft_b173::control::MinecraftHost {
                simulation,
                state: &mut self.control_state,
            };
            if let Some(path) = self.scenario_path.take()
                && let Err(error) = tools.start(&path, &mut host)
            {
                tools.print(&error);
                eprintln!("DX scenario start failed: {error}");
                self.dx_exit_pending = true;
            }
            if self
                .dx_abort_frame
                .is_some_and(|frame| tools.frame >= frame)
                && tools.scenario.as_ref().is_some_and(|s| s.active)
            {
                tools.abort(&mut host);
                self.dx_abort_frame = None;
            }
            match tools.advance(&mut host, true) {
                Ok(Some(bundle)) => {
                    println!("DX_RESULT {}", bundle.join("result.json").display());
                    self.control_state.fixed.pause();
                    self.dx_capture = Some(bundle.join("frame.png"));
                    self.dx_exit_pending = true;
                }
                Err(error) => {
                    tools.print(&error);
                    eprintln!("DX tooling error {error}; partial bundle may exist");
                    if let Some(s) = tools.scenario.as_mut() {
                        s.result.status = "error".into();
                        s.result.error = Some(error);
                    }
                    self.dx_exit_pending = true;
                }
                _ => {}
            }
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
            if let Some(simulation) = self.simulation.as_mut() {
                use rustcraft_control::Host;
                let snapshot = rustcraft_minecraft_b173::control::MinecraftHost {
                    simulation,
                    state: &mut self.control_state,
                }
                .snapshot();
                if let Err(error) = tools.write_snapshot(directory.clone(), snapshot) {
                    tools.print(&error);
                }
                self.dx_capture = Some(directory.join("frame.png"));
            }
        }
        self.dx_boxes.clear();
        self.dx_colors.clear();
        if let Some(sim) = self.simulation.as_ref() {
            use rustcraft_engine_core::Aabb;
            if self.control_state.overlays.contains("collision") {
                self.dx_boxes.push(sim.player.bounds());
                self.dx_colors.push([1., 0.8, 0.1]);
            }
            if self.control_state.overlays.contains("entities") {
                for item in sim.items.iter().take(12) {
                    let p = item.position;
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
        if refresh_debug || tools.console_open || self.dx_text.is_empty() {
            self.dx_text = tools.text(&self.control_state);
        }
        self.devtools = Some(tools);
        self.dx_update_us = dx_started.elapsed().as_micros();
        self.dx_update_max_us = self.dx_update_max_us.max(self.dx_update_us);
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
            match probe.phase {
                0 => probe.parked = self.devtools.take(),
                1 => {
                    self.devtools = probe.parked.take();
                    self.control_state.page.clear();
                    self.control_state.overlays.clear();
                }
                2 => self.control_state.page = "streaming".into(),
                3 => {
                    self.control_state.page.clear();
                    self.control_state
                        .overlays
                        .extend(["streaming".into(), "collision".into()]);
                }
                4 => {
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
            let report = serde_json::json!({"mode":(["disabled","inactive","page","overlay","scenario"][probe.phase]),"event_turns":probe.turns,"samples":probe.samples.len(),"mean_us":mean,"p50_us":quantile(50),"p95_us":quantile(95),"p99_us":quantile(99),"max_us":quantile(100),"render_frames":self.devtools.as_ref().map(|d|d.frame)});
            eprintln!("DX_OVERHEAD {report}");
            probe.reports.push(report);
            probe.samples.clear();
            probe.turns = 0;
            probe.phase += 1;
            if probe.phase == 5 {
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
