mod render_tests;
use rustcraft_agent_api::{AgentIntent, Controller, MoveIntent};
use rustcraft_content::{LocalResourceResolver, ResourceId};
use rustcraft_engine_core::{BlockId, Vec3};
mod bench;
mod debug;
mod gpu_metrics;
use rustcraft_minecraft_b173::blocks::BlocksModule;
use rustcraft_minecraft_b173::flat_world::FlatWorldModule;
use rustcraft_render::diagnostic::Stage;
use rustcraft_render::{BlockTextureResolver, Camera, Face, RenderWorld, Renderer, TextureTile};
use rustcraft_runtime::{RuntimeBootstrap, Simulation};
use std::{path::PathBuf, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

#[derive(Debug, Default)]
struct LocalHumanController {
    forward: f32,
    strafe: f32,
    forward_pressed: bool,
    back_pressed: bool,
    left_pressed: bool,
    right_pressed: bool,
    jump: bool,
    look: Vec3,
    break_held: bool,
    place_pressed: bool,
    captured: bool,
    selection: Option<u8>,
    scroll: i8,
    wheel_remainder: f64,
}

impl LocalHumanController {
    fn key(&mut self, code: KeyCode, state: ElementState) {
        let pressed = state == ElementState::Pressed;
        if pressed {
            self.selection = match code {
                KeyCode::Digit1 => Some(0),
                KeyCode::Digit2 => Some(1),
                KeyCode::Digit3 => Some(2),
                KeyCode::Digit4 => Some(3),
                KeyCode::Digit5 => Some(4),
                KeyCode::Digit6 => Some(5),
                KeyCode::Digit7 => Some(6),
                KeyCode::Digit8 => Some(7),
                KeyCode::Digit9 => Some(8),
                _ => self.selection,
            };
        }
        match code {
            KeyCode::KeyW => self.forward_pressed = pressed,
            KeyCode::KeyS => self.back_pressed = pressed,
            KeyCode::KeyA => self.left_pressed = pressed,
            KeyCode::KeyD => self.right_pressed = pressed,
            KeyCode::Space => self.jump = state == ElementState::Pressed,
            _ => {}
        }
        self.forward = if self.forward_pressed { 1.0 } else { 0.0 }
            - if self.back_pressed { 1.0 } else { 0.0 };
        self.strafe =
            if self.right_pressed { 1.0 } else { 0.0 } - if self.left_pressed { 1.0 } else { 0.0 };
    }
    fn wheel(&mut self, delta: winit::event::MouseScrollDelta) {
        let lines = match delta {
            winit::event::MouseScrollDelta::LineDelta(_, y) => f64::from(y),
            winit::event::MouseScrollDelta::PixelDelta(p) => p.y / 40.,
        };
        if !lines.is_finite() {
            return;
        }
        self.wheel_remainder += lines;
        let steps = self.wheel_remainder.trunc().clamp(-127., 127.) as i8;
        self.wheel_remainder -= f64::from(steps);
        self.scroll = self.scroll.saturating_sub(steps);
    }
    fn capture(&mut self, window: &Window) {
        let _ = window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
        window.set_cursor_visible(false);
        self.captured = true;
    }
    fn release(&mut self, window: &Window) {
        let _ = window.set_cursor_grab(CursorGrabMode::None);
        window.set_cursor_visible(true);
        self.captured = false;
        self.look = Vec3::ZERO;
    }
}
impl Controller for LocalHumanController {
    fn next_intent(&mut self) -> AgentIntent {
        let intent = AgentIntent {
            movement: MoveIntent {
                forward: self.forward,
                strafe: self.strafe,
            },
            look_delta: self.look,
            jump: self.jump,
            primary_action: self.break_held,
            secondary_action: self.place_pressed,
            attack: self.break_held,
            use_action: self.place_pressed,
            select_hotbar: self.selection.take(),
            scroll_hotbar: std::mem::take(&mut self.scroll),
            ..Default::default()
        };
        self.look = Vec3::ZERO;
        self.place_pressed = false;
        intent
    }
}

struct ClientApp {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    simulation: Option<Simulation>,
    presentation: Option<RenderWorld>,
    controller: LocalHumanController,
    clock: rustcraft_runtime::metrics::FixedStepClock,
    last_frame: Instant,
    texture_path: PathBuf,
    diagnostic: Option<Stage>,
    capture: Option<PathBuf>,
    debug: bool,
    metrics: rustcraft_runtime::metrics::Metrics,
    process: rustcraft_runtime::metrics::ProcessSampler,
    last_render: Instant,
    debug_text: String,
    last_snapshot: Instant,
    gpu_metrics: Option<gpu_metrics::Provider>,
    measure_seconds: Option<f64>,
    survival_start: bool,
    inventory_open: bool,
    cursor_stack: Option<rustcraft_runtime::inventory::ItemStack>,
    cursor_position: [f32; 2],
}
impl ClientApp {
    fn new(diagnostic: Option<Stage>, capture: Option<PathBuf>) -> Self {
        Self {
            window: None,
            renderer: None,
            simulation: None,
            presentation: None,
            controller: LocalHumanController::default(),
            clock: Default::default(),
            last_frame: Instant::now(),
            texture_path: LocalResourceResolver::from_environment("reference/assets/terrain.png")
                .resolve(&ResourceId("terrain.png".into())),
            diagnostic,
            capture,
            debug: std::env::var_os("RUSTCRAFT_F3").is_some(),
            metrics: Default::default(),
            process: Default::default(),
            last_render: Instant::now(),
            debug_text: String::new(),
            last_snapshot: Instant::now() - std::time::Duration::from_secs(1),
            gpu_metrics: None,
            measure_seconds: std::env::var("RUSTCRAFT_MEASURE_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok()),
            survival_start: false,
            inventory_open: false,
            cursor_stack: None,
            cursor_position: [0.; 2],
        }
    }
    fn start_world(&mut self) -> Result<(), String> {
        rustcraft_minecraft_b173::validate_package()
            .map_err(|error| format!("minecraft_b173 game profile: {error:?}"))?;
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        let blocks = BlocksModule;
        let flat = FlatWorldModule::default();
        bootstrap
            .register_module(&blocks)
            .map_err(|e| format!("block registration: {e:?}"))?;
        bootstrap
            .register_module(&flat)
            .map_err(|e| format!("flat-world registration: {e:?}"))?;
        let mut world = rustcraft_engine_core::World::new(BlockId(0));
        flat.generate(&mut world, -16, 16, -16, 16);
        if self.diagnostic.is_none() {
            rustcraft_minecraft_b173::flat_world::decorate_sandbox(&mut world, &bootstrap.registry);
        }
        let spawn = Simulation::spawn_above_surface(&world, &bootstrap.registry, 0, 0);
        let mut simulation = Simulation::new(world, bootstrap.registry, spawn);
        if !self.survival_start {
            for name in rustcraft_minecraft_b173::blocks::DEVELOPMENT_LOADOUT {
                let item = simulation
                    .registry
                    .by_name(name)
                    .and_then(|b| b.item)
                    .expect("development loadout item registered");
                simulation.inventory.insert(item, 64, &simulation.registry);
            }
        } else {
            simulation.set_survival();
        }
        eprintln!(
            "player spawn: position={spawn:?} surface_y={:.1}",
            spawn.y - 2.0
        );
        eprintln!("player AABB at spawn: {:?}", simulation.player.bounds());
        let camera = camera_for(&simulation, 16.0 / 9.0);
        let (right, up, forward) = camera.basis();
        eprintln!(
            "camera: player_xyz={:?} eye_xyz={:?} eye_offset=1.62 yaw={} pitch={} forward={forward:?} right={right:?} up={up:?}",
            simulation.player.position, camera.position, camera.yaw, camera.pitch
        );
        let dot = |a: Vec3, b: Vec3| a.x * b.x + a.y * b.y + a.z * b.z;
        eprintln!(
            "camera basis: dot(forward,up)={} dot(right,up)={} dot(forward,right)={} lengths=({},{},{})",
            dot(forward, up),
            dot(right, up),
            dot(forward, right),
            dot(forward, forward).sqrt(),
            dot(right, right).sqrt(),
            dot(up, up).sqrt()
        );
        let presentation = RenderWorld::from_world(&simulation.world);
        self.simulation = Some(simulation);
        self.presentation = Some(presentation);
        Ok(())
    }
    fn debug_key(&mut self, code: KeyCode, state: ElementState, repeat: bool) -> bool {
        if code != KeyCode::F3 {
            return false;
        }
        if state == ElementState::Pressed && !repeat {
            self.debug = !self.debug;
        }
        true
    }
    fn fixed_step(&mut self) {
        let Some(simulation) = self.simulation.as_mut() else {
            return;
        };
        let intent = if self.inventory_open {
            rustcraft_agent_api::AgentIntent::default()
        } else {
            self.controller.next_intent()
        };
        let tick_started = Instant::now();
        simulation.step(intent, 0.05);
        self.metrics.tick(tick_started.elapsed().as_secs_f64());
        let dirty = simulation.take_dirty_sections();
        if let Some(presentation) = self.presentation.as_mut() {
            presentation.sync_sections(&simulation.world, dirty.iter().copied());
        }
        self.rebuild_dirty_meshes(dirty);
    }
    fn rebuild_meshes(&mut self) {
        let (Some(renderer), Some(presentation)) =
            (self.renderer.as_mut(), self.presentation.as_ref())
        else {
            return;
        };
        let started = Instant::now();
        for chunk in presentation.chunks() {
            let mesh_started = Instant::now();
            let mesh = rustcraft_render::build_chunk_mesh(presentation, chunk, &FirstPartyTextures);
            renderer.upload_chunk(chunk.position, chunk.section_y, &mesh);
            self.metrics.mesh(mesh_started.elapsed().as_secs_f64());
        }
        eprintln!(
            "client diagnostics: chunks={} mesh_rebuilds={} vertices={} indices={} initial_mesh_ms={:.3}",
            presentation.chunk_count(),
            renderer.mesh_rebuilds,
            renderer.vertices,
            renderer.indices,
            started.elapsed().as_secs_f64() * 1000.0
        );
        if let Some(sim) = self.simulation.as_mut() {
            sim.take_dirty_sections();
            sim.take_dirty_chunks();
        }
    }
    fn rebuild_dirty_meshes(
        &mut self,
        dirty: impl IntoIterator<Item = rustcraft_engine_core::SectionPos>,
    ) {
        let (Some(renderer), Some(presentation)) =
            (self.renderer.as_mut(), self.presentation.as_ref())
        else {
            return;
        };
        for (position, section_y) in dirty {
            let chunks = presentation
                .chunks()
                .filter(|chunk| chunk.position == position && chunk.section_y == section_y)
                .collect::<Vec<_>>();
            for chunk in chunks {
                let started = Instant::now();
                let mesh =
                    rustcraft_render::build_chunk_mesh(presentation, chunk, &FirstPartyTextures);
                renderer.upload_chunk(position, chunk.section_y, &mesh);
                self.metrics.mesh(started.elapsed().as_secs_f64());
            }
        }
    }
    fn render(&mut self, event_loop: &ActiveEventLoop) {
        self.metrics
            .frames
            .push(self.last_render.elapsed().as_secs_f64());
        self.last_render = Instant::now();
        self.metrics.update();
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let aspect = renderer.width() as f32 / renderer.height().max(1) as f32;
        renderer.telemetry_enabled = self.debug || self.measure_seconds.is_some();
        if self.debug || self.measure_seconds.is_some() {
            self.process.sample();
            if let Some(g) = &mut self.gpu_metrics {
                g.sample();
            }
        }
        let camera = if let Some(stage) = self.diagnostic.filter(|s| !s.normal_world()) {
            stage.camera(aspect)
        } else if let Some(simulation) = self.simulation.as_ref() {
            camera_for(simulation, aspect)
        } else {
            return;
        };
        if self.diagnostic.is_none()
            && let Some(sim) = self.simulation.as_ref()
        {
            if self.debug && self.last_snapshot.elapsed().as_millis() >= 250 {
                self.process.sample();
                self.last_snapshot = Instant::now();
                self.debug_text = debug::DebugMetricsSnapshot::collect(
                    sim,
                    renderer,
                    &self.metrics,
                    &self.process.snapshot,
                    self.gpu_metrics.as_ref().map(|g| &g.snapshot),
                )
                .text;
                if self.inventory_open {
                    self.debug_text = format!(
                        "INVENTORY (E closes)\n{}\n{}",
                        inventory_text(sim),
                        self.debug_text
                    );
                }
            }
            let slots = std::array::from_fn(|i| {
                let stack = sim.inventory.slots()[i]?;
                let block = sim.registry.item(stack.item)?.placeable?;
                Some(rustcraft_render::hud::Slot {
                    model: rustcraft_render::inspection::BlockModel::resolve(
                        rustcraft_engine_core::BlockState::new(block),
                        &FirstPartyTextures,
                    ),
                    top: FirstPartyTextures.texture(block, Face::Top)?,
                    side: FirstPartyTextures.texture(block, Face::North)?,
                    bottom: FirstPartyTextures.texture(block, Face::Bottom)?,
                    tint: FirstPartyTextures.tint(block, Face::North),
                    count: stack.count,
                    block_3d: true,
                })
            });
            let visual = |stack: rustcraft_runtime::inventory::ItemStack| {
                let placeable = sim
                    .registry
                    .item(stack.item)
                    .and_then(|item| item.placeable);
                let block = placeable.unwrap_or(BlockId(5));
                Some(rustcraft_render::hud::Slot {
                    model: rustcraft_render::inspection::BlockModel::resolve(
                        rustcraft_engine_core::BlockState::new(block),
                        &FirstPartyTextures,
                    ),
                    top: FirstPartyTextures.texture(block, Face::Top)?,
                    side: FirstPartyTextures.texture(block, Face::North)?,
                    bottom: FirstPartyTextures.texture(block, Face::Bottom)?,
                    tint: FirstPartyTextures.tint(block, Face::North),
                    count: stack.count,
                    block_3d: placeable.is_some(),
                })
            };
            let inventory_slots =
                std::array::from_fn(|i| sim.inventory.slots()[i].and_then(visual));
            let crafting_slots = std::array::from_fn(|i| sim.crafting_grid[i].and_then(visual));
            let crafting_output = sim.crafting_output().and_then(visual);
            let cursor_slot = self.cursor_stack.and_then(visual);
            let item_sprites = sim
                .items
                .iter()
                .filter_map(|e| {
                    let block = sim
                        .registry
                        .item(e.stack.item)
                        .and_then(|i| i.placeable)
                        .unwrap_or(BlockId(5));
                    Some(rustcraft_render::ItemSprite {
                        model: rustcraft_render::inspection::BlockModel::resolve(
                            rustcraft_engine_core::BlockState::new(block),
                            &FirstPartyTextures,
                        ),
                        position: e.position,
                        top: FirstPartyTextures.texture(block, Face::Top)?,
                        side: FirstPartyTextures.texture(block, Face::North)?,
                        bottom: FirstPartyTextures.texture(block, Face::Bottom)?,
                        tint: FirstPartyTextures.tint(block, Face::North),
                        // Entity age is simulation seconds; Beta RenderItem consumes ticks.
                        age: e.age * 20.0 + self.clock.alpha(),
                        count: e.stack.count,
                        hover_start: (e.id as f32 * 0.61803395).fract(),
                    })
                })
                .collect::<Vec<_>>();
            renderer.set_item_sprites(&item_sprites);
            let mining_target = sim.mining.map(|m| m.target);
            renderer.set_crack_overlay(
                if self.inventory_open {
                    None
                } else {
                    mining_target
                },
                if self.inventory_open {
                    None
                } else {
                    sim.mining.map(|m| m.progress)
                },
            );
            renderer.set_hud(
                &rustcraft_render::hud::HudSnapshot {
                    slots,
                    selected: sim.inventory.selected(),
                    target: sim.target().map(|h| h.block),
                    text: if self.debug { &self.debug_text } else { "" },
                    items: &[],
                    mining_progress: sim.mining.map(|m| m.progress),
                    inventory_open: self.inventory_open,
                    inventory_slots,
                    crafting_slots,
                    crafting_output,
                    cursor_slot,
                    cursor_position: self.cursor_position,
                },
                camera,
            );
        }
        let finished = self
            .measure_seconds
            .is_some_and(|s| self.metrics.uptime.elapsed().as_secs_f64() >= s);
        let capture = if self.measure_seconds.is_none() || finished {
            self.capture.as_deref()
        } else {
            None
        };
        match renderer.render_capture(camera, capture) {
            Ok(()) => {
                if finished {
                    eprintln!(
                        "M2 MEASURE overlay={} fps={:?} low={:?} frame_ms={:?} tps={:?} tick_ms={:?} process={:?} gpu_ms={:?} gpu={:?} mesh_count={} rebuilds={} mesh_ms={:?}",
                        self.debug,
                        self.metrics.frames.fps(),
                        self.metrics.frames.low(),
                        self.metrics.frames.mean().map(|v| v * 1000.),
                        self.metrics.tps,
                        self.metrics.ticks.mean().map(|v| v * 1000.),
                        self.process.snapshot,
                        renderer.gpu_ms(),
                        self.gpu_metrics.as_ref().map(|g| &g.snapshot),
                        renderer.mesh_count(),
                        renderer.mesh_rebuilds,
                        self.metrics.meshes.mean().map(|v| v * 1000.)
                    );
                }
                if capture.is_some() || finished {
                    event_loop.exit();
                }
            }
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                renderer.resize(renderer.width(), renderer.height())
            }
            Err(wgpu::SurfaceError::Timeout) => {}
            Err(wgpu::SurfaceError::OutOfMemory) => std::process::exit(1),
            Err(_) => {}
        }
    }
}

impl ClientApp {
    fn inventory_click(&mut self, slot: usize) {
        if let Some(sim) = self.simulation.as_mut() {
            self.cursor_stack =
                sim.inventory
                    .slot_click(Some(slot), 0, self.cursor_stack, &sim.registry);
        }
    }
    fn inventory_click_at(&mut self, x: f32, y: f32, right: bool) {
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        let scale = rustcraft_render::hud::beta_gui_scale(renderer.width(), renderer.height());
        let panel_w = 176. * scale;
        let panel_h = 166. * scale;
        let panel_x = (renderer.width() as f32 - panel_w) / 2.;
        let panel_y = (renderer.height() as f32 - panel_h) / 2.;
        let slot = 18. * scale;
        let col = ((x - (panel_x + 8. * scale)) / (18. * scale)).floor() as i32;
        let row = ((y - (panel_y + 84. * scale)) / (18. * scale)).floor() as i32;
        if (0..9).contains(&col) && (0..4).contains(&row) {
            let sx = panel_x + (8. + col as f32 * 18.) * scale;
            let sy = panel_y + (84. + row as f32 * 18.) * scale;
            if x >= sx && x <= sx + slot && y >= sy && y <= sy + slot {
                let index = if row == 3 {
                    col as usize
                } else {
                    9 + row as usize * 9 + col as usize
                };
                if rustcraft_render::hud::inventory_slot_position(index).is_some() {
                    if right {
                        if let Some(sim) = self.simulation.as_mut() {
                            self.cursor_stack = sim.inventory.slot_click(
                                Some(index),
                                1,
                                self.cursor_stack,
                                &sim.registry,
                            );
                        }
                    } else {
                        self.inventory_click(index);
                    }
                }
                return;
            }
        }
        let craft_x = panel_x + 88. * scale;
        let craft_y = panel_y + 26. * scale;
        for i in 0..4 {
            let sx = craft_x + (i % 2) as f32 * 18. * scale;
            let sy = craft_y + (i / 2) as f32 * 18. * scale;
            if x >= sx && x <= sx + slot && y >= sy && y <= sy + slot {
                if let Some(sim) = self.simulation.as_mut() {
                    let held = self.cursor_stack.take();
                    self.cursor_stack = sim.swap_crafting_slot(i, held);
                }
                return;
            }
        }
        let output_x = panel_x + 144. * scale;
        let output_y = panel_y + 36. * scale;
        if x >= output_x
            && x <= output_x + slot
            && y >= output_y
            && y <= output_y + slot
            && self.cursor_stack.is_none()
            && let Some(sim) = self.simulation.as_mut()
        {
            self.cursor_stack = sim.take_crafting_output_for_cursor();
        }
    }
}
fn inventory_text(sim: &Simulation) -> String {
    sim.inventory
        .slots()
        .iter()
        .enumerate()
        .map(|(i, s)| {
            format!(
                "{}:{}",
                i + 1,
                s.and_then(|x| sim.registry.item(x.item).map(|d| d.name))
                    .unwrap_or("empty")
            )
        })
        .collect::<Vec<_>>()
        .chunks(9)
        .map(|r| r.join(" | "))
        .collect::<Vec<_>>()
        .join("\n")
}

impl ApplicationHandler for ClientApp {
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // Surface/EGL teardown needs the Wayland connection still alive.
        // run_app consumes/drops the event loop before returning to main.
        self.renderer.take();
        self.window.take();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(self.diagnostic.map_or_else(
                            || "RustCraft M2".to_owned(),
                            |stage| format!("RustCraft diagnostic: {stage:?}"),
                        ))
                        .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720)),
                )
                .expect("create window"),
        );
        if self.diagnostic.is_none_or(Stage::normal_world)
            && let Err(error) = self.start_world()
        {
            eprintln!("unable to initialize local world: {error}");
            event_loop.exit();
            return;
        }
        let result = if let Some(stage) = self.diagnostic {
            eprintln!(
                "diagnostic stage={stage:?}; camera={}; lighting={}; select next stage only after visual acceptance",
                if stage.normal_world() {
                    "player"
                } else {
                    "fixed look-at"
                },
                if stage == Stage::NormalLit {
                    "face brightness"
                } else {
                    "off"
                }
            );
            pollster::block_on(Renderer::new_diagnostic(
                window.clone(),
                &self.texture_path,
                stage,
            ))
        } else {
            pollster::block_on(Renderer::new(window.clone(), &self.texture_path))
        };
        let renderer = match result {
            Ok(renderer) => renderer,
            Err(error) => {
                eprintln!("unable to initialize graphics: {error}");
                std::process::exit(1);
            }
        };
        self.window = Some(window);
        self.gpu_metrics = Some(gpu_metrics::Provider::new(
            renderer.adapter_info.vendor,
            renderer.adapter_info.device,
        ));
        self.renderer = Some(renderer);
        if let Some(stage) = self.diagnostic.filter(|s| !s.normal_world()) {
            let mesh = stage.mesh();
            self.renderer.as_mut().unwrap().upload_chunk(
                rustcraft_engine_core::ChunkPos { x: 0, z: 0 },
                0,
                &mesh,
            );
            eprintln!(
                "diagnostic geometry: vertices={} indices={}",
                mesh.vertices.len(),
                mesh.indices.len()
            );
        } else {
            self.rebuild_meshes();
        }
        self.metrics.uptime = Instant::now();
        self.last_frame = Instant::now();
        self.last_render = Instant::now();
    }
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.clone() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => self.render(event_loop),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = [position.x as f32, position.y as f32];
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.controller.wheel(delta);
            }
            WindowEvent::Focused(false) => {
                self.controller.break_held = false;
                self.controller.place_pressed = false;
                if self.controller.captured {
                    self.controller.release(&window);
                } else {
                    self.controller.look = Vec3::ZERO;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if self.debug_key(code, event.state, event.repeat) {
                        // Client-only shortcut consumed before semantic gameplay input.
                    } else if code == KeyCode::KeyE
                        && event.state == ElementState::Pressed
                        && !event.repeat
                    {
                        self.inventory_open = !self.inventory_open;
                        if self.inventory_open {
                            self.controller.release(&window);
                        } else {
                            self.controller.capture(&window);
                        }
                    } else if code == KeyCode::Escape && event.state == ElementState::Pressed {
                        if self.controller.captured {
                            self.controller.release(&window);
                        } else {
                            event_loop.exit();
                        }
                    } else if code == KeyCode::KeyC
                        && event.state == ElementState::Pressed
                        && !event.repeat
                        && self.inventory_open
                    {
                        if let Some(sim) = self.simulation.as_mut() {
                            let _ = sim.craft_first_available("log_to_planks");
                        }
                    } else {
                        self.controller.key(code, event.state);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. }
                if self.diagnostic.is_none_or(Stage::normal_world) =>
            {
                if self.inventory_open {
                    if state == ElementState::Pressed
                        && (button == MouseButton::Left || button == MouseButton::Right)
                    {
                        self.inventory_click_at(
                            self.cursor_position[0],
                            self.cursor_position[1],
                            button == MouseButton::Right,
                        );
                    }
                    return;
                }
                if state == ElementState::Released && button == MouseButton::Left {
                    self.controller.break_held = false;
                } else if state == ElementState::Released && button == MouseButton::Right {
                    self.controller.place_pressed = false;
                } else if !self.controller.captured {
                    self.controller.capture(&window);
                } else if button == MouseButton::Left {
                    self.controller.break_held = state == ElementState::Pressed;
                } else if button == MouseButton::Right {
                    self.controller.place_pressed = state == ElementState::Pressed;
                }
            }
            _ => {}
        }
    }
    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if self.controller.captured
            && let DeviceEvent::MouseMotion { delta } = event
        {
            self.controller.look.x += delta.0 as f32;
            self.controller.look.y += delta.1 as f32;
        }
    }
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame).as_secs_f64();
        self.last_frame = now;
        let budget = self.clock.advance(elapsed);
        self.metrics.steps = budget.steps;
        self.metrics.catch_up = budget.catch_up;
        for _ in 0..budget.steps {
            self.fixed_step();
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

struct FirstPartyTextures;
impl BlockTextureResolver for FirstPartyTextures {
    fn model_rotation(
        &self,
        state: rustcraft_engine_core::BlockState,
    ) -> rustcraft_engine_core::orientation::ModelRotation {
        rustcraft_minecraft_b173::blocks::BLOCKS
            .iter()
            .find(|b| b.id == state.block)
            .map_or(
                rustcraft_engine_core::orientation::ModelRotation::IDENTITY,
                |b| {
                    b.base_model_rotation
                        .compose(state.model_rotation(b.orientation_property))
                },
            )
    }

    fn tint(&self, block: BlockId, face: Face) -> [f32; 3] {
        rustcraft_minecraft_b173::blocks::tint(block, face == Face::Top)
    }

    fn texture(&self, block: BlockId, face: Face) -> Option<TextureTile> {
        let definition = rustcraft_minecraft_b173::blocks::BLOCKS
            .iter()
            .find(|b| b.id == block)?;
        let (x, y) =
            rustcraft_minecraft_b173::blocks::atlas_tile(definition.textures.face(face as usize))?;
        Some(TextureTile { x, y })
    }
    fn opaque(&self, block: BlockId) -> bool {
        rustcraft_minecraft_b173::blocks::BLOCKS
            .iter()
            .any(|b| b.id == block && b.material == rustcraft_mod_api::Material::Opaque)
    }
    fn visible(&self, block: BlockId) -> bool {
        block.0 != 0
    }
}
fn camera_for(simulation: &Simulation, aspect: f32) -> Camera {
    Camera {
        position: Vec3::new(
            simulation.player.position.x,
            simulation.player.position.y + 1.62,
            simulation.player.position.z,
        ),
        yaw: simulation.player.yaw,
        pitch: simulation.player.pitch,
        aspect,
        fov_y: 70.0_f32.to_radians(),
        near: 0.05,
        far: 256.0,
    }
}
fn main() {
    if render_tests::run() {
        return;
    }
    if std::env::args().any(|a| a == "--bench-m3") {
        bench::run_m3();
        return;
    }
    if std::env::args().any(|a| a == "--bench-m2") {
        bench::run();
        return;
    }
    let mut args = std::env::args().skip(1);
    let mut diagnostic = None;
    let mut capture = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--survival" => {}
            "--fidelity-m3" => {}
            "--diag" => {
                if let Some(stage) = args.next() {
                    diagnostic = Some(Stage::parse(&stage).unwrap_or_else(|error| {
                        eprintln!("{error}");
                        std::process::exit(2)
                    }))
                } else {
                    eprintln!(
                        "usage: rustcraft-client [--survival] [--fidelity-m3] [--diag STAGE] [--capture NEW.png]"
                    );
                    std::process::exit(2);
                }
            }
            "--capture" => {
                if let Some(path) = args.next() {
                    capture = Some(PathBuf::from(path));
                } else {
                    eprintln!(
                        "usage: rustcraft-client [--survival] [--fidelity-m3] [--diag STAGE] [--capture NEW.png]"
                    );
                    std::process::exit(2);
                }
            }
            _ => {
                eprintln!(
                    "usage: rustcraft-client [--survival] [--fidelity-m3] [--diag STAGE] [--capture NEW.png]"
                );
                std::process::exit(2);
            }
        }
    }
    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = ClientApp::new(diagnostic, capture);
    app.survival_start = std::env::args().any(|a| a == "--survival");
    let _ = event_loop.run_app(&mut app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_engine_core::BlockPos;
    use rustcraft_minecraft_b173::blocks::GRASS;
    use rustcraft_minecraft_b173::blocks::STONE;

    #[test]
    fn gameplay_camera_and_controls_share_right_and_down_conventions() {
        let mut app = ClientApp::new(None, None);
        app.start_world().unwrap();
        let sim = app.simulation.as_mut().unwrap();
        let initial = sim.player.position;
        let (right, _, _) = camera_for(sim, 1.0).basis();
        sim.step(
            AgentIntent {
                movement: MoveIntent {
                    forward: 0.0,
                    strafe: 1.0,
                },
                ..Default::default()
            },
            0.05,
        );
        let dx = sim.player.position.x - initial.x;
        let dz = sim.player.position.z - initial.z;
        assert!(dx * right.x + dz * right.z > 0.0);
        sim.step(
            AgentIntent {
                look_delta: Vec3::new(20.0, 20.0, 0.0),
                ..Default::default()
            },
            0.05,
        );
        let camera = camera_for(sim, 1.0);
        assert!(camera.forward().x * right.x > 0.0); // Mouse right turns right.
        assert!(camera.forward().y < 0.0); // Mouse down looks down.
        assert_eq!(camera.position.y, sim.player.position.y + 1.62);
        assert!(camera.position.y > sim.player.position.y);
    }

    #[test]
    fn opposite_keys_preserve_the_key_still_held() {
        let mut controller = LocalHumanController::default();
        controller.key(KeyCode::KeyW, ElementState::Pressed);
        controller.key(KeyCode::KeyS, ElementState::Pressed);
        assert_eq!(controller.forward, 0.0);
        controller.key(KeyCode::KeyW, ElementState::Released);
        assert_eq!(controller.forward, -1.0);
        controller.key(KeyCode::KeyS, ElementState::Released);
        assert_eq!(controller.forward, 0.0);
        controller.key(KeyCode::KeyA, ElementState::Pressed);
        controller.key(KeyCode::KeyD, ElementState::Pressed);
        controller.key(KeyCode::KeyA, ElementState::Released);
        assert_eq!(controller.strafe, 1.0);
    }

    #[test]
    fn dirty_extraction_remeshes_the_changed_section_after_rendering_is_restored() {
        let mut app = ClientApp::new(None, None);
        app.start_world().unwrap();
        let sim = app.simulation.as_mut().unwrap();
        let position = BlockPos { x: 1, y: 2, z: 1 };
        assert!(sim.place_block(position, STONE.id));
        assert_eq!(sim.world.get(position), STONE.id);
        let dirty = sim.take_dirty_chunks();
        assert!(!dirty.is_empty());
        let presentation = app.presentation.as_mut().unwrap();
        presentation.sync_dirty(&sim.world, dirty);
        assert_eq!(presentation.block(position), STONE.id);
        let chunk = presentation
            .chunks()
            .find(|c| c.position.x == 0 && c.position.z == 0 && c.section_y == 0)
            .unwrap();
        let mesh = rustcraft_render::build_chunk_mesh(presentation, chunk, &FirstPartyTextures);
        assert!(mesh.vertices.iter().any(|v| v.position == [1.0, 3.0, 1.0]));
    }

    #[test]
    fn grass_texture_mapping_matches_world_faces() {
        let textures = FirstPartyTextures;
        let tint = textures.tint(GRASS.id, Face::Top);
        assert!(tint[1] > tint[0] && tint[0] > tint[2]);
        assert_eq!(textures.tint(GRASS.id, Face::North), [1.0; 3]);
        assert_eq!(textures.tint(GRASS.id, Face::Bottom), [1.0; 3]);
        assert_eq!(textures.tint(STONE.id, Face::Top), [1.0; 3]);
        assert_eq!(
            textures.texture(GRASS.id, Face::Top),
            Some(TextureTile { x: 0, y: 0 })
        );
        assert_eq!(
            textures.texture(GRASS.id, Face::Bottom),
            Some(TextureTile { x: 2, y: 0 })
        );
        assert_eq!(
            textures.texture(GRASS.id, Face::North),
            Some(TextureTile { x: 3, y: 0 })
        );
    }
}

#[cfg(test)]
mod m2_input_tests {
    use super::*;
    #[test]
    fn attack_is_held_until_mouse_release() {
        let mut c = LocalHumanController {
            break_held: true,
            ..Default::default()
        };
        assert!(c.next_intent().attack);
        assert!(c.next_intent().attack);
        c.break_held = false;
        assert!(!c.next_intent().attack);
    }
    #[test]
    fn digits_and_wheel_are_semantic_one_shot_selection() {
        let mut c = LocalHumanController::default();
        for (i, key) in [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ]
        .into_iter()
        .enumerate()
        {
            c.key(key, ElementState::Pressed);
            assert_eq!(c.next_intent().select_hotbar, Some(i as u8));
            assert_eq!(c.next_intent().select_hotbar, None);
        }
        c.wheel(winit::event::MouseScrollDelta::LineDelta(0., 0.));
        assert_eq!(c.next_intent().scroll_hotbar, 0);
        c.wheel(winit::event::MouseScrollDelta::LineDelta(0., 2.));
        assert_eq!(c.next_intent().scroll_hotbar, -2);
        for _ in 0..4 {
            c.wheel(winit::event::MouseScrollDelta::PixelDelta(
                winit::dpi::PhysicalPosition::new(0., 10.),
            ));
        }
        assert_eq!(c.next_intent().scroll_hotbar, -1);
        assert_eq!(c.next_intent().scroll_hotbar, 0);
    }
    #[test]
    fn f3_is_client_state_and_ignores_repeat_and_release() {
        let mut app = ClientApp::new(None, None);
        app.debug = false;
        assert!(app.debug_key(KeyCode::F3, ElementState::Pressed, false));
        assert!(app.debug);
        app.debug_key(KeyCode::F3, ElementState::Pressed, true);
        app.debug_key(KeyCode::F3, ElementState::Released, false);
        assert!(app.debug);
        app.debug_key(KeyCode::F3, ElementState::Pressed, false);
        assert!(!app.debug);
        assert_eq!(app.controller.next_intent(), AgentIntent::default());
    }
}
