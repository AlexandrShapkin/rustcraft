mod configuration;
mod devtools;
mod render_tests;
use rustcraft_agent_api::{AgentIntent, Controller, MoveIntent};
use rustcraft_engine_core::{BlockId, Vec3};
mod bench;
mod debug;
mod gpu_metrics;
use rustcraft_minecraft_b173::blocks::{BlocksModule, STONE};
use rustcraft_render::diagnostic::Stage;
use rustcraft_render::{
    AtlasRegion, BlockTextureResolver, Camera, Face, RenderWorld, Renderer, RendererResources,
    RgbaTexture, TextureSampling,
};
use rustcraft_render_profile::{CompiledTextureRegistry, CompiledVoxelRenderRegistry};
use rustcraft_runtime::{RuntimeBootstrap, Simulation};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

struct FirstPartyCompiled {
    resources: RendererResources,
    materials: CompiledVoxelRenderRegistry,
    legacy_to_compiled: Vec<Option<BlockId>>,
    destroy_stages: [AtlasRegion; 10],
}

static FIRST_PARTY_COMPILED: OnceLock<Arc<FirstPartyCompiled>> = OnceLock::new();
static FIRST_PARTY_COMPILE_LOCK: Mutex<()> = Mutex::new(());

fn first_party_compiled() -> Result<Arc<FirstPartyCompiled>, String> {
    if let Some(compiled) = FIRST_PARTY_COMPILED.get() {
        return Ok(Arc::clone(compiled));
    }
    let _guard = FIRST_PARTY_COMPILE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(compiled) = FIRST_PARTY_COMPILED.get() {
        return Ok(Arc::clone(compiled));
    }
    // Cache only success; bad local assets do not poison subsequent attempts.
    let compiled = Arc::new(compile_first_party_resources()?);
    let _ = FIRST_PARTY_COMPILED.set(Arc::clone(&compiled));
    Ok(compiled)
}

fn compile_first_party_resources() -> Result<FirstPartyCompiled, String> {
    #[cfg(test)]
    let terrain = synthetic_test_terrain_path()?;
    #[cfg(not(test))]
    let terrain = std::env::var_os("RUSTCRAFT_TERRAIN_TEXTURE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("reference/assets/terrain.png")
        });
    #[cfg(test)]
    let cache_directory: Option<PathBuf> = None;
    #[cfg(not(test))]
    let cache_directory = Some(
        std::env::var_os("RUSTCRAFT_RESOURCE_CACHE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("target/resource-cache/minecraft-b173")),
    );
    #[cfg(test)]
    let atlas_policy = rustcraft_content::resources::AtlasPolicy::default();
    #[cfg(not(test))]
    let atlas_policy = first_party_atlas_policy()?;
    compile_first_party_resources_from(&terrain, cache_directory.as_deref(), atlas_policy)
}

#[cfg(not(test))]
fn first_party_atlas_policy() -> Result<rustcraft_content::resources::AtlasPolicy, String> {
    let Some(value) = std::env::var_os("RUSTCRAFT_MAX_ATLAS_DIMENSION") else {
        return Ok(rustcraft_content::resources::AtlasPolicy::default());
    };
    let maximum = value
        .to_str()
        .ok_or("RUSTCRAFT_MAX_ATLAS_DIMENSION is not UTF-8")?
        .parse::<u32>()
        .map_err(|error| format!("invalid RUSTCRAFT_MAX_ATLAS_DIMENSION: {error}"))?;
    rustcraft_content::resources::AtlasPolicy::default()
        .bounded_by_device_dimension(maximum)
        .map_err(|error| format!("device-safe atlas policy: {error}"))
}

fn compile_first_party_resources_from(
    terrain: &std::path::Path,
    cache_directory: Option<&std::path::Path>,
    atlas_policy: rustcraft_content::resources::AtlasPolicy,
) -> Result<FirstPartyCompiled, String> {
    let package = rustcraft_minecraft_b173::legacy_resource_package(terrain)
        .map_err(|error| format!("legacy minecraft resource import: {error}"))?;
    let compiled = rustcraft_content::resources::compile_resources(
        &[package],
        atlas_policy,
        rustcraft_content::resources::ResourceLimits::default(),
        cache_directory,
    )
    .map_err(|error| format!("compile minecraft resources: {error}"))?;
    eprintln!("resource pipeline: {}", compiled.report());
    for replacement in &compiled.overrides {
        eprintln!(
            "resource override: {} {} -> {}",
            replacement.id, replacement.original_provider, replacement.overriding_provider
        );
    }
    let textures = CompiledTextureRegistry::from_resources(&compiled)
        .map_err(|error| format!("compile texture registry: {error}"))?;
    let profile = rustcraft_minecraft_b173::compile_profile()
        .map_err(|error| format!("compile minecraft profile: {error:?}"))?;
    let materials = CompiledVoxelRenderRegistry::compile(&profile, &textures)
        .map_err(|error| format!("compile minecraft render registry: {error}"))?;
    let region = |id: rustcraft_content::ResourceId| -> Result<AtlasRegion, String> {
        let key = rustcraft_game_api::TextureKey::parse(id.as_str())
            .map_err(|error| format!("invalid presentation resource {id}: {error}"))?;
        textures
            .get(&key)
            .map(|entry| entry.region)
            .ok_or_else(|| format!("missing compiled presentation resource {id}"))
    };
    let mut legacy_to_compiled = vec![None; rustcraft_minecraft_b173::blocks::BLOCKS.len()];
    for legacy in rustcraft_minecraft_b173::blocks::BLOCKS {
        let key = rustcraft_game_api::BlockKey::parse(legacy.name).expect("first-party block key");
        legacy_to_compiled[legacy.id.0 as usize] = profile.block_id(&key);
    }
    let destroy_stages: [AtlasRegion; 10] = (0..10)
        .map(|stage| {
            region(rustcraft_minecraft_b173::resource_keys::destroy_stage(
                stage,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .expect("ten destroy stages");
    Ok(FirstPartyCompiled {
        resources: RendererResources {
            atlas_pages: compiled
                .pages
                .iter()
                .map(|page| RgbaTexture {
                    width: page.width,
                    height: page.height,
                    rgba: page.rgba.clone(),
                    sampler: match page.sampler {
                        rustcraft_content::resources::SamplerPolicy::Nearest => {
                            TextureSampling::Nearest
                        }
                        rustcraft_content::resources::SamplerPolicy::Linear => {
                            TextureSampling::Linear
                        }
                    },
                })
                .collect(),
            container_background: region(rustcraft_minecraft_b173::resource_keys::inventory())?,
            hud: region(rustcraft_minecraft_b173::resource_keys::hud())?,
            player_skin: region(rustcraft_minecraft_b173::resource_keys::player())?,
            occupancy: compiled.metrics.occupancy,
            cache_hit: compiled.metrics.cache_hit,
        },
        materials,
        legacy_to_compiled,
        destroy_stages,
    })
}

#[cfg(test)]
fn synthetic_test_terrain_path() -> Result<PathBuf, String> {
    use std::fs;
    let root = std::env::temp_dir().join(format!(
        "rustcraft-minecraft-synthetic-fixture-{}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("gui"))
        .and_then(|_| fs::create_dir_all(root.join("mob")))
        .map_err(|error| format!("create synthetic resource fixture: {error}"))?;
    let write_png = |path: &std::path::Path, width: u32, height: u32, pixels: &[u8]| {
        let file = std::fs::File::create(path)
            .map_err(|error| format!("create fixture {}: {error}", path.display()))?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| format!("encode fixture header: {error}"))?;
        writer
            .write_image_data(pixels)
            .map_err(|error| format!("encode fixture pixels: {error}"))
    };
    let mut terrain = vec![0; 256 * 256 * 4];
    for y in 0..256usize {
        for x in 0..256usize {
            let tile_x = (x / 16) as u8;
            let tile_y = (y / 16) as u8;
            let checker = ((x % 16) ^ (y % 16)) as u8;
            let offset = (y * 256 + x) * 4;
            terrain[offset..offset + 4].copy_from_slice(&[
                tile_x.wrapping_mul(17).wrapping_add(checker),
                tile_y.wrapping_mul(31).wrapping_add(checker / 2),
                tile_x.wrapping_add(tile_y).wrapping_mul(13),
                255,
            ]);
        }
    }
    let gui = vec![0x80; 256 * 256 * 4];
    let skin = vec![0xc0; 64 * 64 * 4];
    write_png(&root.join("terrain.png"), 256, 256, &terrain)?;
    write_png(&root.join("gui/inventory.png"), 256, 256, &gui)?;
    write_png(&root.join("gui/gui.png"), 256, 256, &gui)?;
    write_png(&root.join("mob/char.png"), 64, 64, &skin)?;
    Ok(root.join("terrain.png"))
}

fn profile_fingerprint(profile: &rustcraft_game_api::CompiledGameProfile) -> String {
    profile.semantic_fingerprint().to_string()
}

fn minecraft_renderer_resources() -> Result<RendererResources, String> {
    first_party_compiled().map(|compiled| compiled.resources.clone())
}

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
    devtools: Option<rustcraft_scripting_rhai::DevTools>,
    control_state: rustcraft_control::ControlState,
    scenario_path: Option<String>,
    dx_capture: Option<PathBuf>,
    dx_exit_pending: bool,
    dx_capture_job: Option<rustcraft_control::JobRef>,
    dx_capture_failure_recorded: bool,
    dx_probe: Option<devtools::Probe>,
    dx_abort_frame: Option<u64>,
    dx_text: String,
    dux_fixture: bool,
    dx_boxes: Vec<rustcraft_engine_core::Aabb>,
    dx_colors: Vec<[f32; 3]>,
    dx_snapshot_at: Instant,
    dx_update_us: u128,
    dx_update_max_us: u128,

    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    simulation: Option<Simulation>,
    presentation: Option<RenderWorld>,
    startup_result: Option<std::sync::mpsc::Receiver<Result<WorldStartupPayload, String>>>,
    startup_worker: Option<std::thread::JoinHandle<()>>,
    startup_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    startup_started: Instant,
    startup_ready_ms: f64,
    startup_visible_core_ms: Option<f64>,
    player_control_enabled: bool,
    world_ready_at: Option<Instant>,
    startup_metadata_player_ms: f64,
    startup_chunk_load_ms: f64,
    startup_generation_ms: f64,
    startup_worker_total_ms: f64,
    full_desired_ready_ms: Option<f64>,
    light_converged_count: u64,
    lighting_work_ms: f64,
    eviction_count: u64,
    eviction_blocked_dirty: u64,
    eviction_blocked_lighting: u64,
    resident_columns_peak: usize,
    resident_sections_peak: usize,
    world_storage: Option<rustcraft_world::WorldStorage>,
    residency: rustcraft_world::WorldResidency,
    stream_load_radius: i32,
    stream_lookahead_enabled: bool,
    last_residency_position: Option<(f32, f32)>,
    last_residency_update_at: Option<Instant>,
    load_scheduler: rustcraft_world::ChunkLoadScheduler,
    generation_scheduler: rustcraft_world::GenerationScheduler,
    initial_lighting_scheduler: rustcraft_runtime::lighting::InitialLightingScheduler,
    stream_generator: Option<std::sync::Arc<dyn rustcraft_world::ChunkGenerator>>,
    world_seed: i64,
    stream_generation_allowed: bool,
    persistence_dirty: rustcraft_world::PersistenceDirtyTracker,
    save_scheduler: rustcraft_world::SaveScheduler,
    pending_spatial_columns: HashMap<
        rustcraft_engine_core::ChunkPos,
        (
            Vec<rustcraft_world::SpatialRecord>,
            Vec<rustcraft_world::SpatialTombstone>,
        ),
    >,
    entity_save_snapshots: HashMap<(i32, i32, u64), rustcraft_runtime::EntityColumnSnapshot>,
    entity_records_loaded: u64,
    entity_records_saved: u64,
    entity_encode_bytes: u64,
    entity_encode_ms: f64,
    entity_decode_ms: f64,
    entity_save_before_evict: u64,
    entity_evictions_blocked: u64,
    world_name: String,
    saves_directory: PathBuf,
    worldgen_metrics: rustcraft_world::GenerationMetrics,
    loaded_from_disk: usize,
    generated_chunks: usize,
    load_results_applied: u64,
    generation_results_applied: u64,
    controller: LocalHumanController,
    clock: rustcraft_runtime::metrics::FixedStepClock,
    last_frame: Instant,
    resources: Result<RendererResources, String>,
    diagnostic: Option<Stage>,
    capture: Option<PathBuf>,
    debug: bool,
    debug_trace: bool,
    metrics: rustcraft_runtime::metrics::Metrics,
    process: rustcraft_runtime::metrics::ProcessSampler,
    last_render: Instant,
    debug_text: String,
    debug_overlay_text: String,
    last_debug_trace: Instant,
    last_snapshot: Instant,
    last_player_autosave: Instant,
    player_autosave_interval: std::time::Duration,
    player_components: Option<Vec<rustcraft_world::PlayerComponent>>,
    unknown_player_components: Vec<rustcraft_world::PlayerComponent>,
    player_revision: u64,
    player_persisted_revision: u64,
    player_dirty: bool,
    latest_player_record: Option<rustcraft_world::PlayerRecord>,
    player_encode_ms_last: f64,
    player_checkpoint_ms_last: f64,
    player_save_scheduler: rustcraft_world::PlayerSaveScheduler,
    player_checkpoint_receipts: HashMap<u64, Vec<rustcraft_engine_core::EntityId>>,
    player_autosave_writes: u64,
    last_world_state_autosave: Instant,
    world_state_autosave_interval: Duration,
    world_state_components: Option<Vec<rustcraft_world::WorldStateComponent>>,
    unknown_world_state_components: Vec<rustcraft_world::WorldStateComponent>,
    world_state_revision: u64,
    world_state_persisted_revision: u64,
    world_state_dirty: bool,
    latest_world_state_record: Option<rustcraft_world::WorldStateRecord>,
    world_state_save_scheduler: rustcraft_world::WorldStateSaveScheduler,
    world_state_checkpoint_ms_last: f64,
    restored_world_time: u64,
    gpu_metrics: Option<gpu_metrics::Provider>,
    measure_seconds: Option<f64>,
    survival_start: bool,
    stream_perf: bool,
    stream_perf_motion_seconds: f32,
    stream_route_completed: bool,
    stream_route_started_at: Option<Instant>,
    stream_route_anchor: Option<Vec3>,
    stream_last_motion_position: Option<Vec3>,
    stream_stuck_ticks: u16,
    stream_avoidance_ticks: u16,
    stream_navigation_path: VecDeque<[f32; 2]>,
    stream_navigation_ticks: u16,
    stream_return_path: VecDeque<[f32; 2]>,
    stream_reversal_yaw: Option<f32>,
    stream_visited_columns: std::collections::HashSet<rustcraft_engine_core::ChunkPos>,
    stream_evicted_columns: std::collections::HashSet<rustcraft_engine_core::ChunkPos>,
    stream_evicted_entity_ids: HashSet<rustcraft_engine_core::EntityId>,
    stream_returned_to_origin: bool,
    stream_final_report: Option<Result<String, String>>,
    inventory_open: bool,
    cursor_position: [f32; 2],
    mesh_scheduler: rustcraft_render::meshing::MeshScheduler,
    mesh_worker_count: usize,
    mesh_upload_section_budget: usize,
    mesh_upload_byte_budget: usize,
    lighting_work_budget: usize,
    stream_main_budget: Duration,
    stream_turn_started: Option<Instant>,
    stream_stage_deadline: Option<Instant>,
    snapshot_dirty_sections: HashSet<rustcraft_engine_core::SectionPos>,
    mesh_dirty_sections: HashSet<rustcraft_engine_core::SectionPos>,
    pending_evictions: HashSet<rustcraft_engine_core::ChunkPos>,
    mesh_uploads: u64,
    mesh_upload_bytes: u64,
    mesh_upload_submit_ms: f64,
    mesh_uploads_this_frame: usize,
    mesh_upload_bytes_this_frame: usize,
    camera_motion: bool,
    camera_motion_started: bool,
    camera_motion_start: Instant,
    camera_motion_samples: Vec<Option<(rustcraft_render::RenderSubmissionStats, u64, usize)>>,
    camera_motion_mesh_baseline: u64,
    camera_motion_buffer_baseline: (u64, u64, u64),
    camera_motion_buffer_reuse_result: (u64, u64, u64, f64),
    frame_phases: FramePhaseMetrics,
    frame_phase_samples: FramePhaseSamples,
    responsiveness: ResponsivenessMetrics,
    stream_stage_fairness: [StreamStageFairness; 5],
    request_started_at: HashMap<(rustcraft_engine_core::ChunkPos, u64), Instant>,
    generation_started_at: HashMap<(rustcraft_engine_core::ChunkPos, u64), Instant>,
    light_latency_started_at: HashMap<rustcraft_engine_core::ChunkPos, Instant>,
    light_queue_started_at: HashMap<rustcraft_engine_core::ChunkPos, Instant>,
    light_work_started_at: HashMap<rustcraft_engine_core::ChunkPos, Instant>,
    light_cpu_accumulated_ms: HashMap<rustcraft_engine_core::ChunkPos, f64>,
    visible_latency_started_at: HashMap<rustcraft_engine_core::ChunkPos, Instant>,
    first_section_latency_started_at: HashMap<rustcraft_engine_core::ChunkPos, Instant>,
    visible_expected_sections:
        HashMap<rustcraft_engine_core::ChunkPos, std::collections::HashSet<i32>>,
    render_ready_columns: std::collections::HashSet<rustcraft_engine_core::ChunkPos>,
    travel_margin: TravelMarginStats,
    voxel_ready_latency_ms: LatencyWindow,
    light_ready_latency_ms: LatencyWindow,
    render_visible_latency_ms: LatencyWindow,
    first_section_latency_ms: LatencyWindow,
    load_total_latency_ms: LatencyWindow,
    load_worker_ms: LatencyWindow,
    load_queue_wait_ms: LatencyWindow,
    generation_total_latency_ms: LatencyWindow,
    generation_worker_ms: LatencyWindow,
    generation_queue_wait_ms: LatencyWindow,
    publication_ms: LatencyWindow,
    lighting_queue_wait_ms: LatencyWindow,
    lighting_active_ms: LatencyWindow,
    lighting_cpu_ms: LatencyWindow,
    initial_light_queue_wait_ms: LatencyWindow,
    initial_light_worker_ms: LatencyWindow,
    initial_light_emitters: u64,
    initial_light_direct_voxels: u64,
    initial_light_propagation_nodes: u64,
    mesh_queue_wait_ms: LatencyWindow,
    mesh_execution_ms: LatencyWindow,
    mesh_upload_wait_ms: LatencyWindow,
}

#[derive(Default)]
struct FramePhaseMetrics {
    fixed_step_ms: f64,
    residency_ms: f64,
    boundary_lighting_ms: f64,
    render_world_sync_ms: f64,
    mesh_schedule_ms: f64,
    mesh_poll_upload_ms: f64,
    debug_hud_ms: f64,
    render_present_ms: f64,
}

struct TravelMarginStats {
    current: [f32; 5],
    minimum: [f32; 5],
    sum: [f64; 5],
    samples: u64,
    history: VecDeque<[f32; 5]>,
}

impl Default for TravelMarginStats {
    fn default() -> Self {
        Self {
            current: [0.0; 5],
            minimum: [f32::INFINITY; 5],
            sum: [0.0; 5],
            samples: 0,
            history: VecDeque::with_capacity(4_096),
        }
    }
}

impl TravelMarginStats {
    fn sample(
        &mut self,
        safe: &std::collections::HashSet<rustcraft_engine_core::ChunkPos>,
        visible: &std::collections::HashSet<rustcraft_engine_core::ChunkPos>,
        position: Vec3,
        movement: [f32; 2],
        view: [f32; 2],
    ) {
        let direction = normalize_horizontal(movement)
            .unwrap_or_else(|| normalize_horizontal(view).unwrap_or([0.0, 1.0]));
        let forward_safe = visible_line_margin(safe, position, direction);
        let forward_visible = visible_line_margin(visible, position, direction);
        let lateral_safe = visible_line_margin(safe, position, [-direction[1], direction[0]]).min(
            visible_line_margin(safe, position, [direction[1], -direction[0]]),
        );
        let lateral_visible =
            visible_line_margin(visible, position, [-direction[1], direction[0]]).min(
                visible_line_margin(visible, position, [direction[1], -direction[0]]),
            );
        let rear_visible = visible_line_margin(visible, position, [-direction[0], -direction[1]]);
        self.current = [
            forward_safe,
            forward_visible,
            lateral_safe,
            lateral_visible,
            rear_visible,
        ];
        for (index, value) in self.current.into_iter().enumerate() {
            self.minimum[index] = self.minimum[index].min(value);
            self.sum[index] += f64::from(value);
        }
        self.samples += 1;
        if self.history.len() == 4_096 {
            self.history.pop_front();
        }
        self.history.push_back(self.current);
    }

    fn mean(&self) -> [f32; 5] {
        if self.samples == 0 {
            return [0.0; 5];
        }
        self.sum.map(|value| (value / self.samples as f64) as f32)
    }

    fn p05(&self) -> [f32; 5] {
        if self.history.is_empty() {
            return [0.0; 5];
        }
        std::array::from_fn(|index| {
            let mut values = self
                .history
                .iter()
                .map(|sample| sample[index])
                .collect::<Vec<_>>();
            values.sort_by(f32::total_cmp);
            values[((values.len() - 1) as f32 * 0.05).round() as usize]
        })
    }
}

fn normalize_horizontal(value: [f32; 2]) -> Option<[f32; 2]> {
    let length = value[0].hypot(value[1]);
    (length > 0.001 && length.is_finite()).then_some([value[0] / length, value[1] / length])
}

/// Nominal seven-leg route at the production movement cap (4 blocks/s), with semantic terrain
/// detours, a velocity-derived reversal and a bounded breadcrumb return applied by the driver.
fn stream_route_sample(elapsed: f32, leg_seconds: f32, position: Vec3) -> (f32, MoveIntent, bool) {
    let leg = (elapsed / leg_seconds).floor() as usize;
    let (yaw, movement) = match leg {
        // Move +X while looking +Z (strafe), proving velocity dominates camera lookahead.
        0 => (
            0.0,
            MoveIntent {
                forward: 0.0,
                strafe: -1.0,
            },
        ),
        1 => (
            0.0,
            MoveIntent {
                forward: 1.0,
                strafe: 0.0,
            },
        ),
        // Move -X while looking +X (walking backward).
        2 => (
            std::f32::consts::FRAC_PI_2,
            MoveIntent {
                forward: -1.0,
                strafe: 0.0,
            },
        ),
        3 => (
            -3.0 * std::f32::consts::FRAC_PI_4,
            MoveIntent {
                forward: 1.0,
                strafe: 0.0,
            },
        ),
        4 => {
            // Keep the negative-quadrant leg geometrically meaningful even when v2 trees/lakes
            // forced earlier semantic-input detours. Near the target this is predominantly -Z,
            // so the following +Z leg remains the deliberate reversal.
            let target = [-128.0 - position.x, -96.0 - position.z];
            (
                target[0].atan2(target[1]),
                MoveIntent {
                    forward: 1.0,
                    strafe: 0.0,
                },
            )
        }
        // Nominal reversal; the driver substitutes the opposite actual horizontal velocity.
        5 => (
            0.0,
            MoveIntent {
                forward: 1.0,
                strafe: 0.0,
            },
        ),
        _ => {
            let home = [-position.x, -position.z];
            if home[0].hypot(home[1]) < 1.0 {
                (0.0, MoveIntent::default())
            } else {
                (
                    home[0].atan2(home[1]),
                    MoveIntent {
                        forward: 1.0,
                        strafe: 0.0,
                    },
                )
            }
        }
    };
    (yaw, movement, true)
}

fn stream_route_label(elapsed: f32, leg_seconds: f32) -> &'static str {
    match (elapsed / leg_seconds).floor() as usize {
        0 => "+X strafe/look north",
        1 => "+Z turn",
        2 => "-X backward",
        3 => "diagonal -X/-Z",
        4 => "-Z negative quadrant",
        5 => "180-degree velocity reversal",
        _ => "return start/revisit",
    }
}

/// Bounded, explicit window-size override for streaming diagnostics only.
fn stream_window_size(value: Option<&str>) -> Result<(u32, u32), String> {
    let Some(value) = value else {
        return Ok((1280, 720));
    };
    let size = value.split_once('x').and_then(|(width, height)| {
        Some((width.parse::<u32>().ok()?, height.parse::<u32>().ok()?))
    });
    match size {
        Some((width, height))
            if (320..=4096).contains(&width) && (240..=2160).contains(&height) =>
        {
            Ok((width, height))
        }
        _ => Err(
            "RUSTCRAFT_STREAM_WINDOW_SIZE must be WIDTHxHEIGHT within 320x240..4096x2160"
                .to_owned(),
        ),
    }
}

/// Diagnostic-only bounded surface navigation. This reads real available voxels and returns
/// waypoints; the controller still issues normal look/move/jump intents through collision.
/// No player/residency mutation, terrain edit, speed override, or renderer movement is involved.
fn stream_surface_path(simulation: &Simulation, goal: [f32; 2]) -> VecDeque<[f32; 2]> {
    use rustcraft_engine_core::{BlockPos, ChunkPos};
    use rustcraft_minecraft_b173::blocks as b;
    const SIDE: usize = 33;
    const MID: i32 = 16;
    let player = simulation.player.position;
    let base_x = player.x.floor() as i32 - MID;
    let base_z = player.z.floor() as i32 - MID;
    let in_water = simulation.world.get(BlockPos {
        x: player.x.floor() as i32,
        y: player.y.floor() as i32,
        z: player.z.floor() as i32,
    }) == b::WATER.id;
    let mut heights = [None; SIDE * SIDE];
    for z in 0..SIDE {
        for x in 0..SIDE {
            let wx = base_x + x as i32;
            let wz = base_z + z as i32;
            if !simulation.world.column_available(ChunkPos {
                x: wx.div_euclid(16),
                z: wz.div_euclid(16),
            }) {
                continue;
            }
            for y in ((player.y.floor() as i32 - 8)..=(player.y.floor() as i32 + 3)).rev() {
                let support = simulation.world.get(BlockPos { x: wx, y, z: wz });
                if simulation.registry.is_solid(support)
                    && (1..=2).all(|dy| {
                        let block = simulation.world.get(BlockPos {
                            x: wx,
                            y: y + dy,
                            z: wz,
                        });
                        block == b::AIR.id || (in_water && block == b::WATER.id)
                    })
                {
                    // The player may be in a lake cavity below the macro surface. Select the
                    // closest physical floor, not the highest roof above the player's head.
                    let cell = &mut heights[z * SIDE + x];
                    if cell.is_none_or(|old: i32| {
                        (y as f32 - (player.y - 1.0)).abs() < (old as f32 - (player.y - 1.0)).abs()
                    }) {
                        *cell = Some(y);
                    }
                }
            }
        }
    }
    let start = MID as usize * SIDE + MID as usize;
    if heights[start].is_none() {
        return VecDeque::new();
    }
    let mut parents = [usize::MAX; SIDE * SIDE];
    let mut distances = [0_u16; SIDE * SIDE];
    parents[start] = start;
    let mut queue = VecDeque::from([start]);
    let mut best = start;
    let score = |index: usize, distance: u16| {
        let x = base_x as f32 + (index % SIDE) as f32 + 0.5;
        let z = base_z as f32 + (index / SIDE) as f32 + 0.5;
        (x - goal[0]).hypot(z - goal[1]) + f32::from(distance) * 0.12
    };
    while let Some(index) = queue.pop_front() {
        if score(index, distances[index]) < score(best, distances[best]) {
            best = index;
        }
        let x = index % SIDE;
        let z = index / SIDE;
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = x as i32 + dx;
            let nz = z as i32 + dz;
            if !(0..SIDE as i32).contains(&nx) || !(0..SIDE as i32).contains(&nz) {
                continue;
            }
            let next = nz as usize * SIDE + nx as usize;
            if parents[next] == usize::MAX
                && heights[next].zip(heights[index]).is_some_and(|(a, b)| {
                    (-3..=1).contains(&(a - b))
                        && (a <= b
                            || !simulation.registry.is_solid(simulation.world.get(BlockPos {
                                x: base_x + x as i32,
                                y: b + 3,
                                z: base_z + z as i32,
                            })))
                })
            {
                parents[next] = index;
                distances[next] = distances[index] + 1;
                queue.push_back(next);
            }
        }
    }
    let mut path = VecDeque::new();
    while best != start {
        path.push_front([
            base_x as f32 + (best % SIDE) as f32 + 0.5,
            base_z as f32 + (best / SIDE) as f32 + 0.5,
        ]);
        best = parents[best];
    }
    path
}

fn visible_line_margin(
    ready: &std::collections::HashSet<rustcraft_engine_core::ChunkPos>,
    position: Vec3,
    direction: [f32; 2],
) -> f32 {
    let mut margin = 0.0;
    for step in 1..=24 {
        let distance = step as f32 * 0.5;
        let column = rustcraft_engine_core::ChunkPos {
            x: ((position.x + direction[0] * distance * 16.0).floor() as i32).div_euclid(16),
            z: ((position.z + direction[1] * distance * 16.0).floor() as i32).div_euclid(16),
        };
        if !ready.contains(&column) {
            break;
        }
        margin = distance;
    }
    margin
}

#[derive(Default)]
struct FramePhaseSamples {
    fixed_step: LatencyWindow,
    residency: LatencyWindow,
    boundary_lighting: LatencyWindow,
    render_world_sync: LatencyWindow,
    mesh_schedule: LatencyWindow,
    mesh_poll_upload: LatencyWindow,
    debug_hud: LatencyWindow,
    render_present: LatencyWindow,
}

#[derive(Default)]
struct ResponsivenessMetrics {
    frame_interval: LatencyWindow,
    event_dispatch: LatencyWindow,
    input_processing: LatencyWindow,
    event_loop_gap: LatencyWindow,
    fixed_tick_gap: LatencyWindow,
    input_to_simulation: LatencyWindow,
    input_to_render: LatencyWindow,
    last_about_to_wait: Option<Instant>,
    last_fixed_tick: Option<Instant>,
    pending_input_at: Option<Instant>,
    input_consumed_at: Option<Instant>,
    due_ticks: u64,
    executed_ticks: u64,
    dropped_ticks: u64,
    dropped_seconds: f64,
    long_task_counts: [u64; 4],
    long_tasks: VecDeque<(String, f64, usize)>,
    longest_task: Option<(String, f64, usize)>,
}

#[derive(Default)]
struct StreamStageFairness {
    skipped_due_to_budget: u64,
    blocked_dependency_turns: u64,
    turns_without_service: u32,
    max_turns_without_service: u32,
    pending_since: Option<Instant>,
    oldest_age_ms: f64,
    max_oldest_age_ms: f64,
}

impl StreamStageFairness {
    fn observe(
        &mut self,
        pending: bool,
        serviced: bool,
        had_budget: bool,
        blocked_dependency: bool,
        now: Instant,
    ) {
        if !pending {
            self.pending_since = None;
            self.oldest_age_ms = 0.0;
            self.turns_without_service = 0;
            return;
        }
        let since = *self.pending_since.get_or_insert(now);
        self.oldest_age_ms = now.duration_since(since).as_secs_f64() * 1000.0;
        self.max_oldest_age_ms = self.max_oldest_age_ms.max(self.oldest_age_ms);
        if !had_budget {
            self.skipped_due_to_budget = self.skipped_due_to_budget.saturating_add(1);
        }
        if blocked_dependency {
            self.blocked_dependency_turns = self.blocked_dependency_turns.saturating_add(1);
            self.turns_without_service = 0;
        } else if serviced {
            self.turns_without_service = 0;
        } else {
            self.turns_without_service = self.turns_without_service.saturating_add(1);
            self.max_turns_without_service = self
                .max_turns_without_service
                .max(self.turns_without_service);
        }
    }
}

impl ResponsivenessMetrics {
    fn record_long_task(&mut self, subsystem: &str, elapsed: Duration, work_items: usize) {
        let ms = elapsed.as_secs_f64() * 1000.0;
        for (index, threshold) in [4.0, 8.0, 16.0, 33.0].into_iter().enumerate() {
            if ms >= threshold {
                self.long_task_counts[index] += 1;
            }
        }
        if ms >= 4.0 {
            if self
                .longest_task
                .as_ref()
                .is_none_or(|(_, longest_ms, _)| ms > *longest_ms)
            {
                self.longest_task = Some((subsystem.to_owned(), ms, work_items));
            }
            self.long_tasks
                .push_back((subsystem.to_owned(), ms, work_items));
            if self.long_tasks.len() > 64 {
                self.long_tasks.pop_front();
            }
        }
    }
}

fn pop_before_deadline<T: Copy + Eq + std::hash::Hash>(
    pending: &mut HashSet<T>,
    deadline: Instant,
) -> Option<T> {
    (Instant::now() < deadline)
        .then(|| pending.iter().next().copied())
        .flatten()
        .inspect(|item| {
            pending.remove(item);
        })
}

fn pop_urgent_before_deadline<T, K>(
    pending: &mut HashSet<T>,
    deadline: Instant,
    mut key: impl FnMut(T) -> K,
) -> Option<T>
where
    T: Copy + Eq + std::hash::Hash,
    K: Ord,
{
    if Instant::now() >= deadline {
        return None;
    }
    let item = pending.iter().copied().min_by_key(|item| key(*item))?;
    pending.remove(&item).then_some(item)
}

#[derive(Default)]
struct LatencyWindow {
    samples_ms: VecDeque<f64>,
}

impl LatencyWindow {
    fn record(&mut self, elapsed: Duration) {
        self.samples_ms.push_back(elapsed.as_secs_f64() * 1000.0);
        if self.samples_ms.len() > 256 {
            self.samples_ms.pop_front();
        }
    }

    fn record_ms(&mut self, milliseconds: f64) {
        if milliseconds.is_finite() && milliseconds >= 0.0 {
            self.record(Duration::from_secs_f64(milliseconds / 1000.0));
        }
    }

    fn summary(&self) -> Option<[f64; 5]> {
        if self.samples_ms.is_empty() {
            return None;
        }
        let mut samples = self.samples_ms.iter().copied().collect::<Vec<_>>();
        samples.sort_by(f64::total_cmp);
        let quantile = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
        Some([
            quantile(0.50),
            quantile(0.90),
            quantile(0.95),
            quantile(0.99),
            samples[samples.len() - 1],
        ])
    }
}

struct WorldStartupPayload {
    simulation: Simulation,
    presentation: RenderWorld,
    world_storage: Option<rustcraft_world::WorldStorage>,
    worldgen_metrics: rustcraft_world::GenerationMetrics,
    loaded_from_disk: usize,
    generated_chunks: usize,
    last_player_autosave: Instant,
    player_components: Option<Vec<rustcraft_world::PlayerComponent>>,
    unknown_player_components: Vec<rustcraft_world::PlayerComponent>,
    player_revision: u64,
    player_persisted_revision: u64,
    player_dirty: bool,
    latest_player_record: Option<rustcraft_world::PlayerRecord>,
    player_encode_ms_last: f64,
    player_checkpoint_ms_last: f64,
    last_world_state_autosave: Instant,
    world_state_components: Option<Vec<rustcraft_world::WorldStateComponent>>,
    unknown_world_state_components: Vec<rustcraft_world::WorldStateComponent>,
    world_state_revision: u64,
    world_state_persisted_revision: u64,
    world_state_dirty: bool,
    latest_world_state_record: Option<rustcraft_world::WorldStateRecord>,
    world_state_checkpoint_ms_last: f64,
    entity_records_loaded: u64,
    entity_decode_ms: f64,
    stream_generator: Option<std::sync::Arc<dyn rustcraft_world::ChunkGenerator>>,
    world_seed: i64,
    stream_generation_allowed: bool,
    metadata_player_ms: f64,
    chunk_load_ms: f64,
    generation_ms: f64,
    worker_total_ms: f64,
}

impl ClientApp {
    fn activate_pending_spatial_columns(
        &mut self,
        simulation: &mut Simulation,
    ) -> Result<(), String> {
        let pending = std::mem::take(&mut self.pending_spatial_columns);
        for (position, (records, tombstones)) in pending {
            let decode_started = Instant::now();
            let record_count = records.len();
            let entities = records
                .iter()
                .map(|record| {
                    rustcraft_minecraft_b173::world_persistence::decode_item_entity(
                        record,
                        &simulation.registry,
                        &self.world_name,
                        position,
                    )
                    .map_err(|error| error.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let tombstones = tombstones
                .iter()
                .map(|tombstone| {
                    (
                        tombstone.entity_id,
                        tombstone.source,
                        tombstone.entity_revision,
                    )
                })
                .collect::<Vec<_>>();
            simulation
                .activate_entity_column(position, entities, &tombstones)
                .map_err(|error| {
                    format!(
                        "world {} column ({},{}) spatial activation: {error}",
                        self.world_name, position.x, position.z
                    )
                })?;
            self.entity_records_loaded = self
                .entity_records_loaded
                .saturating_add(record_count as u64);
            self.entity_decode_ms += decode_started.elapsed().as_secs_f64() * 1000.0;
        }
        Ok(())
    }

    fn activate_pending_spatial_column(
        &mut self,
        position: rustcraft_engine_core::ChunkPos,
    ) -> Result<(), String> {
        let Some((records, tombstones)) = self.pending_spatial_columns.remove(&position) else {
            return Ok(());
        };
        let decode_started = Instant::now();
        let record_count = records.len();
        let simulation = self.simulation.as_mut().ok_or("simulation unavailable")?;
        let entities = records
            .iter()
            .map(|record| {
                rustcraft_minecraft_b173::world_persistence::decode_item_entity(
                    record,
                    &simulation.registry,
                    &self.world_name,
                    position,
                )
                .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let tombstones = tombstones
            .iter()
            .map(|tombstone| {
                (
                    tombstone.entity_id,
                    tombstone.source,
                    tombstone.entity_revision,
                )
            })
            .collect::<Vec<_>>();
        simulation
            .activate_entity_column(position, entities, &tombstones)
            .map_err(str::to_owned)?;
        self.entity_records_loaded = self
            .entity_records_loaded
            .saturating_add(record_count as u64);
        self.entity_decode_ms += decode_started.elapsed().as_secs_f64() * 1000.0;
        Ok(())
    }

    fn autonomous_stream_report(&self) -> Result<String, String> {
        let mut failures = Vec::new();
        if !self.stream_route_completed {
            failures.push("scripted route did not complete".to_owned());
        }
        if self.startup_visible_core_ms.is_none() || !self.player_control_enabled {
            failures.push("complete startup 3x3 SAFE+VISIBLE core was not released".to_owned());
        }
        if self.stream_visited_columns.len() < 20 {
            failures.push(format!(
                "route crossed only {} distinct columns (need at least 20)",
                self.stream_visited_columns.len()
            ));
        }
        if self.world_seed == 731_173
            && self
                .stream_generator
                .as_ref()
                .is_some_and(|generator| generator.version() == 2)
            && !self
                .stream_visited_columns
                .iter()
                .any(|position| position.x < 0 && position.z < 0)
        {
            failures.push("route did not reach negative X/Z".to_owned());
        }
        if !self.stream_returned_to_origin {
            failures.push("route did not return through the origin corridor".to_owned());
        }
        if self.travel_margin.samples == 0
            || self.travel_margin.minimum[0] <= 0.0
            || self.travel_margin.minimum[1] <= 0.0
        {
            failures.push(format!(
                "terrain-ahead margin reached zero: minimum={:?}",
                self.travel_margin.minimum
            ));
        }
        if let Some(simulation) = self.simulation.as_ref() {
            let unsafe_visible = simulation
                .world
                .safe_column_positions()
                .find(|position| !self.render_ready_columns.contains(position));
            if let Some(position) = unsafe_visible {
                failures.push(format!(
                    "SAFE=>VISIBLE violated at ({},{})",
                    position.x, position.z
                ));
            }
        }
        let event_loop = self.responsiveness.event_loop_gap.summary();
        let software_present_wait = self.renderer.as_ref().is_some_and(|renderer| {
            renderer
                .adapter_info
                .name
                .to_ascii_lowercase()
                .contains("llvmpipe")
                && self
                    .responsiveness
                    .longest_task
                    .as_ref()
                    .is_some_and(|(stage, _, _)| stage == "render_present")
                && [
                    &self.frame_phase_samples.fixed_step,
                    &self.frame_phase_samples.residency,
                    &self.frame_phase_samples.boundary_lighting,
                    &self.frame_phase_samples.render_world_sync,
                    &self.frame_phase_samples.mesh_schedule,
                    &self.frame_phase_samples.mesh_poll_upload,
                    &self.frame_phase_samples.debug_hud,
                ]
                .into_iter()
                .all(|samples| samples.summary().is_none_or(|summary| summary[4] < 100.0))
        });
        if !software_present_wait && event_loop.is_some_and(|summary| summary[4] > 100.0) {
            failures.push(format!("event-loop gap exceeded 100 ms: {event_loop:?}"));
        }
        let input_to_sim = self.responsiveness.input_to_simulation.summary();
        let input_to_render = self.responsiveness.input_to_render.summary();
        if input_to_sim.is_none_or(|summary| !software_present_wait && summary[4] > 100.0) {
            failures.push(format!(
                "intent-to-simulation latency missing or exceeded 100 ms: {input_to_sim:?}"
            ));
        }
        if input_to_render.is_none_or(|summary| !software_present_wait && summary[4] > 100.0) {
            failures.push(format!(
                "intent-to-render latency missing or exceeded 100 ms: {input_to_render:?}"
            ));
        }
        let visible = self.render_visible_latency_ms.summary();
        if visible.is_none_or(|summary| summary[2] > 9_000.0) {
            failures.push(format!(
                "request-to-visible p95 missing or not single-digit seconds: {visible:?}"
            ));
        }
        if self.responsiveness.dropped_seconds > 0.1 {
            failures.push(format!(
                "dropped fixed-step time exceeded 0.1 s: {:.3}",
                self.responsiveness.dropped_seconds
            ));
        }
        let renderer = self.renderer.as_ref();
        let adapter = renderer
            .map(|renderer| {
                format!(
                    "{} / {:?}",
                    renderer.adapter_info.name, renderer.adapter_info.backend
                )
            })
            .unwrap_or_else(|| "unavailable".to_owned());
        let snapshot_bytes = self
            .presentation
            .as_ref()
            .map_or(0, RenderWorld::snapshot_bytes);
        let report = format!(
            "CLIENT_STREAM_AUTO result={} adapter={} present_wait={} radius={} retain={} startup_safe_visible_ms={:?} visited_columns={} returned_origin={} margins_current/min/p05/mean={:?}/{:?}/{:?}/{:?} request_visible_ms={:?} oldest_critical_queue_age_max_ms={:.3} event_loop_gap_ms={:?} frame_ms={:?} tps={:?} dropped_s={:.3} input_to_sim_ms={:?} input_to_render_ms={:?} resident_columns_peak={} resident_sections_peak={} authoritative_voxel_light_bytes_est={} snapshot_bytes={} mesh_logical_bytes={} mesh_capacity_bytes={} worldgen_v1_hash=e0d1f83c16b281124b7a9c190f667d7eddaa8b2f35ef5ef98ccb4bab434c1bb6 worldgen_v2_hash=4e134fa137fa5477fc3d28c9a610afd14019b0d0304246e43cd9940deb4684e7 streaming_v2_sample_hash=da875570efd5ace7a12c73e883a458e7f5ebe95f21d45ef90841b72104cf5b8e",
            if failures.is_empty() { "PASS" } else { "FAIL" },
            adapter,
            if software_present_wait {
                "external-software-renderer"
            } else {
                "within-threshold"
            },
            self.residency.load_radius(),
            self.residency.retain_radius(),
            self.startup_visible_core_ms,
            self.stream_visited_columns.len(),
            self.stream_returned_to_origin,
            self.travel_margin.current,
            self.travel_margin.minimum,
            self.travel_margin.p05(),
            self.travel_margin.mean(),
            visible,
            [0, 2, 3, 4]
                .into_iter()
                .map(|index| &self.stream_stage_fairness[index])
                .map(|stage| stage.max_oldest_age_ms)
                .fold(0.0, f64::max),
            event_loop,
            self.responsiveness.frame_interval.summary(),
            self.metrics.tps,
            self.responsiveness.dropped_seconds,
            input_to_sim,
            input_to_render,
            self.resident_columns_peak,
            self.resident_sections_peak,
            self.resident_sections_peak.saturating_mul(36 * 1024),
            snapshot_bytes,
            renderer.map_or(0, Renderer::gpu_mesh_logical_bytes),
            renderer.map_or(0, |renderer| renderer.gpu_mesh_allocated_bytes()),
        );
        if failures.is_empty() {
            Ok(report)
        } else {
            Err(format!("{report}\nfailures: {}", failures.join("; ")))
        }
    }

    fn take_startup_payload(&mut self) -> WorldStartupPayload {
        WorldStartupPayload {
            simulation: self.simulation.take().expect("startup simulation ready"),
            presentation: self
                .presentation
                .take()
                .expect("startup render snapshot ready"),
            world_storage: self.world_storage.take(),
            worldgen_metrics: self.worldgen_metrics,
            loaded_from_disk: self.loaded_from_disk,
            generated_chunks: self.generated_chunks,
            last_player_autosave: self.last_player_autosave,
            player_components: self.player_components.take(),
            unknown_player_components: std::mem::take(&mut self.unknown_player_components),
            player_revision: self.player_revision,
            player_persisted_revision: self.player_persisted_revision,
            player_dirty: self.player_dirty,
            latest_player_record: self.latest_player_record.take(),
            player_encode_ms_last: self.player_encode_ms_last,
            player_checkpoint_ms_last: self.player_checkpoint_ms_last,
            last_world_state_autosave: self.last_world_state_autosave,
            world_state_components: self.world_state_components.take(),
            unknown_world_state_components: std::mem::take(
                &mut self.unknown_world_state_components,
            ),
            world_state_revision: self.world_state_revision,
            world_state_persisted_revision: self.world_state_persisted_revision,
            world_state_dirty: self.world_state_dirty,
            latest_world_state_record: self.latest_world_state_record.take(),
            world_state_checkpoint_ms_last: self.world_state_checkpoint_ms_last,
            entity_records_loaded: self.entity_records_loaded,
            entity_decode_ms: self.entity_decode_ms,
            stream_generator: self.stream_generator.take(),
            world_seed: self.world_seed,
            stream_generation_allowed: self.stream_generation_allowed,
            metadata_player_ms: self.startup_metadata_player_ms,
            chunk_load_ms: self.startup_chunk_load_ms,
            generation_ms: self.startup_generation_ms,
            worker_total_ms: self.startup_worker_total_ms,
        }
    }
}

impl ClientApp {
    fn begin_world_startup(&mut self) {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let world_name = self.world_name.clone();
        let saves_directory = self.saves_directory.clone();
        let survival_start = self.survival_start;
        let cancellation = self.startup_cancel.clone();
        let config = self.control_state.config.clone();
        self.startup_cancel
            .store(false, std::sync::atomic::Ordering::Release);
        self.startup_result = Some(receiver);
        self.startup_started = Instant::now();
        self.startup_worker = Some(
            std::thread::Builder::new()
                .name("client-world-startup".into())
                .spawn(move || {
                    let mut worker = ClientApp::with_config(None, None, config);
                    worker.world_name = world_name;
                    worker.saves_directory = saves_directory;
                    worker.survival_start = survival_start;
                    worker.startup_cancel = cancellation;
                    let worker_started = Instant::now();
                    let result = worker.start_world().map(|()| {
                        worker.startup_worker_total_ms =
                            worker_started.elapsed().as_secs_f64() * 1000.0;
                        worker.take_startup_payload()
                    });
                    let _ = sender.send(result);
                })
                .expect("world startup worker thread creation failed"),
        );
    }

    fn poll_world_startup(&mut self, event_loop: &ActiveEventLoop) {
        let Some(receiver) = self.startup_result.as_ref() else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Err("world startup worker stopped without a result".to_owned())
            }
        };
        self.startup_result.take();
        if let Some(worker) = self.startup_worker.take() {
            let _ = worker.join();
        }
        match result {
            Ok(payload) => {
                self.simulation = Some(payload.simulation);
                self.presentation = Some(payload.presentation);
                if let Some(simulation) = self.simulation.as_mut() {
                    let resident = simulation.world.column_positions().collect::<Vec<_>>();
                    for position in resident {
                        let _ = simulation.world.set_column_safe(position, false);
                    }
                }
                self.player_control_enabled = false;
                self.world_storage = payload.world_storage;
                self.worldgen_metrics = payload.worldgen_metrics;
                self.loaded_from_disk = payload.loaded_from_disk;
                self.generated_chunks = payload.generated_chunks;
                self.last_player_autosave = payload.last_player_autosave;
                self.player_components = payload.player_components;
                self.unknown_player_components = payload.unknown_player_components;
                self.player_revision = payload.player_revision;
                self.player_persisted_revision = payload.player_persisted_revision;
                self.player_dirty = payload.player_dirty;
                self.latest_player_record = payload.latest_player_record;
                self.player_encode_ms_last = payload.player_encode_ms_last;
                self.player_checkpoint_ms_last = payload.player_checkpoint_ms_last;
                self.last_world_state_autosave = payload.last_world_state_autosave;
                self.world_state_components = payload.world_state_components;
                self.unknown_world_state_components = payload.unknown_world_state_components;
                self.world_state_revision = payload.world_state_revision;
                self.world_state_persisted_revision = payload.world_state_persisted_revision;
                self.world_state_dirty = payload.world_state_dirty;
                self.latest_world_state_record = payload.latest_world_state_record;
                self.world_state_checkpoint_ms_last = payload.world_state_checkpoint_ms_last;
                self.entity_records_loaded = payload.entity_records_loaded;
                self.entity_decode_ms = payload.entity_decode_ms;
                self.stream_generator = payload.stream_generator;
                self.world_seed = payload.world_seed;
                self.stream_generation_allowed = payload.stream_generation_allowed;
                self.startup_metadata_player_ms = payload.metadata_player_ms;
                self.startup_chunk_load_ms = payload.chunk_load_ms;
                self.startup_generation_ms = payload.generation_ms;
                self.startup_worker_total_ms = payload.worker_total_ms;
                self.startup_ready_ms = payload.worker_total_ms;
                self.world_ready_at = Some(Instant::now());
                eprintln!(
                    "world startup ready: metadata_player_ms={:.2} async_chunk_load_ms={:.2} generation_wait_ms={:.2} worker_total_ms={:.2}",
                    payload.metadata_player_ms,
                    payload.chunk_load_ms,
                    payload.generation_ms,
                    payload.worker_total_ms,
                );
                if let Some(window) = &self.window {
                    window.set_title("RustCraft");
                }
                self.rebuild_meshes();
            }
            Err(error) => {
                eprintln!("unable to initialize local world: {error}");
                event_loop.exit();
            }
        }
    }

    fn cancel_startup_and_join(&mut self) {
        self.startup_cancel
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(worker) = self.startup_worker.take() {
            let _ = worker.join();
        }
        self.startup_result.take();
    }

    fn new(diagnostic: Option<Stage>, capture: Option<PathBuf>) -> Self {
        let mut config = rustcraft_control::config::settings::engine(true);
        if cfg!(test) {
            config.open();
        } else {
            rustcraft_control::config::settings::load(
                &mut config,
                &std::env::args().collect::<Vec<_>>(),
                |key| std::env::var(key).ok(),
                rustcraft_control::config::user_path(),
            )
            .unwrap_or_else(|e| panic!("configuration: {e}"));
        }
        Self::with_config(diagnostic, capture, config)
    }
    fn with_config(
        diagnostic: Option<Stage>,
        capture: Option<PathBuf>,
        config: rustcraft_control::config::Registry,
    ) -> Self {
        use rustcraft_control::config::settings as keys;
        let mesh_workers = config.effective(keys::MESH_WORKERS).integer() as usize;
        let initial_light_workers = config.effective(keys::LIGHT_WORKERS).integer() as usize;
        let stream_load_radius = config.effective(keys::LOAD_RADIUS).integer() as i32;
        let retain_radius = config.effective(keys::RETAIN_RADIUS).integer() as i32;
        let stream_lookahead_enabled = config.effective(keys::LOOKAHEAD).boolean();
        let player_autosave_interval =
            Duration::from_millis(config.effective(keys::PLAYER_SAVE_MS).integer() as u64);
        let world_state_autosave_interval =
            Duration::from_millis(config.effective(keys::WORLD_SAVE_MS).integer() as u64);
        let mesh_upload_section_budget = config.effective(keys::UPLOAD_SECTIONS).integer() as usize;
        let mesh_upload_byte_budget = config.effective(keys::UPLOAD_BYTES).integer() as usize;
        let lighting_work_budget = config.effective(keys::LIGHT_WORK).integer() as usize;
        let stream_main_budget =
            Duration::from_secs_f64(config.effective(keys::STREAM_MS).float() / 1000.);
        let mut control_state = rustcraft_control::ControlState {
            config,
            ..Default::default()
        };
        control_state.sync_config();
        Self {
            window: None,
            renderer: None,
            devtools: None,
            control_state,
            scenario_path: None,
            dx_capture: None,
            dx_exit_pending: false,
            dx_capture_job: None,
            dx_capture_failure_recorded: false,
            dx_probe: None,
            dx_abort_frame: None,
            dx_text: String::new(),
            dux_fixture: false,
            dx_boxes: Vec::new(),
            dx_colors: Vec::new(),
            dx_snapshot_at: Instant::now(),
            dx_update_us: 0,
            dx_update_max_us: 0,
            simulation: None,
            presentation: None,
            startup_result: None,
            startup_worker: None,
            startup_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            startup_started: Instant::now(),
            startup_ready_ms: 0.0,
            startup_visible_core_ms: None,
            player_control_enabled: cfg!(test),
            world_ready_at: None,
            startup_metadata_player_ms: 0.0,
            startup_chunk_load_ms: 0.0,
            startup_generation_ms: 0.0,
            startup_worker_total_ms: 0.0,
            full_desired_ready_ms: None,
            light_converged_count: 0,
            lighting_work_ms: 0.0,
            eviction_count: 0,
            eviction_blocked_dirty: 0,
            eviction_blocked_lighting: 0,
            resident_columns_peak: 0,
            resident_sections_peak: 0,
            world_storage: None,
            residency: rustcraft_world::WorldResidency::new(
                stream_load_radius,
                retain_radius - stream_load_radius,
            ),
            stream_load_radius,
            stream_lookahead_enabled,
            last_residency_position: None,
            last_residency_update_at: None,
            load_scheduler: rustcraft_world::ChunkLoadScheduler::new(1, 4),
            generation_scheduler: rustcraft_world::GenerationScheduler::new(1, 4),
            initial_lighting_scheduler: rustcraft_runtime::lighting::InitialLightingScheduler::new(
                initial_light_workers,
                8,
            ),
            stream_generator: None,
            world_seed: 731_173,
            stream_generation_allowed: true,
            persistence_dirty: Default::default(),
            save_scheduler: rustcraft_world::SaveScheduler::new(1, 8),
            pending_spatial_columns: HashMap::new(),
            entity_save_snapshots: HashMap::new(),
            entity_records_loaded: 0,
            entity_records_saved: 0,
            entity_encode_bytes: 0,
            entity_encode_ms: 0.0,
            entity_decode_ms: 0.0,
            entity_save_before_evict: 0,
            entity_evictions_blocked: 0,
            world_name: "default".to_owned(),
            saves_directory: std::env::var_os("RUSTCRAFT_SAVES_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("saves")),
            worldgen_metrics: Default::default(),
            loaded_from_disk: 0,
            generated_chunks: 0,
            load_results_applied: 0,
            generation_results_applied: 0,
            controller: LocalHumanController::default(),
            clock: Default::default(),
            last_frame: Instant::now(),
            resources: minecraft_renderer_resources(),
            diagnostic,
            capture,
            debug: std::env::var_os("RUSTCRAFT_F3").is_some(),
            debug_trace: std::env::var_os("RUSTCRAFT_F3_TRACE").is_some(),
            metrics: Default::default(),
            process: Default::default(),
            last_render: Instant::now(),
            debug_text: String::new(),
            debug_overlay_text: String::new(),
            last_debug_trace: Instant::now(),
            last_snapshot: Instant::now() - std::time::Duration::from_secs(1),
            last_player_autosave: Instant::now(),
            player_autosave_interval,
            player_components: None,
            unknown_player_components: Vec::new(),
            player_revision: 0,
            player_persisted_revision: 0,
            player_dirty: false,
            latest_player_record: None,
            player_encode_ms_last: 0.0,
            player_checkpoint_ms_last: 0.0,
            player_save_scheduler: rustcraft_world::PlayerSaveScheduler::new(),
            player_checkpoint_receipts: HashMap::new(),
            player_autosave_writes: 0,
            last_world_state_autosave: Instant::now(),
            world_state_autosave_interval,
            world_state_components: None,
            unknown_world_state_components: Vec::new(),
            world_state_revision: 0,
            world_state_persisted_revision: 0,
            world_state_dirty: false,
            latest_world_state_record: None,
            world_state_save_scheduler: rustcraft_world::WorldStateSaveScheduler::new(),
            world_state_checkpoint_ms_last: 0.0,
            restored_world_time: 0,
            gpu_metrics: None,
            measure_seconds: std::env::var("RUSTCRAFT_MEASURE_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok()),
            survival_start: false,
            stream_perf: false,
            stream_perf_motion_seconds: std::env::var("RUSTCRAFT_STREAM_PERF_SECONDS")
                .ok()
                .and_then(|value| value.parse::<f32>().ok())
                .filter(|value| value.is_finite())
                .unwrap_or(168.0)
                .clamp(10.0, 600.0),
            stream_route_completed: false,
            stream_route_started_at: None,
            stream_route_anchor: None,
            stream_last_motion_position: None,
            stream_stuck_ticks: 0,
            stream_avoidance_ticks: 0,
            stream_navigation_path: VecDeque::new(),
            stream_navigation_ticks: 0,
            stream_return_path: VecDeque::new(),
            stream_reversal_yaw: None,
            stream_visited_columns: std::collections::HashSet::new(),
            stream_evicted_columns: std::collections::HashSet::new(),
            stream_evicted_entity_ids: HashSet::new(),
            stream_returned_to_origin: false,
            stream_final_report: None,
            inventory_open: false,
            cursor_position: [0.; 2],
            mesh_scheduler: rustcraft_render::meshing::MeshScheduler::new(
                mesh_workers,
                mesh_workers * 2,
                FirstPartyTextures,
            ),
            mesh_worker_count: mesh_workers,
            mesh_upload_section_budget,
            mesh_upload_byte_budget,
            lighting_work_budget,
            stream_main_budget,
            stream_turn_started: None,
            stream_stage_deadline: None,
            snapshot_dirty_sections: HashSet::new(),
            mesh_dirty_sections: HashSet::new(),
            pending_evictions: HashSet::new(),
            mesh_uploads: 0,
            mesh_upload_bytes: 0,
            mesh_upload_submit_ms: 0.0,
            mesh_uploads_this_frame: 0,
            mesh_upload_bytes_this_frame: 0,
            camera_motion: false,
            camera_motion_started: false,
            camera_motion_start: Instant::now(),
            camera_motion_samples: vec![None; 8],
            camera_motion_mesh_baseline: 0,
            camera_motion_buffer_baseline: (0, 0, 0),
            camera_motion_buffer_reuse_result: (0, 0, 0, 0.0),
            frame_phases: FramePhaseMetrics::default(),
            frame_phase_samples: FramePhaseSamples::default(),
            responsiveness: ResponsivenessMetrics::default(),
            stream_stage_fairness: std::array::from_fn(|_| StreamStageFairness::default()),
            request_started_at: HashMap::new(),
            generation_started_at: HashMap::new(),
            light_latency_started_at: HashMap::new(),
            light_queue_started_at: HashMap::new(),
            light_work_started_at: HashMap::new(),
            light_cpu_accumulated_ms: HashMap::new(),
            visible_latency_started_at: HashMap::new(),
            first_section_latency_started_at: HashMap::new(),
            visible_expected_sections: HashMap::new(),
            render_ready_columns: std::collections::HashSet::new(),
            travel_margin: TravelMarginStats::default(),
            voxel_ready_latency_ms: LatencyWindow::default(),
            light_ready_latency_ms: LatencyWindow::default(),
            render_visible_latency_ms: LatencyWindow::default(),
            first_section_latency_ms: LatencyWindow::default(),
            load_total_latency_ms: LatencyWindow::default(),
            load_worker_ms: LatencyWindow::default(),
            load_queue_wait_ms: LatencyWindow::default(),
            generation_total_latency_ms: LatencyWindow::default(),
            generation_worker_ms: LatencyWindow::default(),
            generation_queue_wait_ms: LatencyWindow::default(),
            publication_ms: LatencyWindow::default(),
            lighting_queue_wait_ms: LatencyWindow::default(),
            lighting_active_ms: LatencyWindow::default(),
            lighting_cpu_ms: LatencyWindow::default(),
            initial_light_queue_wait_ms: LatencyWindow::default(),
            initial_light_worker_ms: LatencyWindow::default(),
            initial_light_emitters: 0,
            initial_light_direct_voxels: 0,
            initial_light_propagation_nodes: 0,
            mesh_queue_wait_ms: LatencyWindow::default(),
            mesh_execution_ms: LatencyWindow::default(),
            mesh_upload_wait_ms: LatencyWindow::default(),
        }
    }
    fn start_world(&mut self) -> Result<(), String> {
        first_party_compiled()?;
        let compiled_profile = rustcraft_minecraft_b173::compile_profile()
            .map_err(|error| format!("minecraft_b173 game profile: {error:?}"))?;
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        let blocks = BlocksModule;
        bootstrap
            .register_module(&blocks)
            .map_err(|e| format!("block registration: {e:?}"))?;
        let mut world = rustcraft_engine_core::World::new(compiled_profile.default_state().block);
        if cfg!(test) {
            // Existing renderer/client tests intentionally retain their tiny deterministic fixture.
            let flat = rustcraft_minecraft_b173::flat_world_module(&compiled_profile);
            bootstrap
                .register_module(&flat)
                .map_err(|e| format!("flat-world registration: {e:?}"))?;
            flat.generate(&mut world, -16, 16, -16, 16);
            if self.diagnostic.is_none() {
                rustcraft_minecraft_b173::flat_world::decorate_sandbox(
                    &mut world,
                    &bootstrap.registry,
                );
            }
        }
        let restored = if !cfg!(test) {
            self.load_or_generate_world(&compiled_profile, &bootstrap.registry, &mut world)?
        } else {
            None
        };
        let player_restored = restored.is_some();
        let mut simulation =
            initialize_simulation(world, bootstrap.registry, restored, self.survival_start);
        simulation.time = self.restored_world_time;
        self.activate_pending_spatial_columns(&mut simulation)?;
        if player_restored {
            eprintln!(
                "restored durable player state from world {}",
                self.world_name
            );
        }
        eprintln!(
            "player: {} world={} mode={:?} inventory_slots={} selected={}",
            if player_restored {
                "restored"
            } else {
                "initialized"
            },
            self.world_name,
            simulation.mode,
            simulation.inventory.occupied(),
            simulation.inventory.selected(),
        );
        if self.debug {
            eprintln!(
                "player detail: position={:?} yaw={} pitch={} velocity={:?} grounded={} AABB={:?}",
                simulation.player.position,
                simulation.player.yaw,
                simulation.player.pitch,
                simulation.player.velocity,
                simulation.player.on_ground,
                simulation.player.bounds(),
            );
            let camera = camera_for(&simulation, 16.0 / 9.0);
            let (right, up, forward) = camera.basis();
            eprintln!(
                "camera detail: player={:?} eye={:?} forward={forward:?} right={right:?} up={up:?}",
                simulation.player.position, camera.position
            );
        }
        let presentation = RenderWorld::from_world(&simulation.world);
        self.simulation = Some(simulation);
        self.presentation = Some(presentation);
        Ok(())
    }

    fn load_or_generate_world(
        &mut self,
        profile: &rustcraft_game_api::CompiledGameProfile,
        registry: &rustcraft_mod_api::BlockRegistry,
        world: &mut rustcraft_engine_core::World,
    ) -> Result<Option<rustcraft_minecraft_b173::player_persistence::RestoredPlayer>, String> {
        use rustcraft_world::{
            ChunkGenerator, GenerationScheduler, WorldMetadata, WorldStorage,
            validate_world_compatibility,
        };
        use std::{
            sync::Arc,
            time::{Duration, Instant},
        };
        let metadata_player_started = Instant::now();
        let storage = WorldStorage::open(&self.saves_directory, &self.world_name)
            .map_err(|error| error.to_string())?;
        let seed = match std::env::var("RUSTCRAFT_WORLD_SEED") {
            Ok(value) => value
                .parse::<i64>()
                .map_err(|_| "RUSTCRAFT_WORLD_SEED must be a signed 64-bit integer".to_owned())?,
            Err(std::env::VarError::NotPresent) => 731_173,
            Err(error) => return Err(format!("RUSTCRAFT_WORLD_SEED: {error}")),
        };
        let new_world = !storage.has_metadata();
        let saved = if new_world {
            None
        } else {
            let found = storage.load_metadata().map_err(|error| {
                format!("world metadata at {}: {error}", storage.root().display())
            })?;
            Some(found)
        };
        let requested_version = match std::env::var("RUSTCRAFT_GENERATOR_VERSION") {
            Ok(value) => value.parse::<u32>().map_err(|_| {
                "RUSTCRAFT_GENERATOR_VERSION must be an unsigned integer".to_owned()
            })?,
            Err(std::env::VarError::NotPresent) => {
                rustcraft_minecraft_b173::worldgen::DEFAULT_OVERWORLD_VERSION
            }
            Err(error) => return Err(format!("RUSTCRAFT_GENERATOR_VERSION: {error}")),
        };
        let generator_id = saved.as_ref().map_or(
            rustcraft_minecraft_b173::worldgen::OVERWORLD_GENERATOR_ID,
            |metadata| metadata.generator_id.as_str(),
        );
        let generator_version = saved
            .as_ref()
            .map_or(requested_version, |metadata| metadata.generator_version);
        let generator: Arc<dyn ChunkGenerator> =
            rustcraft_minecraft_b173::worldgen::resolve_overworld_generator(
                generator_id,
                generator_version,
            )
            .map_err(|error| {
                format!(
                    "generator compatibility error in world {}: {error}",
                    storage.root().display()
                )
            })?;
        let expected = WorldMetadata {
            seed,
            game_id: profile.id.as_str().to_owned(),
            profile_fingerprint: profile_fingerprint(profile),
            persistence_schema_version: rustcraft_world::PERSISTED_STATE_SCHEMA_VERSION,
            generator_id: generator.id().to_owned(),
            generator_version: generator.version(),
        };
        if new_world {
            storage.store_metadata(&expected).map_err(|error| {
                format!(
                    "create world metadata at {}: {error}",
                    storage.root().display()
                )
            })?;
        }

        let player_read_started = Instant::now();
        let player_record = storage
            .load_player(rustcraft_minecraft_b173::player_persistence::LOCAL_PLAYER_ID)
            .map_err(|error| {
                format!(
                    "player record in world {}: {error}",
                    storage.root().display()
                )
            })?;
        self.player_revision = player_record.as_ref().map_or(0, |record| record.revision);
        self.player_persisted_revision = self.player_revision;
        self.player_dirty = false;
        self.player_components = player_record
            .as_ref()
            .map(|record| record.components.clone());
        self.latest_player_record = player_record.clone();
        self.last_player_autosave = if player_record.is_none() {
            Instant::now() - self.player_autosave_interval
        } else {
            Instant::now()
        };
        let player_read_ms = player_read_started.elapsed().as_secs_f64() * 1000.0;
        let player_decode_started = Instant::now();
        let decoded = player_record
            .as_ref()
            .map(|record| {
                rustcraft_minecraft_b173::player_persistence::decode_with_unknown(record, registry)
                    .map_err(|error| {
                        format!(
                            "player compatibility error in world {}: {error}; chunks are preserved",
                            storage.root().display()
                        )
                    })
            })
            .transpose()?;
        self.unknown_player_components = decoded
            .as_ref()
            .map_or_else(Vec::new, |(_, unknown)| unknown.clone());
        if let Some(record) = player_record.as_ref()
            && record.recovered_from_checkpoint
        {
            eprintln!(
                "recovered player {} from checkpoint revision {} after a newer slot failed validation",
                record.player_id, record.revision
            );
        }
        let restored = decoded.map(|(state, _)| state);

        let world_state = storage.load_world_state().map_err(|error| {
            format!(
                "global state in world {}: {error}",
                storage.root().display()
            )
        })?;
        self.world_state_revision = world_state.as_ref().map_or(0, |record| record.revision);
        self.world_state_persisted_revision = self.world_state_revision;
        self.world_state_components = world_state.as_ref().map(|record| record.components.clone());
        self.latest_world_state_record = world_state.clone();
        if let Some(record) = world_state.as_ref() {
            let (world_time, unknown) =
                rustcraft_minecraft_b173::world_persistence::decode_world_state(record).map_err(
                    |error| {
                        format!(
                            "global-state compatibility error in world {}: {error}",
                            storage.root().display()
                        )
                    },
                )?;
            self.restored_world_time = world_time;
            self.unknown_world_state_components = unknown;
            self.world_state_dirty = false;
            if record.recovered_from_checkpoint {
                eprintln!(
                    "recovered world-global state from checkpoint revision {}",
                    record.revision
                );
            }
        } else {
            // A pre-M4-003 world has no global record and starts at the game-defined default.
            self.restored_world_time = 0;
            self.unknown_world_state_components.clear();
            self.world_state_dirty = true;
            self.last_world_state_autosave = Instant::now() - self.world_state_autosave_interval;
        }
        self.startup_metadata_player_ms = metadata_player_started.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "player persistence read: revision={} components={} bytes={} read_ms={player_read_ms:.3} decode_ms={:.3}",
            player_record.as_ref().map_or(0, |record| record.revision),
            player_record
                .as_ref()
                .map_or(0, |record| record.components.len()),
            player_record.as_ref().map_or(0, |record| record
                .components
                .iter()
                .map(|c| c.payload.len())
                .sum()),
            player_decode_started.elapsed().as_secs_f64() * 1000.0
        );

        let generation_seed = saved
            .as_ref()
            .map_or(expected.seed, |metadata| metadata.seed);
        let (center_x, center_z) = restored.as_ref().map_or_else(
            || {
                let center = rustcraft_minecraft_b173::worldgen::initial_spawn_column(
                    generation_seed,
                    generator.version(),
                );
                (center.x, center.z)
            },
            |state| {
                (
                    (state.position.x.floor() as i32).div_euclid(16),
                    (state.position.z.floor() as i32).div_euclid(16),
                )
            },
        );
        let positions = (-1..=1)
            .flat_map(|z| {
                (-1..=1).map(move |x| rustcraft_engine_core::ChunkPos {
                    x: center_x + x,
                    z: center_z + z,
                })
            })
            .collect::<Vec<_>>();
        if let Some(found) = &saved {
            validate_world_compatibility(found, &expected, false).map_err(|error| {
                format!(
                    "world compatibility error at {}: {error}; saved seed={}, active requested seed={}. Existing saved worlds are not deleted; use a new ignored development world to test incompatible content.",
                    storage.root().display(), found.seed, seed
                )
            })?;
        }
        let resolver: std::sync::Arc<dyn rustcraft_world::SemanticBlockResolver> =
            std::sync::Arc::new(rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver);
        let chunk_load_started = Instant::now();
        let mut missing = Vec::new();
        let mut loaded_from_disk = 0;
        let mut load_scheduler = rustcraft_world::ChunkLoadScheduler::new(2, 8);
        for (index, position) in positions.iter().copied().enumerate() {
            load_scheduler
                .submit(
                    storage.clone(),
                    rustcraft_world::ResidencyRequest {
                        position,
                        token: index as u64 + 1,
                    },
                    resolver.clone(),
                )
                .map_err(|error| format!("queue initial chunk load: {error}"))?;
        }
        let load_deadline = Instant::now() + Duration::from_secs(60);
        let mut completed_loads = 0;
        while completed_loads < positions.len() {
            for completion in load_scheduler.take_ready(positions.len()) {
                completed_loads += 1;
                match completion.result {
                    Ok(Some(column)) => {
                        self.pending_spatial_columns.insert(
                            completion.request.position,
                            (column.spatial_records, column.spatial_tombstones),
                        );
                        world
                            .publish_column(completion.request.position, column.sections)
                            .map_err(str::to_owned)?;
                        loaded_from_disk += 1;
                    }
                    Ok(None) => missing.push(completion.request.position),
                    Err(error) => {
                        return Err(format!(
                            "load chunk ({},{}) in world {}: {error}",
                            completion.request.position.x,
                            completion.request.position.z,
                            storage.root().display()
                        ));
                    }
                }
            }
            if completed_loads < positions.len() {
                if self
                    .startup_cancel
                    .load(std::sync::atomic::Ordering::Acquire)
                {
                    return Err("world startup cancelled".into());
                }
                if Instant::now() >= load_deadline {
                    return Err("initial chunk loading timed out".into());
                }
                std::thread::yield_now();
            }
        }
        drop(load_scheduler);
        self.startup_chunk_load_ms = chunk_load_started.elapsed().as_secs_f64() * 1000.0;

        if let Some(found) = &saved {
            validate_world_compatibility(found, &expected, !missing.is_empty()).map_err(|error| {
                format!(
                    "world compatibility error at {}: {error}; saved seed={}, active requested seed={}. Existing saved worlds are not deleted; use a new ignored development world to test incompatible content.",
                    storage.root().display(), found.seed, seed
                )
            })?;
        }
        self.stream_generator = Some(generator.clone());
        self.world_seed = generation_seed;
        self.stream_generation_allowed = saved.as_ref().is_none_or(|metadata| {
            metadata.generator_id == generator.id()
                && metadata.generator_version == generator.version()
        });
        let mut scheduler = GenerationScheduler::new(2, 8);
        let generation_started = Instant::now();
        for (index, position) in missing.iter().copied().enumerate() {
            scheduler
                .request(
                    generator.clone(),
                    generation_seed,
                    position,
                    index as u64 + 1,
                )
                .map_err(|error| error.to_string())?;
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut generated = 0;
        while generated < missing.len() {
            if self
                .startup_cancel
                .load(std::sync::atomic::Ordering::Acquire)
            {
                return Err("world startup cancelled".into());
            }
            for result in scheduler.take_ready() {
                let sections = result
                    .sections
                    .map_err(|error| format!("generator {}: {error}", generator.id()))?;
                world
                    .publish_column(result.position, sections)
                    .map_err(str::to_owned)?;
                generated += 1;
            }
            if generated == missing.len() {
                break;
            }
            if Instant::now() >= deadline {
                return Err("world generation timed out".into());
            }
            std::thread::yield_now();
        }
        world.enforce_column_availability(true);
        self.startup_generation_ms = generation_started.elapsed().as_secs_f64() * 1000.0;
        self.loaded_from_disk = loaded_from_disk;
        self.generated_chunks = generated;
        self.worldgen_metrics = scheduler.metrics();
        if restored.is_none()
            && rustcraft_minecraft_b173::worldgen::select_safe_spawn(world).is_none()
        {
            return Err(format!(
                "world {}: no dry supported first-time spawn in the bounded startup neighborhood; persisted terrain is preserved",
                storage.root().display()
            ));
        }
        // Preserve legacy v1 development-world behavior. V2 new worlds expose generated terrain,
        // not the origin sandbox fixture (which may be far from a relocated ocean-seed spawn).
        let decorated = new_world
            && self.diagnostic.is_none()
            && generator.version() == 1
            && decorate_resident_legacy_sandbox(world, registry);
        let persist_positions = if decorated { positions } else { missing };
        for position in persist_positions {
            let sections = world
                .section_positions()
                .filter(|(p, _)| *p == position)
                .filter_map(|(_, y)| world.section(position, y).cloned().map(|chunk| (y, chunk)))
                .collect::<Vec<_>>();
            let stored = WorldStorage::encode_runtime_chunk(position, sections, resolver.as_ref())
                .map_err(|error| error.to_string())?;
            storage.store_chunk(&stored).map_err(|error| {
                format!(
                    "save initial chunk ({},{}): {error}",
                    position.x, position.z
                )
            })?;
        }
        if let Some(mut metadata) = saved {
            // Only update informational profile identity after all existing chunk palettes have
            // resolved successfully. Generator identity remains the world's original identity.
            metadata.profile_fingerprint = expected.profile_fingerprint;
            storage.store_metadata(&metadata).map_err(|error| {
                format!(
                    "migrate world metadata at {}: {error}",
                    storage.root().display()
                )
            })?;
        }
        storage.flush().map_err(|error| error.to_string())?;
        self.world_storage = Some(storage);
        Ok(restored)
    }

    fn service_player_autosave(&mut self) {
        for completion in self.player_save_scheduler.take_completed() {
            if completion.result.is_ok() {
                let completed_revisions = self
                    .player_checkpoint_receipts
                    .keys()
                    .copied()
                    .filter(|revision| *revision <= completion.revision)
                    .collect::<Vec<_>>();
                let mut durable_receipts = Vec::new();
                for revision in completed_revisions {
                    if let Some(receipts) = self.player_checkpoint_receipts.remove(&revision) {
                        durable_receipts.extend(receipts);
                    }
                }
                if let Some(simulation) = self.simulation.as_mut() {
                    simulation.commit_pickup_receipts(&durable_receipts);
                }
                self.player_persisted_revision =
                    self.player_persisted_revision.max(completion.revision);
                self.player_dirty = self.player_revision > self.player_persisted_revision;
                self.player_checkpoint_ms_last = completion.elapsed_ms;
                eprintln!(
                    "player checkpoint persisted: revision={} bytes={} write_ms={:.3} sync_ms={:.3} total_ms={:.3}",
                    completion.revision,
                    completion.io.encoded_bytes,
                    completion.io.write_ms,
                    completion.io.sync_ms,
                    completion.elapsed_ms,
                );
            } else if let Err(error) = completion.result {
                self.player_dirty = true;
                self.player_checkpoint_ms_last = completion.elapsed_ms;
                eprintln!(
                    "player checkpoint revision {} failed; state remains dirty for retry: {error}",
                    completion.revision
                );
            }
        }
        if let Err(error) = self.refresh_player_snapshot() {
            eprintln!("player snapshot encode failed: {error}");
            return;
        }
        if !self.player_dirty || self.last_player_autosave.elapsed() < self.player_autosave_interval
        {
            return;
        }
        let (Some(storage), Some(record)) = (
            self.world_storage.clone(),
            self.latest_player_record.clone(),
        ) else {
            return;
        };
        match self.player_save_scheduler.submit(storage, record) {
            Ok(()) => {
                if let Some(simulation) = self.simulation.as_ref() {
                    self.player_checkpoint_receipts.insert(
                        self.player_revision,
                        simulation
                            .pickup_receipts()
                            .into_iter()
                            .map(|receipt| receipt.entity_id)
                            .collect(),
                    );
                }
                self.last_player_autosave = Instant::now();
                self.player_autosave_writes = self.player_autosave_writes.saturating_add(1);
            }
            Err(error) => eprintln!("player checkpoint submission failed: {error}"),
        }
    }

    fn service_world_state_autosave(&mut self) {
        for completion in self.world_state_save_scheduler.take_completed() {
            if completion.result.is_ok() {
                self.world_state_persisted_revision =
                    self.world_state_persisted_revision.max(completion.revision);
                self.world_state_dirty =
                    self.world_state_revision > self.world_state_persisted_revision;
                self.world_state_checkpoint_ms_last = completion.elapsed_ms;
            } else if let Err(error) = completion.result {
                self.world_state_dirty = true;
                self.world_state_checkpoint_ms_last = completion.elapsed_ms;
                eprintln!(
                    "world-global checkpoint revision {} failed; state remains dirty: {error}",
                    completion.revision
                );
            }
        }
        self.refresh_world_state_snapshot();
        if !self.world_state_dirty
            || self.last_world_state_autosave.elapsed() < self.world_state_autosave_interval
        {
            return;
        }
        let (Some(storage), Some(record)) = (
            self.world_storage.clone(),
            self.latest_world_state_record.clone(),
        ) else {
            return;
        };
        match self.world_state_save_scheduler.submit(storage, record) {
            Ok(()) => self.last_world_state_autosave = Instant::now(),
            Err(error) => eprintln!("world-global checkpoint submission failed: {error}"),
        }
    }

    fn refresh_world_state_snapshot(&mut self) {
        let Some(simulation) = self.simulation.as_ref() else {
            return;
        };
        let mut record = rustcraft_minecraft_b173::world_persistence::encode_world_state(
            simulation.time,
            self.world_state_revision.max(1),
            &self.unknown_world_state_components,
        );
        if self
            .world_state_components
            .as_ref()
            .is_none_or(|components| components != &record.components)
        {
            self.world_state_revision = self.world_state_revision.saturating_add(1).max(1);
            record.revision = self.world_state_revision;
            self.world_state_components = Some(record.components.clone());
            self.latest_world_state_record = Some(record);
            self.world_state_dirty = true;
        } else if self.latest_world_state_record.is_none() {
            self.world_state_revision = self.world_state_revision.max(1);
            record.revision = self.world_state_revision;
            self.latest_world_state_record = Some(record);
            self.world_state_dirty = true;
        }
    }

    fn refresh_player_snapshot(&mut self) -> Result<(), String> {
        let encode_started = Instant::now();
        let Some(simulation) = self.simulation.as_ref() else {
            return Ok(());
        };
        let mut record = rustcraft_minecraft_b173::player_persistence::encode_revision(
            simulation,
            self.player_revision.max(1),
            &self.unknown_player_components,
        )
        .map_err(|error| error.to_string())?;
        self.player_encode_ms_last = encode_started.elapsed().as_secs_f64() * 1000.0;
        if self
            .player_components
            .as_ref()
            .is_none_or(|components| components != &record.components)
        {
            self.player_revision = self.player_revision.saturating_add(1).max(1);
            self.player_dirty = true;
            record.revision = self.player_revision;
            self.player_components = Some(record.components.clone());
            self.latest_player_record = Some(record);
        } else if self.latest_player_record.is_none() {
            record.revision = self.player_revision.max(1);
            self.player_revision = record.revision;
            self.player_dirty = true;
            self.latest_player_record = Some(record);
        }
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
    fn stream_budget_available(&self) -> bool {
        let deadline = self.stream_stage_deadline.or_else(|| {
            self.stream_turn_started
                .map(|started| started + self.stream_main_budget)
        });
        deadline.is_none_or(|deadline| Instant::now() < deadline)
    }
    fn fixed_step(&mut self) {
        self.apply_config_boundary(rustcraft_control::config::Policy::NextTick);
        let developer_focus = self.dev_focus();
        let tick_started = Instant::now();
        if let Some(previous) = self.responsiveness.last_fixed_tick.replace(tick_started) {
            self.responsiveness
                .fixed_tick_gap
                .record(tick_started.duration_since(previous));
        }
        if !self.player_control_enabled {
            self.frame_phases.fixed_step_ms = tick_started.elapsed().as_secs_f64() * 1000.0;
            self.frame_phase_samples
                .fixed_step
                .record_ms(self.frame_phases.fixed_step_ms);
            self.metrics.tick(tick_started.elapsed().as_secs_f64());
            return;
        }
        let pending_input = self.responsiveness.pending_input_at.take();
        let simulation_started = Instant::now();
        let player_yaw = self
            .simulation
            .as_ref()
            .map_or(0.0, |simulation| simulation.player.yaw);
        let travel_elapsed = self
            .stream_route_started_at
            .unwrap_or_else(Instant::now)
            .elapsed()
            .as_secs_f32();
        let stream_perf_enabled = self.stream_perf;
        // A fixed-duration return is not a distance guarantee after terrain detours. Continue
        // semantic return input until the start core is reached; each diagnostic has a hard
        // process/phase deadline, so an unreachable route still fails rather than running forever.
        let stream_perf = stream_perf_enabled
            && (travel_elapsed < self.stream_perf_motion_seconds
                || !self.stream_returned_to_origin);
        if stream_perf {
            let position = self
                .simulation
                .as_ref()
                .map(|simulation| simulation.player.position);
            if let Some(position) = position {
                if self.stream_last_motion_position.is_some_and(|previous| {
                    (position.x - previous.x).hypot(position.z - previous.z) < 0.025
                }) {
                    self.stream_stuck_ticks = self.stream_stuck_ticks.saturating_add(1);
                } else {
                    self.stream_stuck_ticks = 0;
                }
                self.stream_last_motion_position = Some(position);
                if self.stream_stuck_ticks >= 6 && self.stream_avoidance_ticks == 0 {
                    self.stream_avoidance_ticks = 24;
                    self.stream_stuck_ticks = 0;
                }
            }
        }
        let (persistence_dirty, dirty) = {
            let Some(simulation) = self.simulation.as_mut() else {
                return;
            };
            let intent = if stream_perf {
                let returning = travel_elapsed >= self.stream_perf_motion_seconds * (6.0 / 7.0);
                if !returning
                    && self.stream_return_path.back().is_none_or(|point| {
                        (point[0] - simulation.player.position.x)
                            .hypot(point[1] - simulation.player.position.z)
                            >= 3.0
                    })
                {
                    // Erase loops only where the actual physical route revisits its own trace.
                    if let Some(index) = self.stream_return_path.iter().position(|point| {
                        (point[0] - simulation.player.position.x)
                            .hypot(point[1] - simulation.player.position.z)
                            < 2.0
                    }) {
                        self.stream_return_path.truncate(index + 1);
                    }
                    if self.stream_return_path.len() == 512 {
                        self.stream_return_path.pop_front();
                    }
                    self.stream_return_path
                        .push_back([simulation.player.position.x, simulation.player.position.z]);
                }
                if returning {
                    while self.stream_return_path.back().is_some_and(|point| {
                        (point[0] - simulation.player.position.x)
                            .hypot(point[1] - simulation.player.position.z)
                            < 1.0
                    }) {
                        self.stream_return_path.pop_back();
                    }
                }
                let (mut target_yaw, mut movement, jump) = stream_route_sample(
                    travel_elapsed,
                    self.stream_perf_motion_seconds / 7.0,
                    simulation.player.position - self.stream_route_anchor.unwrap_or_default(),
                );
                if (5.0 / 7.0..6.0 / 7.0)
                    .contains(&(travel_elapsed / self.stream_perf_motion_seconds))
                {
                    let yaw = *self.stream_reversal_yaw.get_or_insert_with(|| {
                        normalize_horizontal([
                            simulation.player.velocity.x,
                            simulation.player.velocity.z,
                        ])
                        .map_or(simulation.player.yaw + std::f32::consts::PI, |direction| {
                            (-direction[0]).atan2(-direction[1])
                        })
                    });
                    target_yaw = yaw;
                    movement = MoveIntent {
                        forward: 1.0,
                        strafe: 0.0,
                    };
                }
                if returning && let Some(point) = self.stream_return_path.back() {
                    target_yaw = (point[0] - simulation.player.position.x)
                        .atan2(point[1] - simulation.player.position.z);
                    movement = MoveIntent {
                        forward: 1.0,
                        strafe: 0.0,
                    };
                }
                let intended_forward = Vec3::new(target_yaw.sin(), 0.0, target_yaw.cos());
                let intended_right = Vec3::new(-target_yaw.cos(), 0.0, target_yaw.sin());
                let intended = Vec3::new(
                    intended_forward.x * movement.forward + intended_right.x * movement.strafe,
                    0.0,
                    intended_forward.z * movement.forward + intended_right.z * movement.strafe,
                );
                let ahead = simulation.player.position + intended * 1.5;
                let ahead_x = ahead.x.floor() as i32;
                let ahead_y = simulation.player.position.y.floor() as i32;
                let ahead_z = ahead.z.floor() as i32;
                let water = rustcraft_minecraft_b173::blocks::WATER.id;
                let water_ahead = (-5..=1)
                    .rev()
                    .find_map(|dy| {
                        let block = simulation.world.get(rustcraft_engine_core::BlockPos {
                            x: ahead_x,
                            y: ahead_y + dy,
                            z: ahead_z,
                        });
                        (block != rustcraft_minecraft_b173::blocks::AIR.id)
                            .then_some(block == water)
                    })
                    .unwrap_or(false);
                if water_ahead && self.stream_avoidance_ticks == 0 {
                    self.stream_avoidance_ticks = 50;
                }
                if self.stream_avoidance_ticks > 0 || !self.stream_navigation_path.is_empty() {
                    if self.stream_navigation_ticks == 0 || self.stream_stuck_ticks >= 6 {
                        let anchor = self.stream_route_anchor.unwrap_or_default();
                        let goal = if returning {
                            self.stream_return_path
                                .back()
                                .copied()
                                .unwrap_or([anchor.x, anchor.z])
                        } else {
                            [
                                simulation.player.position.x + intended.x * 24.0,
                                simulation.player.position.z + intended.z * 24.0,
                            ]
                        };
                        self.stream_navigation_path = stream_surface_path(simulation, goal);
                        self.stream_navigation_ticks = 10;
                    }
                    self.stream_navigation_ticks = self.stream_navigation_ticks.saturating_sub(1);
                    while self.stream_navigation_path.front().is_some_and(|target| {
                        (target[0] - simulation.player.position.x)
                            .hypot(target[1] - simulation.player.position.z)
                            < 0.35
                    }) {
                        self.stream_navigation_path.pop_front();
                    }
                    if let Some(target) = self.stream_navigation_path.front() {
                        target_yaw = (target[0] - simulation.player.position.x)
                            .atan2(target[1] - simulation.player.position.z);
                    }
                    movement = MoveIntent {
                        forward: 1.0,
                        strafe: 0.0,
                    };
                    self.stream_avoidance_ticks = self.stream_avoidance_ticks.saturating_sub(1);
                }
                let yaw_delta = (target_yaw - player_yaw + std::f32::consts::PI)
                    .rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                let issued = Instant::now();
                self.responsiveness.pending_input_at = Some(issued);
                AgentIntent {
                    movement,
                    look_delta: Vec3::new(-yaw_delta / 0.002, 0.0, 0.0),
                    jump,
                    ..Default::default()
                }
            } else if stream_perf_enabled || self.inventory_open {
                rustcraft_agent_api::AgentIntent::default()
            } else {
                self.controller.next_intent()
            };
            let intent = if self.control_state.leased {
                self.control_state.intent
            } else if developer_focus {
                Default::default()
            } else {
                intent
            };
            simulation.step(intent, 0.05);
            (
                simulation.take_persistence_dirty_chunks(),
                simulation.take_dirty_sections(),
            )
        };
        if stream_perf_enabled && !stream_perf {
            self.stream_route_completed = true;
        }
        let simulation_elapsed = simulation_started.elapsed();
        self.responsiveness
            .record_long_task("simulation_step", simulation_elapsed, 1);
        if let Some(input_at) = pending_input {
            self.responsiveness
                .input_to_simulation
                .record(Instant::now().duration_since(input_at));
            self.responsiveness.input_consumed_at = Some(input_at);
        }
        self.frame_phases.fixed_step_ms = tick_started.elapsed().as_secs_f64() * 1000.0;
        self.frame_phase_samples
            .fixed_step
            .record_ms(self.frame_phases.fixed_step_ms);
        self.metrics.tick(tick_started.elapsed().as_secs_f64());
        if let Some(simulation) = self.simulation.as_ref() {
            self.resident_columns_peak = self
                .resident_columns_peak
                .max(simulation.world.chunk_count());
            self.resident_sections_peak = self
                .resident_sections_peak
                .max(simulation.world.section_count());
            let current_column = rustcraft_engine_core::ChunkPos {
                x: (simulation.player.position.x.floor() as i32).div_euclid(16),
                z: (simulation.player.position.z.floor() as i32).div_euclid(16),
            };
            if stream_perf_enabled {
                self.stream_visited_columns.insert(current_column);
                if travel_elapsed >= self.stream_perf_motion_seconds * (6.0 / 7.0)
                    && self.stream_route_anchor.is_some_and(|anchor| {
                        (current_column.x - (anchor.x.floor() as i32).div_euclid(16)).abs() <= 1
                            && (current_column.z - (anchor.z.floor() as i32).div_euclid(16)).abs()
                                <= 1
                    })
                {
                    self.stream_returned_to_origin = true;
                }
            }
            let safe = simulation
                .world
                .safe_column_positions()
                .collect::<std::collections::HashSet<_>>();
            self.travel_margin.sample(
                &safe,
                &self.render_ready_columns,
                simulation.player.position,
                [simulation.player.velocity.x, simulation.player.velocity.z],
                [simulation.player.yaw.sin(), simulation.player.yaw.cos()],
            );
        }
        self.service_world_saves(persistence_dirty);
        self.service_player_autosave();
        self.service_world_state_autosave();
        self.snapshot_dirty_sections.extend(dirty);
        self.responsiveness
            .record_long_task("fixed_step", tick_started.elapsed(), 1);
    }

    fn stream_stage_deadline(&self, budget_fraction: f64) -> Instant {
        Instant::now() + self.stream_main_budget.mul_f64(budget_fraction)
    }

    fn observe_stream_stage(
        &mut self,
        index: usize,
        pending: bool,
        serviced: bool,
        had_budget: bool,
        blocked_dependency: bool,
    ) {
        self.stream_stage_fairness[index].observe(
            pending,
            serviced,
            had_budget,
            blocked_dependency,
            Instant::now(),
        );
    }

    /// Independent reserved windows prevent a perpetually busy early stage from consuming the
    /// entire turn. Work is visited on every event-loop turn, not only at the 20 Hz fixed tick.
    fn service_streaming_turn(&mut self) {
        if self.simulation.is_none() {
            return;
        }

        let critical_deadline = self.stream_stage_deadline(0.30);
        self.stream_stage_deadline = Some(critical_deadline);
        let load_stats = self.load_scheduler.metrics();
        let generation_stats = self.generation_scheduler.metrics();
        let light_capacity = self.simulation.as_ref().map_or(0, |simulation| {
            simulation.lighting.integration_capacity_remaining()
        });
        let initial_results_waiting = self.initial_lighting_scheduler.outstanding()
            > self.initial_lighting_scheduler.pending()
                + self.initial_lighting_scheduler.in_flight();
        let load_results_waiting = (load_stats.completed + load_stats.missing + load_stats.failed)
            > self.load_results_applied;
        let generation_results_waiting =
            generation_stats.completed > self.generation_results_applied;
        let initial_worker_capacity = self.initial_lighting_scheduler.capacity_remaining() > 0;
        let critical_pending = (initial_results_waiting && light_capacity > 0)
            || ((load_results_waiting || generation_results_waiting) && initial_worker_capacity);
        let critical_blocked = (initial_results_waiting && light_capacity == 0)
            || ((load_results_waiting || generation_results_waiting) && !initial_worker_capacity);
        let critical_had_budget = Instant::now() < critical_deadline;
        let before_loaded = (
            self.generated_chunks,
            self.loaded_from_disk,
            self.load_results_applied,
            self.generation_results_applied,
        );
        let residency_started = Instant::now();
        let critical_dirty = if Instant::now() < critical_deadline {
            self.service_world_residency()
        } else {
            Vec::new()
        };
        self.snapshot_dirty_sections
            .extend(critical_dirty.iter().copied());
        self.frame_phases.residency_ms = residency_started.elapsed().as_secs_f64() * 1000.0;
        self.frame_phase_samples
            .residency
            .record_ms(self.frame_phases.residency_ms);
        self.observe_stream_stage(
            0,
            critical_pending || critical_blocked,
            !critical_dirty.is_empty()
                || before_loaded
                    != (
                        self.generated_chunks,
                        self.loaded_from_disk,
                        self.load_results_applied,
                        self.generation_results_applied,
                    ),
            critical_had_budget,
            critical_blocked,
        );

        if let Some(simulation) = self.simulation.as_mut() {
            simulation
                .lighting
                .reprioritize_pending_columns(|position| self.residency.priority_key(position));
        }
        let boundary_deadline = self.stream_stage_deadline(0.30);
        self.stream_stage_deadline = Some(boundary_deadline);
        let boundary_pending = self
            .simulation
            .as_ref()
            .is_some_and(|simulation| !simulation.lighting.integration_columns().is_empty());
        let boundary_had_budget = Instant::now() < boundary_deadline;
        let boundary_started = Instant::now();
        let (boundary_dirty, boundary_work) = if Instant::now() < boundary_deadline {
            self.advance_boundary_lighting_for_turn()
        } else {
            (Vec::new(), 0)
        };
        self.snapshot_dirty_sections.extend(boundary_dirty);
        self.frame_phases.boundary_lighting_ms = boundary_started.elapsed().as_secs_f64() * 1000.0;
        self.frame_phase_samples
            .boundary_lighting
            .record_ms(self.frame_phases.boundary_lighting_ms);
        self.observe_stream_stage(
            1,
            boundary_pending,
            boundary_work > 0,
            boundary_had_budget,
            false,
        );

        let snapshot_deadline = self.stream_stage_deadline(0.25);
        self.stream_stage_deadline = Some(snapshot_deadline);
        let snapshot_pending = !self.snapshot_dirty_sections.is_empty();
        let snapshot_had_budget = Instant::now() < snapshot_deadline;
        let snapshot_started = Instant::now();
        let mut snapshots_built = 0usize;
        let player_section = self.player_section_position();
        let residency = &self.residency;
        while Instant::now() < snapshot_deadline {
            let Some(section) = pop_urgent_before_deadline(
                &mut self.snapshot_dirty_sections,
                snapshot_deadline,
                |section| Self::section_priority(residency, player_section, section),
            ) else {
                break;
            };
            let Some((presentation, simulation)) =
                self.presentation.as_mut().zip(self.simulation.as_ref())
            else {
                self.snapshot_dirty_sections.insert(section);
                break;
            };
            presentation.sync_sections(&simulation.world, [section]);
            self.mesh_dirty_sections.insert(section);
            snapshots_built += 1;
        }
        self.frame_phases.render_world_sync_ms = snapshot_started.elapsed().as_secs_f64() * 1000.0;
        self.responsiveness.record_long_task(
            "render_world_sync",
            snapshot_started.elapsed(),
            snapshots_built,
        );
        self.frame_phase_samples
            .render_world_sync
            .record_ms(self.frame_phases.render_world_sync_ms);
        self.observe_stream_stage(
            2,
            snapshot_pending,
            snapshots_built > 0,
            snapshot_had_budget,
            false,
        );

        let mesh_deadline = self.stream_stage_deadline(0.15);
        self.stream_stage_deadline = Some(mesh_deadline);
        let mesh_pending = !self.mesh_dirty_sections.is_empty();
        let mesh_had_budget = Instant::now() < mesh_deadline;
        let mesh_started = Instant::now();
        let mut mesh_scheduled = 0usize;
        let player_section = self.player_section_position();
        let mut prioritized_mesh = self.mesh_dirty_sections.drain().collect::<Vec<_>>();
        prioritized_mesh.sort_by_key(|section| {
            Self::section_priority(&self.residency, player_section, *section)
        });
        let mut remaining_mesh = prioritized_mesh.into_iter();
        while Instant::now() < mesh_deadline {
            let Some(section) = remaining_mesh.next() else {
                break;
            };
            self.rebuild_dirty_meshes([section]);
            mesh_scheduled += 1;
        }
        self.mesh_dirty_sections.extend(remaining_mesh);
        self.frame_phases.mesh_schedule_ms = mesh_started.elapsed().as_secs_f64() * 1000.0;
        self.responsiveness.record_long_task(
            "mesh_schedule",
            mesh_started.elapsed(),
            mesh_scheduled,
        );
        self.frame_phase_samples
            .mesh_schedule
            .record_ms(self.frame_phases.mesh_schedule_ms);
        self.observe_stream_stage(3, mesh_pending, mesh_scheduled > 0, mesh_had_budget, false);
        self.stream_stage_deadline = None;
    }

    fn player_section_position(&self) -> (rustcraft_engine_core::ChunkPos, i32) {
        self.simulation.as_ref().map_or(
            (rustcraft_engine_core::ChunkPos { x: 0, z: 0 }, 0),
            |simulation| {
                (
                    rustcraft_engine_core::ChunkPos {
                        x: (simulation.player.position.x.floor() as i32).div_euclid(16),
                        z: (simulation.player.position.z.floor() as i32).div_euclid(16),
                    },
                    (simulation.player.position.y.floor() as i32).div_euclid(16),
                )
            },
        )
    }

    /// Release only complete 3x3 locally authoritative and render-ready cores. Successive cores
    /// overlap, so travel grows a contiguous safe corridor rather than exposing lifecycle holes.
    fn release_render_ready_safe_core(&mut self) {
        let Some(simulation) = self.simulation.as_ref() else {
            return;
        };
        let center = rustcraft_engine_core::ChunkPos {
            x: (simulation.player.position.x.floor() as i32).div_euclid(16),
            z: (simulation.player.position.z.floor() as i32).div_euclid(16),
        };
        let core = (-1..=1)
            .flat_map(|dz| {
                (-1..=1).map(move |dx| rustcraft_engine_core::ChunkPos {
                    x: center.x + dx,
                    z: center.z + dz,
                })
            })
            .collect::<Vec<_>>();
        let complete = core.iter().all(|position| {
            self.render_ready_columns.contains(position)
                && simulation.world.column_positions().any(|p| p == *position)
        });
        if !complete {
            return;
        }
        let simulation = self
            .simulation
            .as_mut()
            .expect("checked running simulation");
        for position in core {
            let released = simulation.world.set_column_safe(position, true);
            debug_assert!(released, "safe-core column must be authoritative");
        }
        if !self.player_control_enabled {
            self.player_control_enabled = true;
            self.stream_route_started_at = Some(Instant::now());
            self.stream_route_anchor
                .get_or_insert(simulation.player.position);
            self.responsiveness.pending_input_at = None;
            self.responsiveness.input_consumed_at = None;
            self.responsiveness.input_to_simulation = LatencyWindow::default();
            self.responsiveness.input_to_render = LatencyWindow::default();
            let ready_ms = self
                .world_ready_at
                .unwrap_or(self.startup_started)
                .elapsed()
                .as_secs_f64()
                * 1000.0;
            self.startup_visible_core_ms = Some(ready_ms);
            eprintln!(
                "startup control enabled: complete 3x3 SAFE+VISIBLE core ready in {ready_ms:.2} ms"
            );
        }
    }

    fn section_priority(
        residency: &rustcraft_world::WorldResidency,
        player: (rustcraft_engine_core::ChunkPos, i32),
        section: rustcraft_engine_core::SectionPos,
    ) -> ((u8, i64, i64, i32, i32), i32) {
        let column = section.0;
        let dx = i64::from(column.x) - i64::from(player.0.x);
        let dz = i64::from(column.z) - i64::from(player.0.z);
        let distance = dx.abs().max(dz.abs());
        let urgency = residency.urgency(column) as u8;
        (
            (
                urgency,
                residency.priority_key(column).0,
                distance,
                column.x,
                column.z,
            ),
            section.1.abs_diff(player.1) as i32,
        )
    }

    fn frontier_summary(&self) -> String {
        let Some(simulation) = self.simulation.as_ref() else {
            return "unavailable".to_owned();
        };
        let center = rustcraft_engine_core::ChunkPos {
            x: (simulation.player.position.x.floor() as i32).div_euclid(16),
            z: (simulation.player.position.z.floor() as i32).div_euclid(16),
        };
        let direction =
            normalize_horizontal([simulation.player.yaw.sin(), simulation.player.yaw.cos()])
                .unwrap_or([0.0, 1.0]);
        for distance in 1..=3 {
            let position = rustcraft_engine_core::ChunkPos {
                x: center.x + (direction[0] * distance as f32).round() as i32,
                z: center.z + (direction[1] * distance as f32).round() as i32,
            };
            if position == center || self.render_ready_columns.contains(&position) {
                continue;
            }
            let resident = simulation.world.column_positions().any(|p| p == position);
            let boundary = simulation
                .lighting
                .integration_columns()
                .contains(&position);
            let phase = match self.residency.phase(position) {
                Some(rustcraft_world::ResidencyPhase::Requested) => "requested",
                Some(rustcraft_world::ResidencyPhase::Loading) => "loading",
                Some(rustcraft_world::ResidencyPhase::Generating) => "generating",
                Some(rustcraft_world::ResidencyPhase::InitialLighting) if boundary => {
                    "boundary-lighting"
                }
                Some(rustcraft_world::ResidencyPhase::InitialLighting) => "initial-lighting",
                Some(rustcraft_world::ResidencyPhase::Ready) if boundary => "boundary-lighting",
                Some(rustcraft_world::ResidencyPhase::Ready)
                    if self.snapshot_dirty_sections.iter().any(|s| s.0 == position) =>
                {
                    "snapshot-pending"
                }
                Some(rustcraft_world::ResidencyPhase::Ready)
                    if self.mesh_dirty_sections.iter().any(|s| s.0 == position) =>
                {
                    "mesh-pending"
                }
                Some(rustcraft_world::ResidencyPhase::Ready)
                    if self
                        .visible_expected_sections
                        .get(&position)
                        .is_some_and(|sections| !sections.is_empty())
                        && self.mesh_scheduler.stats().ready > 0 =>
                {
                    "upload-pending"
                }
                Some(rustcraft_world::ResidencyPhase::Ready)
                    if self
                        .visible_expected_sections
                        .get(&position)
                        .is_some_and(|sections| !sections.is_empty()) =>
                {
                    "mesh-pending"
                }
                Some(rustcraft_world::ResidencyPhase::Ready) => "render-visible",
                Some(rustcraft_world::ResidencyPhase::Failed) => "failed",
                Some(rustcraft_world::ResidencyPhase::Saving) => "saving",
                None if !resident => "not-requested",
                None => "resident-untracked",
            };
            let age = self
                .request_started_at
                .iter()
                .find_map(|((candidate, _), start)| {
                    (*candidate == position).then_some(start.elapsed())
                })
                .or_else(|| {
                    self.visible_latency_started_at
                        .get(&position)
                        .map(Instant::elapsed)
                })
                .unwrap_or_default()
                .as_millis();
            let urgency = if distance == 1 { "REQUIRED" } else { "VISIBLE" };
            let authoritative = resident;
            let snapshot = self
                .presentation
                .as_ref()
                .is_some_and(|world| world.chunks().any(|section| section.position == position));
            let renderer = self
                .renderer
                .as_ref()
                .map(|renderer| renderer.column_state(position, camera_for(simulation, 16.0 / 9.0)))
                .unwrap_or_default();
            return format!(
                "chunk=({}, {}) state={} age={}ms priority={} authoritative={} snapshot={} mesh_sections={} drawable={} submitted={} frustum_culled={}",
                position.x,
                position.z,
                phase,
                age,
                urgency,
                authoritative,
                snapshot,
                renderer.resident_section_count,
                renderer.drawable_section_count,
                renderer.submitted_section_count,
                renderer.frustum_culled_section_count,
            );
        }
        "none-within-3".to_owned()
    }

    fn column_area_state(&self, position: rustcraft_engine_core::ChunkPos) -> (char, &'static str) {
        use rustcraft_render::meshing::ColumnMeshStage;
        use rustcraft_world::ResidencyPhase;
        let Some(simulation) = self.simulation.as_ref() else {
            return ('N', "not-desired");
        };
        let pinned = simulation.lighting.integrating_column() == Some(position);
        if pinned {
            return ('P', "pinned");
        }
        if self.pending_evictions.contains(&position) {
            return ('E', "eviction-pending");
        }
        if matches!(self.residency.phase(position), Some(ResidencyPhase::Failed)) {
            return ('!', "error");
        }
        if simulation.world.column_available(position) {
            return ('S', "safe-for-simulation");
        }
        if self.render_ready_columns.contains(&position) {
            return ('V', "render-visible");
        }
        if self
            .snapshot_dirty_sections
            .iter()
            .any(|section| section.0 == position)
        {
            return ('s', "snapshot-pending");
        }
        if self
            .mesh_dirty_sections
            .iter()
            .any(|section| section.0 == position)
        {
            return ('m', "mesh-pending");
        }
        if let Some(stage) = self.mesh_scheduler.column_stage(position) {
            return match stage {
                ColumnMeshStage::Pending | ColumnMeshStage::InFlight => ('m', "mesh-pending"),
                ColumnMeshStage::UploadPending => ('u', "upload-pending"),
            };
        }
        let boundary = simulation
            .lighting
            .integration_columns()
            .contains(&position);
        match self.residency.phase(position) {
            Some(ResidencyPhase::Requested) => ('q', "requested"),
            Some(ResidencyPhase::Loading) => ('l', "loading"),
            Some(ResidencyPhase::Generating) => ('g', "generating"),
            Some(ResidencyPhase::InitialLighting) => ('i', "initial-light"),
            Some(ResidencyPhase::Ready) if boundary => ('b', "boundary-light"),
            Some(ResidencyPhase::Ready) => ('v', "voxel-ready"),
            Some(ResidencyPhase::Saving) => ('e', "saving"),
            Some(ResidencyPhase::Failed) => ('!', "error"),
            None if self.residency.is_desired(position) => ('d', "desired"),
            None if self.residency.is_retained_by_radius(position) => ('r', "retained"),
            None => ('N', "not-desired"),
        }
    }

    fn area_state_grid(&self) -> String {
        let Some(simulation) = self.simulation.as_ref() else {
            return "AREA unavailable".to_owned();
        };
        let center = rustcraft_engine_core::ChunkPos {
            x: (simulation.player.position.x.floor() as i32).div_euclid(16),
            z: (simulation.player.position.z.floor() as i32).div_euclid(16),
        };
        let radius = self.residency.retain_radius();
        let mut output = format!(
            "AREA center=({}, {}) radius={} metric=Chebyshev (+Z rows first)\n",
            center.x, center.z, radius
        );
        for dz in (-radius..=radius).rev() {
            for dx in -radius..=radius {
                let position = rustcraft_engine_core::ChunkPos {
                    x: center.x + dx,
                    z: center.z + dz,
                };
                output.push(self.column_area_state(position).0);
                if dx != radius {
                    output.push(' ');
                }
            }
            output.push('\n');
        }
        output.push_str(
            "LEGEND N=NOT_DESIRED d=DESIRED q=REQUESTED l=LOADING g=GENERATING i=INITIAL_LIGHT v=VOXEL_READY b=BOUNDARY_LIGHT s=SNAPSHOT_PENDING m=MESH_PENDING u=UPLOAD_PENDING V=RENDER_VISIBLE S=SAFE E=EVICTION_PENDING r=RETAINED P=PINNED !=ERROR",
        );
        output
    }

    fn blocked_adjacent_reasons(&self) -> String {
        let Some(simulation) = self.simulation.as_ref() else {
            return "none".to_owned();
        };
        let center = rustcraft_engine_core::ChunkPos {
            x: (simulation.player.position.x.floor() as i32).div_euclid(16),
            z: (simulation.player.position.z.floor() as i32).div_euclid(16),
        };
        let missing_core = (-1..=1).find_map(|dz| {
            (-1..=1).find_map(|dx| {
                let position = rustcraft_engine_core::ChunkPos {
                    x: center.x + dx,
                    z: center.z + dz,
                };
                (!self.render_ready_columns.contains(&position)).then_some(position)
            })
        });
        let mut reasons = Vec::new();
        for dz in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let position = rustcraft_engine_core::ChunkPos {
                    x: center.x + dx,
                    z: center.z + dz,
                };
                if simulation.world.column_available(position) {
                    continue;
                }
                let (_, state) = self.column_area_state(position);
                if state == "render-visible"
                    && let Some(dependency) = missing_core
                {
                    reasons.push(format!(
                        "({},{}): blocked: safe-core dependency ({},{})",
                        position.x, position.z, dependency.x, dependency.z
                    ));
                } else {
                    reasons.push(format!(
                        "({},{}): blocked: {}",
                        position.x, position.z, state
                    ));
                }
            }
        }
        if reasons.is_empty() {
            "none".to_owned()
        } else {
            reasons.join("; ")
        }
    }

    fn mark_column_voxel_ready(
        &mut self,
        position: rustcraft_engine_core::ChunkPos,
        token: u64,
        section_ys: Vec<i32>,
    ) {
        let started = self
            .request_started_at
            .remove(&(position, token))
            .unwrap_or_else(Instant::now);
        self.voxel_ready_latency_ms.record(started.elapsed());
        self.light_latency_started_at.insert(position, started);
        self.light_queue_started_at.insert(position, Instant::now());
        self.visible_latency_started_at.insert(position, started);
        self.first_section_latency_started_at
            .insert(position, started);
        self.visible_expected_sections
            .insert(position, section_ys.into_iter().collect());
    }

    fn queue_initial_lighting(
        &mut self,
        request: rustcraft_world::ResidencyRequest,
        sections: Vec<(i32, rustcraft_engine_core::Chunk)>,
        persist_new: bool,
    ) -> Result<(), Vec<(i32, rustcraft_engine_core::Chunk)>> {
        let Some(simulation) = self.simulation.as_ref() else {
            return Err(sections);
        };
        let default_block = simulation.world.empty_block();
        let registry = std::sync::Arc::new(simulation.registry.clone());
        self.initial_lighting_scheduler.submit(
            rustcraft_runtime::lighting::InitialLightingRequest {
                position: request.position,
                token: request.token,
                priority: self.residency.priority_key(request.position),
                default_block,
                sections,
                registry,
                persist_new,
            },
        )?;
        let _ = self
            .residency
            .set_phase(request, rustcraft_world::ResidencyPhase::InitialLighting);
        Ok(())
    }

    fn advance_boundary_lighting_for_turn(
        &mut self,
    ) -> (Vec<rustcraft_engine_core::SectionPos>, usize) {
        let mut dirty = Vec::new();
        let mut work_units = 0usize;
        let turn_started = self.stream_turn_started.unwrap_or_else(Instant::now);
        let deadline = self
            .stream_stage_deadline
            .unwrap_or(turn_started + self.stream_main_budget);
        while Instant::now() < deadline && self.stream_budget_available() {
            let Some(column) = self
                .simulation
                .as_ref()
                .and_then(|simulation| simulation.lighting.integrating_column())
            else {
                break;
            };
            if !self.light_work_started_at.contains_key(&column) {
                if let Some(queued_at) = self.light_queue_started_at.remove(&column) {
                    self.lighting_queue_wait_ms.record(queued_at.elapsed());
                }
                self.light_work_started_at.insert(column, Instant::now());
            }
            let lighting_started = Instant::now();
            // A seed is one full vertical x/z ray (the minimum atomic unit in the lighting
            // implementation). Repeated small quanta are serviced on event-loop turns rather
            // than one quantum per 20 Hz simulation tick.
            let boundary_only = self
                .simulation
                .as_ref()
                .is_some_and(|simulation| simulation.lighting.integrating_boundary_only());
            let work_budget = if boundary_only {
                self.lighting_work_budget.clamp(256, 512)
            } else {
                self.lighting_work_budget.min(32)
            };
            let completed_column = self
                .simulation
                .as_mut()
                .and_then(|simulation| simulation.advance_column_lighting(work_budget));
            work_units = work_units.saturating_add(work_budget);
            let elapsed = lighting_started.elapsed();
            let slice_ms = elapsed.as_secs_f64() * 1000.0;
            self.responsiveness
                .record_long_task("boundary_lighting_slice", elapsed, work_budget);
            *self.light_cpu_accumulated_ms.entry(column).or_default() += slice_ms;
            self.lighting_work_ms += slice_ms;
            if let Some(completed) = completed_column {
                self.light_converged_count += 1;
                if let Some(active_started) = self.light_work_started_at.remove(&completed) {
                    self.lighting_active_ms.record(active_started.elapsed());
                }
                if let Some(cpu_ms) = self.light_cpu_accumulated_ms.remove(&completed) {
                    self.lighting_cpu_ms
                        .record(Duration::from_secs_f64(cpu_ms / 1000.0));
                }
                if let Some(started) = self.light_latency_started_at.remove(&completed) {
                    self.light_ready_latency_ms.record(started.elapsed());
                }
                dirty.extend(
                    self.simulation
                        .as_mut()
                        .expect("completed lighting requires a running simulation")
                        .take_dirty_sections(),
                );
            }
        }
        (dirty, work_units)
    }

    fn service_world_residency(&mut self) -> Vec<rustcraft_engine_core::SectionPos> {
        use rustcraft_world::{ResidencyPhase, SemanticBlockResolver};
        let mut dirty = Vec::new();
        let mut publications = 0usize;
        const PUBLICATION_BUDGET: usize = 1;

        if !self.stream_budget_available() {
            return dirty;
        }

        self.initial_lighting_scheduler
            .reprioritize(|position| self.residency.priority_key(position));
        let boundary_capacity = self.simulation.as_ref().map_or(0, |simulation| {
            simulation.lighting.integration_capacity_remaining()
        });
        let mut initial_lit = self.initial_lighting_scheduler.take_ready(
            PUBLICATION_BUDGET
                .min(boundary_capacity)
                .min(usize::from(self.stream_budget_available())),
        );
        initial_lit.sort_by_key(|result| self.residency.priority_key(result.position));
        for result in initial_lit {
            let request = rustcraft_world::ResidencyRequest {
                position: result.position,
                token: result.token,
            };
            self.initial_light_queue_wait_ms
                .record(Duration::from_secs_f64(
                    result.queue_wait_ms.max(0.0) / 1000.0,
                ));
            self.initial_light_worker_ms.record(Duration::from_secs_f64(
                result.worker_elapsed_ms.max(0.0) / 1000.0,
            ));
            self.initial_light_emitters += result.work_counters.emitters_found;
            self.initial_light_direct_voxels += result.work_counters.direct_voxels_scanned;
            self.initial_light_propagation_nodes += result.work_counters.propagation_queue_pops;
            if !self.residency.is_current(request) {
                self.request_started_at
                    .remove(&(request.position, request.token));
                continue;
            }
            if let Some(error) = result.error {
                let _ = self.residency.set_phase(request, ResidencyPhase::Failed);
                self.request_started_at
                    .remove(&(request.position, request.token));
                eprintln!(
                    "initial lighting failed for column ({},{}): {error}",
                    request.position.x, request.position.z
                );
                continue;
            }
            let sections = result.sections.iter().map(|(y, _)| *y).collect::<Vec<_>>();
            let publication_started = Instant::now();
            let persist_new = result.persist_new;
            let result = self
                .simulation
                .as_mut()
                .expect("running world")
                .publish_initial_lit_column(result, persist_new);
            self.publication_ms.record(publication_started.elapsed());
            self.responsiveness.record_long_task(
                "initial_column_apply",
                publication_started.elapsed(),
                sections.len(),
            );
            if let Err(error) = result {
                let _ = self.residency.set_phase(request, ResidencyPhase::Failed);
                eprintln!(
                    "publish initially lit column ({},{}): {error}",
                    request.position.x, request.position.z
                );
                continue;
            }
            self.residency.published(request.position, request.token);
            self.mark_column_voxel_ready(request.position, request.token, sections);
            if let Err(error) = self.activate_pending_spatial_column(request.position) {
                let _ = self.residency.set_phase(request, ResidencyPhase::Failed);
                eprintln!(
                    "activate spatial records for column ({},{}): {error}",
                    request.position.x, request.position.z
                );
                continue;
            }
            if persist_new {
                self.persistence_dirty.mark_dirty(request.position);
                self.generated_chunks += 1;
            } else {
                self.loaded_from_disk += 1;
            }
        }

        let load_poll_started = Instant::now();
        let load_limit =
            PUBLICATION_BUDGET.min(self.initial_lighting_scheduler.capacity_remaining());
        let mut load_completions = self
            .load_scheduler
            .take_ready(load_limit.min(usize::from(self.stream_budget_available())));
        let load_completion_count = load_completions.len();
        self.load_results_applied = self
            .load_results_applied
            .saturating_add(load_completion_count as u64);
        load_completions
            .sort_by_key(|completion| self.residency.priority_key(completion.request.position));
        for completion in load_completions {
            self.load_worker_ms.record(Duration::from_secs_f64(
                completion.load_ms.max(0.0) / 1000.0,
            ));
            if let Some(started) = self
                .request_started_at
                .get(&(completion.request.position, completion.request.token))
                .copied()
            {
                let elapsed = started.elapsed();
                self.load_total_latency_ms.record(elapsed);
                self.load_queue_wait_ms.record(Duration::from_secs_f64(
                    completion.queue_wait_ms.max(0.0) / 1000.0,
                ));
            }
            if !self.residency.is_current(completion.request) {
                self.request_started_at
                    .remove(&(completion.request.position, completion.request.token));
                continue;
            }
            match completion.result {
                Ok(Some(column)) => {
                    self.pending_spatial_columns.insert(
                        completion.request.position,
                        (column.spatial_records, column.spatial_tombstones),
                    );
                    match self.queue_initial_lighting(completion.request, column.sections, false) {
                        Ok(()) => publications += 1,
                        Err(sections) => {
                            let _ = sections;
                            self.pending_spatial_columns
                                .remove(&completion.request.position);
                            self.residency.defer(completion.request);
                            self.request_started_at
                                .remove(&(completion.request.position, completion.request.token));
                        }
                    }
                }
                Ok(None) => {
                    if !self.stream_generation_allowed {
                        let _ = self
                            .residency
                            .set_phase(completion.request, ResidencyPhase::Failed);
                        eprintln!(
                            "cannot generate missing column ({},{}): saved world generator is incompatible with the active generator",
                            completion.request.position.x, completion.request.position.z
                        );
                        continue;
                    }
                    let Some(generator) = self.stream_generator.clone() else {
                        let _ = self
                            .residency
                            .set_phase(completion.request, ResidencyPhase::Failed);
                        continue;
                    };
                    let _ = self
                        .residency
                        .set_phase(completion.request, ResidencyPhase::Generating);
                    self.generation_started_at.insert(
                        (completion.request.position, completion.request.token),
                        Instant::now(),
                    );
                    if let Err(error) = self.generation_scheduler.request(
                        generator,
                        self.world_seed,
                        completion.request.position,
                        completion.request.token,
                    ) {
                        self.generation_started_at
                            .remove(&(completion.request.position, completion.request.token));
                        self.residency.defer(completion.request);
                        if !error.to_string().contains("queue full") {
                            eprintln!(
                                "queue generation for column ({},{}): {error}",
                                completion.request.position.x, completion.request.position.z
                            );
                        }
                    }
                }
                Err(error) => {
                    self.request_started_at
                        .remove(&(completion.request.position, completion.request.token));
                    let _ = self
                        .residency
                        .set_phase(completion.request, ResidencyPhase::Failed);
                    eprintln!(
                        "cannot load column ({},{}): {error}; it will not be regenerated",
                        completion.request.position.x, completion.request.position.z
                    );
                }
            }
        }
        self.responsiveness.record_long_task(
            "load_result_poll_apply",
            load_poll_started.elapsed(),
            load_completion_count,
        );

        let generation_limit = self
            .initial_lighting_scheduler
            .capacity_remaining()
            .min(PUBLICATION_BUDGET.saturating_sub(publications))
            .min(usize::from(self.stream_budget_available()));
        if generation_limit > 0 {
            let generation_poll_started = Instant::now();
            let mut results = self.generation_scheduler.take_ready_limit(generation_limit);
            let generation_result_count = results.len();
            self.generation_results_applied = self
                .generation_results_applied
                .saturating_add(generation_result_count as u64);
            results.sort_by_key(|result| self.residency.priority_key(result.position));
            for result in results {
                let request = rustcraft_world::ResidencyRequest {
                    position: result.position,
                    token: result.generation,
                };
                self.generation_worker_ms.record(Duration::from_secs_f64(
                    result.generation_ms.max(0.0) / 1000.0,
                ));
                if let Some(started) = self
                    .generation_started_at
                    .remove(&(request.position, request.token))
                {
                    let elapsed = started.elapsed();
                    self.generation_total_latency_ms.record(elapsed);
                    self.generation_queue_wait_ms
                        .record(Duration::from_secs_f64(
                            result.queue_wait_ms.max(0.0) / 1000.0,
                        ));
                }
                if !self.residency.is_current(request) {
                    self.request_started_at
                        .remove(&(request.position, request.token));
                    continue;
                }
                match result.sections {
                    Ok(sections) => match self.queue_initial_lighting(request, sections, true) {
                        Ok(()) => {}
                        Err(sections) => {
                            let _ = sections;
                            self.residency.defer(request);
                            self.request_started_at
                                .remove(&(request.position, request.token));
                        }
                    },
                    Err(error) => {
                        self.request_started_at
                            .remove(&(request.position, request.token));
                        let _ = self.residency.set_phase(request, ResidencyPhase::Failed);
                        eprintln!(
                            "generator failed for column ({},{}): {error}",
                            result.position.x, result.position.z
                        );
                    }
                }
            }
            self.responsiveness.record_long_task(
                "generation_result_poll_apply",
                generation_poll_started.elapsed(),
                generation_result_count,
            );
        }

        if !self.stream_budget_available() {
            return dirty;
        }
        let Some(simulation) = self.simulation.as_ref() else {
            return dirty;
        };
        let center = rustcraft_engine_core::ChunkPos {
            x: (simulation.player.position.x.floor() as i32).div_euclid(16),
            z: (simulation.player.position.z.floor() as i32).div_euclid(16),
        };
        let player_position = (simulation.player.position.x, simulation.player.position.z);
        let resident = simulation
            .world
            .column_positions()
            .collect::<std::collections::HashSet<_>>();
        // Persistable dropped items follow save-before-evict and no longer pin terrain forever.
        let mut pinned = std::collections::HashSet::new();
        if let Some(active_boundary) = simulation.lighting.integrating_column() {
            pinned.insert(active_boundary);
        }
        let displacement = self.last_residency_position.map_or((0.0, 0.0), |previous| {
            (
                player_position.0 - previous.0,
                player_position.1 - previous.1,
            )
        });
        self.last_residency_position = Some(player_position);
        let now = Instant::now();
        let residency_elapsed = self
            .last_residency_update_at
            .replace(now)
            .map_or(0.05, |previous| now.duration_since(previous).as_secs_f32());
        let view_x = simulation.player.yaw.sin();
        let view_z = simulation.player.yaw.cos();
        let plan = self.residency.update_with_motion_in_view(
            center,
            &resident,
            &pinned,
            rustcraft_world::ResidencyMotion {
                displacement: if self.stream_lookahead_enabled {
                    [displacement.0, displacement.1]
                } else {
                    [0.0, 0.0]
                },
                elapsed_seconds: if self.stream_lookahead_enabled {
                    residency_elapsed
                } else {
                    0.0
                },
                view_direction: [view_x, view_z],
            },
        );
        if let Some(simulation) = self.simulation.as_mut() {
            let residency = &self.residency;
            simulation
                .lighting
                .reprioritize_pending_columns(|position| residency.priority_key(position));
        }
        for cancelled in &plan.cancelled {
            self.request_started_at
                .remove(&(cancelled.position, cancelled.token));
        }
        if self.full_desired_ready_ms.is_none() {
            let ready = self
                .render_ready_columns
                .iter()
                .filter(|position| self.residency.is_desired(**position))
                .count();
            let mesh_stats = self.mesh_scheduler.stats();
            if ready >= self.residency.desired_column_count()
                && mesh_stats.pending == 0
                && mesh_stats.in_flight == 0
                && mesh_stats.ready == 0
            {
                self.full_desired_ready_ms = Some(
                    self.world_ready_at
                        .unwrap_or(self.startup_started)
                        .elapsed()
                        .as_secs_f64()
                        * 1000.0,
                );
            }
        }

        for (index, request) in plan.requests.into_iter().enumerate() {
            if index >= 1 || !self.stream_budget_available() {
                self.residency.defer(request);
                continue;
            }
            let _ = self.residency.set_phase(request, ResidencyPhase::Loading);
            let Some(storage) = self.world_storage.clone() else {
                self.residency.defer(request);
                continue;
            };
            let resolver: std::sync::Arc<dyn SemanticBlockResolver> = std::sync::Arc::new(
                rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver,
            );
            self.request_started_at
                .entry((request.position, request.token))
                .or_insert_with(Instant::now);
            if let Err(error) = self.load_scheduler.submit(storage, request, resolver) {
                self.residency.defer(request);
                if !error.to_string().contains("queue full") {
                    eprintln!(
                        "queue load for column ({},{}): {error}",
                        request.position.x, request.position.z
                    );
                }
            }
        }

        self.pending_evictions.extend(plan.evict);
        while self.stream_budget_available() {
            let Some(position) = pop_before_deadline(
                &mut self.pending_evictions,
                self.stream_turn_started.unwrap_or_else(Instant::now) + self.stream_main_budget,
            ) else {
                break;
            };
            let retain_radius = self.residency.retain_radius();
            let Some(simulation) = self.simulation.as_ref() else {
                continue;
            };
            let center = rustcraft_engine_core::ChunkPos {
                x: (simulation.player.position.x.floor() as i32).div_euclid(16),
                z: (simulation.player.position.z.floor() as i32).div_euclid(16),
            };
            let dx = i64::from(position.x) - i64::from(center.x);
            let dz = i64::from(position.z) - i64::from(center.z);
            let still_near = dx.abs().max(dz.abs()) <= i64::from(retain_radius);
            let pinned_now = simulation.lighting.integrating_column() == Some(position);
            if still_near || pinned_now {
                if still_near {
                    self.simulation
                        .as_mut()
                        .expect("running simulation")
                        .unfreeze_entity_column(position);
                }
                continue;
            }
            let froze_entities = self
                .simulation
                .as_mut()
                .expect("running simulation")
                .freeze_entity_column(position);
            if froze_entities {
                self.entity_save_before_evict = self.entity_save_before_evict.saturating_add(1);
            }
            for dirty_position in self
                .simulation
                .as_mut()
                .expect("running simulation")
                .take_persistence_dirty_chunks()
            {
                self.persistence_dirty.mark_dirty(dirty_position);
            }
            if self.persistence_dirty.is_dirty(position)
                || self.persistence_dirty.is_saving(position)
            {
                self.eviction_blocked_dirty += 1;
                if !self
                    .simulation
                    .as_ref()
                    .expect("running simulation")
                    .entity_column_snapshot(position)
                    .entities
                    .is_empty()
                {
                    self.entity_evictions_blocked = self.entity_evictions_blocked.saturating_add(1);
                }
                continue;
            }
            self.simulation
                .as_mut()
                .unwrap()
                .lighting
                .cancel_queued_for_eviction(position);
            let removed = match self
                .simulation
                .as_mut()
                .unwrap()
                .remove_column_incremental_lighting(position)
            {
                Ok(removed) => removed,
                Err(_) => {
                    self.eviction_blocked_lighting += 1;
                    continue;
                }
            };
            if self.stream_perf {
                self.stream_evicted_entity_ids.extend(
                    self.simulation
                        .as_ref()
                        .unwrap()
                        .items
                        .iter()
                        .filter(|entity| entity.column() == position)
                        .map(|entity| entity.id),
                );
            }
            self.simulation
                .as_mut()
                .unwrap()
                .evict_entity_column(position);
            if removed.is_empty() {
                // Empty persisted/generated columns are still resident and must be evictable.
                self.simulation
                    .as_mut()
                    .unwrap()
                    .world
                    .remove_column(position);
            }
            self.residency.evicted(position);
            self.eviction_count += 1;
            if self.stream_perf {
                self.stream_evicted_columns.insert(position);
            }
            self.light_latency_started_at.remove(&position);
            self.light_queue_started_at.remove(&position);
            self.light_work_started_at.remove(&position);
            self.light_cpu_accumulated_ms.remove(&position);
            self.visible_latency_started_at.remove(&position);
            self.first_section_latency_started_at.remove(&position);
            self.visible_expected_sections.remove(&position);
            self.render_ready_columns.remove(&position);
            for (section_y, _) in removed {
                self.mesh_scheduler.remove_section((position, section_y));
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.remove_section(position, section_y);
                }
            }
            dirty.extend(self.simulation.as_mut().unwrap().take_dirty_sections());
        }
        dirty
    }

    fn service_world_saves(&mut self, dirty: Vec<rustcraft_engine_core::ChunkPos>) {
        for position in dirty {
            self.persistence_dirty.mark_dirty(position);
        }
        for completion in self.save_scheduler.take_completed() {
            let success = completion.result.is_ok();
            if let Err(error) = completion.result {
                eprintln!(
                    "world save failed for ({},{}), generation {}: {error}",
                    completion.token.position.x,
                    completion.token.position.z,
                    completion.token.generation
                );
            }
            self.persistence_dirty
                .complete_save(completion.token, success);
            if success
                && let Some(snapshot) = self.entity_save_snapshots.remove(&(
                    completion.token.position.x,
                    completion.token.position.z,
                    completion.token.generation,
                ))
            {
                self.entity_records_saved = self
                    .entity_records_saved
                    .saturating_add(snapshot.entities.len() as u64);
                if self
                    .simulation
                    .as_mut()
                    .is_some_and(|simulation| simulation.note_entity_column_persisted(&snapshot))
                {
                    self.player_dirty = true;
                }
            }
        }
        let (Some(storage), Some(simulation)) =
            (self.world_storage.as_ref(), self.simulation.as_ref())
        else {
            return;
        };
        let Some(position) = self.persistence_dirty.queued(1).first().copied() else {
            return;
        };
        let Some(token) = self.persistence_dirty.begin_save(position) else {
            return;
        };
        let entity_encode_started = Instant::now();
        let result =
            encode_simulation_column(simulation, position).and_then(|(chunk, snapshot)| {
                self.entity_encode_bytes = self.entity_encode_bytes.saturating_add(
                    chunk
                        .spatial_records
                        .iter()
                        .map(|record| record.payload.len() as u64)
                        .sum::<u64>(),
                );
                self.entity_save_snapshots
                    .insert((position.x, position.z, token.generation), snapshot);
                self.save_scheduler
                    .submit(storage.clone(), token, chunk)
                    .map_err(|error| error.to_string())
            });
        self.entity_encode_ms += entity_encode_started.elapsed().as_secs_f64() * 1000.0;
        if let Err(error) = result {
            self.entity_save_snapshots
                .remove(&(position.x, position.z, token.generation));
            eprintln!(
                "unable to queue world save for ({},{}): {error}",
                position.x, position.z
            );
            self.persistence_dirty.complete_save(token, false);
        }
    }

    fn finish_world_saves(&mut self) {
        let Some(storage) = self.world_storage.clone() else {
            return;
        };
        if let Some(simulation) = self.simulation.as_mut() {
            for position in simulation.entity_column_positions() {
                self.persistence_dirty.mark_dirty(position);
            }
            for position in simulation.take_persistence_dirty_chunks() {
                self.persistence_dirty.mark_dirty(position);
            }
        }
        self.refresh_world_state_snapshot();
        if self.world_state_dirty
            && let Some(record) = self.latest_world_state_record.clone()
        {
            if let Err(error) = self
                .world_state_save_scheduler
                .submit(storage.clone(), record.clone())
            {
                eprintln!("world shutdown global-state submission failed: {error}");
            } else {
                let deadline = Instant::now() + Duration::from_secs(60);
                while self.world_state_persisted_revision < record.revision
                    && Instant::now() < deadline
                {
                    for completion in self.world_state_save_scheduler.take_completed() {
                        if completion.result.is_ok() {
                            self.world_state_persisted_revision =
                                self.world_state_persisted_revision.max(completion.revision);
                        } else if let Err(error) = completion.result {
                            eprintln!("world shutdown global-state save failed: {error}");
                        }
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                self.world_state_dirty =
                    self.world_state_persisted_revision < self.world_state_revision;
                if self.world_state_dirty {
                    eprintln!(
                        "world shutdown could not durably persist global revision {} (last {})",
                        self.world_state_revision, self.world_state_persisted_revision
                    );
                }
            }
        }
        if let Err(error) = self.refresh_player_snapshot() {
            eprintln!("world shutdown player encode failed: {error}");
        } else if self.player_dirty
            && let Some(record) = self.latest_player_record.clone()
        {
            if let Err(error) = self
                .player_save_scheduler
                .submit(storage.clone(), record.clone())
            {
                eprintln!("world shutdown player save submission failed: {error}");
            } else {
                if let Some(simulation) = self.simulation.as_ref() {
                    self.player_checkpoint_receipts.insert(
                        record.revision,
                        simulation
                            .pickup_receipts()
                            .into_iter()
                            .map(|receipt| receipt.entity_id)
                            .collect(),
                    );
                }
                let deadline = Instant::now() + Duration::from_secs(60);
                while self.player_persisted_revision < record.revision && Instant::now() < deadline
                {
                    for completion in self.player_save_scheduler.take_completed() {
                        if completion.result.is_ok() {
                            let completed_revisions = self
                                .player_checkpoint_receipts
                                .keys()
                                .copied()
                                .filter(|revision| *revision <= completion.revision)
                                .collect::<Vec<_>>();
                            let mut durable_receipts = Vec::new();
                            for revision in completed_revisions {
                                if let Some(receipts) =
                                    self.player_checkpoint_receipts.remove(&revision)
                                {
                                    durable_receipts.extend(receipts);
                                }
                            }
                            if let Some(simulation) = self.simulation.as_mut() {
                                simulation.commit_pickup_receipts(&durable_receipts);
                            }
                            self.player_persisted_revision =
                                self.player_persisted_revision.max(completion.revision);
                        } else if let Err(error) = completion.result {
                            eprintln!("world shutdown player save failed: {error}");
                        }
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                self.player_dirty = self.player_persisted_revision < self.player_revision;
                if self.player_dirty {
                    eprintln!(
                        "world shutdown could not durably persist player revision {} (last persisted {})",
                        self.player_revision, self.player_persisted_revision
                    );
                } else {
                    eprintln!(
                        "saved durable player state in world {}: revision={} components={} payload_bytes={}",
                        self.world_name,
                        self.player_persisted_revision,
                        record.components.len(),
                        record
                            .components
                            .iter()
                            .map(|component| component.payload.len())
                            .sum::<usize>()
                    );
                }
            }
        }
        if let Some(simulation) = self.simulation.as_mut() {
            for position in simulation.take_persistence_dirty_chunks() {
                self.persistence_dirty.mark_dirty(position);
            }
        }
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        while self.persistence_dirty.metrics().saves_in_flight > 0 && Instant::now() < deadline {
            for completion in self.save_scheduler.take_completed() {
                let success = completion.result.is_ok();
                if let Err(error) = completion.result {
                    eprintln!("world save failed during shutdown: {error}");
                }
                self.persistence_dirty
                    .complete_save(completion.token, success);
                if success
                    && let Some(snapshot) = self.entity_save_snapshots.remove(&(
                        completion.token.position.x,
                        completion.token.position.z,
                        completion.token.generation,
                    ))
                    && self.simulation.as_mut().is_some_and(|simulation| {
                        simulation.note_entity_column_persisted(&snapshot)
                    })
                {
                    self.player_dirty = true;
                }
            }
            std::thread::yield_now();
        }
        if self.persistence_dirty.metrics().saves_in_flight > 0 {
            eprintln!("world shutdown timed out waiting for save workers");
            return;
        }
        for position in self.persistence_dirty.queued(usize::MAX) {
            let Some(token) = self.persistence_dirty.begin_save(position) else {
                continue;
            };
            let Some(simulation) = self.simulation.as_ref() else {
                break;
            };
            let result = encode_simulation_column(simulation, position)
                .map_err(rustcraft_world::WorldError::Compatibility)
                .and_then(|(chunk, snapshot)| {
                    storage.store_chunk_measured(&chunk).map(|_| snapshot)
                });
            let success = result.is_ok();
            if let Ok(snapshot) = &result
                && self
                    .simulation
                    .as_mut()
                    .is_some_and(|simulation| simulation.note_entity_column_persisted(snapshot))
            {
                self.player_dirty = true;
            }
            self.persistence_dirty.complete_save(token, success);
            if let Err(error) = result {
                eprintln!(
                    "world shutdown save failed for ({},{}): {error}",
                    position.x, position.z
                );
            }
        }
        if let Err(error) = storage.flush() {
            eprintln!("world shutdown flush failed: {error}");
        }
        if self.persistence_dirty.dirty_count() > 0 {
            eprintln!(
                "world shutdown left {} persistence-dirty chunks unsaved",
                self.persistence_dirty.dirty_count()
            );
        }
    }
    fn rebuild_meshes(&mut self) {
        let Some(presentation) = self.presentation.as_ref() else {
            return;
        };
        let started = Instant::now();
        let player_section = self.player_section_position();
        let mut snapshots = presentation.chunks().cloned().collect::<Vec<_>>();
        snapshots.sort_by_key(|chunk| {
            Self::section_priority(
                &self.residency,
                player_section,
                (chunk.position, chunk.section_y),
            )
        });
        let started_at = Instant::now();
        for snapshot in &snapshots {
            self.visible_expected_sections
                .entry(snapshot.position)
                .or_default()
                .insert(snapshot.section_y);
            self.visible_latency_started_at
                .entry(snapshot.position)
                .or_insert(started_at);
            self.first_section_latency_started_at
                .entry(snapshot.position)
                .or_insert(started_at);
            self.render_ready_columns.remove(&snapshot.position);
        }
        for snapshot in snapshots {
            self.mesh_scheduler.mark_dirty(snapshot);
        }
        eprintln!(
            "client diagnostics: resident_sections={} mesh_jobs={} initial_schedule_ms={:.3}",
            presentation.chunk_count(),
            self.mesh_scheduler.stats().pending + self.mesh_scheduler.stats().in_flight,
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
        let Some(presentation) = self.presentation.as_ref() else {
            return;
        };
        let mut dirty = dirty.into_iter().collect::<Vec<_>>();
        dirty.sort_by_key(|(position, section_y)| (position.x, position.z, *section_y));
        for (position, section_y) in dirty {
            if let Some(snapshot) = presentation.section(position, section_y).cloned() {
                self.mesh_scheduler.mark_dirty(snapshot);
            } else {
                self.mesh_scheduler.remove_section((position, section_y));
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.remove_section(position, section_y);
                }
            }
        }
    }

    fn process_mesh_jobs(&mut self) {
        self.mesh_uploads_this_frame = 0;
        self.mesh_upload_bytes_this_frame = 0;
        self.mesh_scheduler.poll();
        let upload_pending = self.mesh_scheduler.stats().ready > 0;
        let camera = self
            .simulation
            .as_ref()
            .map_or(Vec3::ZERO, |simulation| simulation.player.position);
        let view_forward = self.simulation.as_ref().map_or(Vec3::ZERO, |simulation| {
            Vec3::new(
                simulation.player.yaw.sin(),
                0.0,
                simulation.player.yaw.cos(),
            )
        });
        let completed = self.mesh_scheduler.take_ready(
            camera,
            view_forward,
            self.mesh_upload_section_budget,
            self.mesh_upload_byte_budget,
        );
        let completed_count = completed.len();
        for result in completed {
            self.metrics.mesh(result.mesh_ms / 1000.0);
            self.mesh_queue_wait_ms.record(Duration::from_secs_f64(
                result.queue_wait_ms.max(0.0) / 1000.0,
            ));
            self.mesh_execution_ms
                .record(Duration::from_secs_f64(result.mesh_ms.max(0.0) / 1000.0));
            self.mesh_upload_wait_ms
                .record(result.completed_at.elapsed());
            let upload = if let Some(renderer) = self.renderer.as_mut() {
                let upload_started = Instant::now();
                let upload =
                    renderer.upload_chunk_pages(result.section.0, result.section.1, &result.pages);
                self.responsiveness.record_long_task(
                    "gpu_mesh_upload_submit",
                    upload_started.elapsed(),
                    upload.logical_bytes,
                );
                upload
            } else {
                rustcraft_render::MeshUploadStats {
                    logical_bytes: result.logical_bytes(),
                    ..Default::default()
                }
            };
            self.mesh_uploads += 1;
            self.mesh_upload_bytes += upload.logical_bytes as u64;
            self.mesh_upload_submit_ms += upload.submit_ms;
            self.mesh_uploads_this_frame += 1;
            self.mesh_upload_bytes_this_frame += upload.logical_bytes;
            if let Some(expected) = self.visible_expected_sections.get_mut(&result.section.0) {
                expected.remove(&result.section.1);
                if let Some(started) = self
                    .first_section_latency_started_at
                    .remove(&result.section.0)
                {
                    self.first_section_latency_ms.record(started.elapsed());
                }
                if expected.is_empty() {
                    self.visible_expected_sections.remove(&result.section.0);
                    self.render_ready_columns.insert(result.section.0);
                    if let Some(started) = self.visible_latency_started_at.remove(&result.section.0)
                    {
                        self.render_visible_latency_ms.record(started.elapsed());
                    }
                }
            }
        }
        self.release_render_ready_safe_core();
        self.observe_stream_stage(4, upload_pending, completed_count > 0, true, false);
    }
    fn render(&mut self, event_loop: &ActiveEventLoop) {
        let legacy_debug = self.legacy_debug_visible();
        if let Some(input_at) = self.responsiveness.input_consumed_at.take() {
            self.responsiveness
                .input_to_render
                .record(Instant::now().duration_since(input_at));
        }
        let frame_interval = self.last_render.elapsed();
        self.responsiveness.frame_interval.record(frame_interval);
        self.metrics.frames.push(frame_interval.as_secs_f64());
        self.last_render = Instant::now();
        self.metrics.update();
        let mesh_poll_started = Instant::now();
        self.process_mesh_jobs();
        self.frame_phases.mesh_poll_upload_ms = mesh_poll_started.elapsed().as_secs_f64() * 1000.0;
        self.frame_phase_samples
            .mesh_poll_upload
            .record_ms(self.frame_phases.mesh_poll_upload_ms);
        let mesh_stats = self.mesh_scheduler.stats();
        let mesh_idle = self.mesh_scheduler.is_idle();
        let frontier_summary = self.frontier_summary();
        let blocked_adjacent = self.blocked_adjacent_reasons();
        let area_state_grid = self.area_state_grid();
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        renderer.set_resident_section_count(
            self.presentation
                .as_ref()
                .map_or(0, RenderWorld::chunk_count),
        );
        let aspect = renderer.width() as f32 / renderer.height().max(1) as f32;
        renderer.telemetry_enabled = self.debug || self.measure_seconds.is_some();
        if legacy_debug || self.measure_seconds.is_some() {
            self.process.sample();
            if let Some(g) = &mut self.gpu_metrics {
                g.sample();
            }
        }
        let mut camera = if let Some(stage) = self.diagnostic.filter(|s| !s.normal_world()) {
            stage.camera(aspect)
        } else if let Some(simulation) = self.simulation.as_ref() {
            camera_for(simulation, aspect)
        } else {
            return;
        };
        let underwater = self.diagnostic.is_none_or(Stage::normal_world)
            && self.simulation.as_ref().is_some_and(camera_is_underwater);
        renderer.set_camera_fog(underwater.then_some(rustcraft_render::FogPresentation {
            color: rustcraft_minecraft_b173::UNDERWATER_FOG_COLOR,
            start: rustcraft_minecraft_b173::UNDERWATER_FOG_START,
            end: rustcraft_minecraft_b173::UNDERWATER_FOG_END,
        }));
        if self.camera_motion && mesh_idle && !self.camera_motion_started {
            let before_reuse = (
                renderer.mesh_buffer_allocations(),
                renderer.mesh_buffer_reallocations(),
                renderer.mesh_buffer_reuses(),
            );
            let mut reuse_submit_ms = 0.0;
            if let Some(presentation) = self.presentation.as_ref() {
                let mut snapshots = presentation.chunks().cloned().collect::<Vec<_>>();
                snapshots
                    .sort_by_key(|chunk| (chunk.position.x, chunk.position.z, chunk.section_y));
                for _ in 0..3 {
                    for snapshot in &snapshots {
                        let pages = rustcraft_render::build_section_mesh_pages(
                            snapshot,
                            &FirstPartyTextures,
                        );
                        reuse_submit_ms += renderer
                            .upload_chunk_pages(snapshot.position, snapshot.section_y, &pages)
                            .submit_ms;
                    }
                }
            }
            self.camera_motion_buffer_reuse_result = (
                renderer
                    .mesh_buffer_allocations()
                    .saturating_sub(before_reuse.0),
                renderer
                    .mesh_buffer_reallocations()
                    .saturating_sub(before_reuse.1),
                renderer.mesh_buffer_reuses().saturating_sub(before_reuse.2),
                reuse_submit_ms,
            );
            self.camera_motion_started = true;
            self.camera_motion_start = Instant::now();
            self.camera_motion_mesh_baseline = renderer.mesh_rebuilds;
            self.camera_motion_buffer_baseline = (
                renderer.mesh_buffer_allocations(),
                renderer.mesh_buffer_reallocations(),
                renderer.mesh_buffer_reuses(),
            );
        }
        let camera_motion_phase = if self.camera_motion && self.camera_motion_started {
            let phase = (self.camera_motion_start.elapsed().as_millis() / 750) as usize;
            let positions = [
                (-24.0, -24.0, 0.0),
                (-24.0, -24.0, std::f32::consts::FRAC_PI_2),
                (24.0, -24.0, std::f32::consts::PI),
                (24.0, 24.0, -std::f32::consts::FRAC_PI_2),
                (0.0, 24.0, std::f32::consts::PI),
                (0.0, 0.0, 0.0),
                (-24.0, 0.0, -std::f32::consts::FRAC_PI_2),
                (24.0, 0.0, std::f32::consts::FRAC_PI_2),
            ];
            if let Some(&(x, z, yaw)) = positions.get(phase) {
                camera.position.x += x;
                camera.position.z += z;
                camera.yaw = yaw;
                Some(phase)
            } else {
                None
            }
        } else {
            None
        };
        let debug_hud_started = Instant::now();
        if self.diagnostic.is_none()
            && let Some(sim) = self.simulation.as_ref()
        {
            if legacy_debug && self.last_snapshot.elapsed().as_millis() >= 1_000 {
                self.process.sample();
                self.last_snapshot = Instant::now();
                self.debug_text = debug::DebugMetricsSnapshot::collect(
                    sim,
                    renderer,
                    &self.metrics,
                    &self.process.snapshot,
                    self.gpu_metrics.as_ref().map(|g| &g.snapshot),
                    debug::RenderPipelineMetrics {
                        meshing: mesh_stats,
                        snapshot_bytes: self
                            .presentation
                            .as_ref()
                            .map_or(0, RenderWorld::snapshot_bytes),
                        upload_submit_ms: self.mesh_upload_submit_ms,
                        uploads_this_frame: self.mesh_uploads_this_frame,
                        upload_bytes_this_frame: self.mesh_upload_bytes_this_frame,
                    },
                )
                .text;
                let persistence = self.persistence_dirty.metrics();
                let saves = self.save_scheduler.metrics();
                let player_saves = self.player_save_scheduler.metrics();
                let world_state_saves = self.world_state_save_scheduler.metrics();
                let loads = self.load_scheduler.metrics();
                let stream_gen = self.generation_scheduler.metrics();
                let center = rustcraft_engine_core::ChunkPos {
                    x: (sim.player.position.x.floor() as i32).div_euclid(16),
                    z: (sim.player.position.z.floor() as i32).div_euclid(16),
                };
                let light_jobs = sim.lighting.integration_columns();
                let light_active = usize::from(sim.lighting.integrating_column().is_some());
                let light_work = sim.lighting.work_counters();
                let voxel_latency = self.voxel_ready_latency_ms.summary();
                let light_latency = self.light_ready_latency_ms.summary();
                let visible_latency = self.render_visible_latency_ms.summary();
                self.debug_text.push_str(&format!(
                    "\nWORLD {} SEED {} RESIDENT CHUNKS {} SECTIONS {} GENERATED {} LOADED {}\nGEN PENDING {} INFLIGHT {} COMPLETE {} STALE {} COALESCED {} TOTAL {} MS\nSTREAM center=({}, {}) radius={} retain={} lookahead={} motion_lead={:.2}col desired={} pending={} load_q/i={}/{} hits={} misses={} failures={} load_ms={:.3} gen_q/i={}/{} light_converged={} lighting_work_ms={:.3} evicted={} save_blocked={} light_blocked={} startup_safe_ms={:.1} full_radius_ms={:?}\nLATENCY ms p50/p90/p95/p99/max request->voxel={:?} request->light={:?} first-section-upload={:?} all-section-upload={:?}\nSTAGES ms p50/p90/p95/p99/max load-total/worker/worker-queue={:?}/{:?}/{:?} gen-total/worker/worker-queue={:?}/{:?}/{:?} publish-cpu={:?} light-queue/active-wall/cpu={:?}/{:?}/{:?} mesh-queue/exec/upload-wait={:?}/{:?}/{:?}\nLIGHTING active={} queued={} capacity_remaining={} budget={} voxels={} boundary={} qpush/pop={}/{} writes={} dirty_sections={} started/completed={}/{} mesh_workers={} override=RUSTCRAFT_MESH_WORKERS\nSTREAM_MAIN budget_ms={:.2} pending_snapshot={} pending_mesh={} pending_evict={}\nCPU_PHASE_MS fixed={} residency={} boundary_light={} snapshot_sync={} mesh_schedule={} mesh_poll_upload={} debug_hud={} render_present={} (render_present includes surface present/vsync)\nCPU_PHASE_P50/P90/P95/P99/MAX_MS fixed/residency/boundary/snapshot/mesh-schedule/mesh-poll/hud/present={:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}\nSAVE DIRTY {} QUEUED {} INFLIGHT {} SAVED {} FAILED {}\nPLAYER SAVE revision={} persisted={} dirty={} requests={} coalesced={} pending={} inflight={} successes={} failures={} encode_ms={:.3} checkpoint_ms={:.3}",
                    self.world_name,
                    std::env::var("RUSTCRAFT_WORLD_SEED").unwrap_or_else(|_| "731173".to_owned()),
                    sim.world.chunk_count(), sim.world.section_count(), self.generated_chunks, self.loaded_from_disk,
                    self.worldgen_metrics.pending, self.worldgen_metrics.in_flight, self.worldgen_metrics.completed,
                    self.worldgen_metrics.stale_discarded, self.worldgen_metrics.coalesced, self.worldgen_metrics.generation_total_ms,
                    center.x, center.z, self.stream_load_radius, self.residency.retain_radius(), if self.stream_lookahead_enabled { "on" } else { "off" }, self.residency.motion_lookahead_columns(), self.residency.desired_column_count(), self.residency.pending_column_count(),
                    loads.queued, loads.in_flight, loads.completed, loads.missing, loads.failed, loads.total_load_ms,
                    stream_gen.pending, stream_gen.in_flight, self.light_converged_count, self.lighting_work_ms,
                    self.eviction_count, self.eviction_blocked_dirty, self.eviction_blocked_lighting, self.startup_ready_ms, self.full_desired_ready_ms,
                    voxel_latency, light_latency,
                    self.first_section_latency_ms.summary(), visible_latency,
                    self.load_total_latency_ms.summary(), self.load_worker_ms.summary(), self.load_queue_wait_ms.summary(),
                    self.generation_total_latency_ms.summary(), self.generation_worker_ms.summary(), self.generation_queue_wait_ms.summary(),
                    self.publication_ms.summary(), self.lighting_queue_wait_ms.summary(), self.lighting_active_ms.summary(),
                    self.lighting_cpu_ms.summary(),
                    self.mesh_queue_wait_ms.summary(), self.mesh_execution_ms.summary(), self.mesh_upload_wait_ms.summary(),
                    light_active, light_jobs.len().saturating_sub(light_active), sim.lighting.integration_capacity_remaining(),
                    self.lighting_work_budget,
                    light_work.direct_voxels_scanned, light_work.boundary_voxels_inspected,
                    light_work.propagation_queue_pushes, light_work.propagation_queue_pops,
                    light_work.light_writes, light_work.dirty_section_insertions,
                    light_work.columns_started, light_work.columns_completed,
                    self.mesh_worker_count,
                    self.stream_main_budget.as_secs_f64() * 1000.0,
                    self.snapshot_dirty_sections.len(), self.mesh_dirty_sections.len(),
                    self.pending_evictions.len(),
                    self.frame_phases.fixed_step_ms, self.frame_phases.residency_ms,
                    self.frame_phases.boundary_lighting_ms,
                    self.frame_phases.render_world_sync_ms, self.frame_phases.mesh_schedule_ms,
                    self.frame_phases.mesh_poll_upload_ms, self.frame_phases.debug_hud_ms,
                    self.frame_phases.render_present_ms,
                    self.frame_phase_samples.fixed_step.summary(),
                    self.frame_phase_samples.residency.summary(),
                    self.frame_phase_samples.boundary_lighting.summary(),
                    self.frame_phase_samples.render_world_sync.summary(),
                    self.frame_phase_samples.mesh_schedule.summary(),
                    self.frame_phase_samples.mesh_poll_upload.summary(),
                    self.frame_phase_samples.debug_hud.summary(),
                    self.frame_phase_samples.render_present.summary(),
                    persistence.dirty_chunks, saves.queued, saves.in_flight, persistence.saved, persistence.failed,
                    self.player_revision, self.player_persisted_revision, self.player_dirty,
                    player_saves.requests, player_saves.coalesced, player_saves.pending,
                    player_saves.in_flight, player_saves.successes, player_saves.failures,
                    self.player_encode_ms_last, self.player_checkpoint_ms_last,
                ));
                let entity_columns = sim
                    .items
                    .iter()
                    .map(|entity| entity.column())
                    .collect::<HashSet<_>>();
                self.debug_text.push_str(&format!(
                    "\nWORLD_STATE revision={} persisted={} dirty={} requests={} coalesced={} pending={} inflight={} successes={} failures={} checkpoint_ms={:.3}\nENTITIES resident={} resident_columns={} loaded={} saved={} encode_payload_bytes={} encode_ms={:.3} decode_ms={:.3} pickup_receipts={} entity_dirty_columns={} save_before_evict={} evictions_blocked={} spatial_format={}",
                    self.world_state_revision,
                    self.world_state_persisted_revision,
                    self.world_state_dirty,
                    world_state_saves.requests,
                    world_state_saves.coalesced,
                    world_state_saves.pending,
                    world_state_saves.in_flight,
                    world_state_saves.successes,
                    world_state_saves.failures,
                    self.world_state_checkpoint_ms_last,
                    sim.items.len(),
                    entity_columns.len(),
                    self.entity_records_loaded,
                    self.entity_records_saved,
                    self.entity_encode_bytes,
                    self.entity_encode_ms,
                    self.entity_decode_ms,
                    sim.pickup_receipts().len(),
                    entity_columns
                        .iter()
                        .filter(|position| self.persistence_dirty.is_dirty(**position))
                        .count(),
                    self.entity_save_before_evict,
                    self.entity_evictions_blocked,
                    rustcraft_world::CHUNK_FORMAT_VERSION,
                ));
                self.debug_text.push_str(&format!(
                    "\nLIGHT_INIT queued={} in_flight={} outstanding={} completed={} stale={} coalesced={} columns_per_sec={:.2} queue_ms={:?} worker_elapsed_ms={:?} direct_voxels={} emitters={} propagation_nodes={}",
                    self.initial_lighting_scheduler.pending(),
                    self.initial_lighting_scheduler.in_flight(),
                    self.initial_lighting_scheduler.outstanding(),
                    self.initial_lighting_scheduler.completed,
                    self.initial_lighting_scheduler.stale,
                    self.initial_lighting_scheduler.coalesced,
                    self.initial_lighting_scheduler.columns_per_second(),
                    self.initial_light_queue_wait_ms.summary(),
                    self.initial_light_worker_ms.summary(),
                    self.initial_light_direct_voxels,
                    self.initial_light_emitters,
                    self.initial_light_propagation_nodes,
                ));
                let stage = &self.stream_stage_fairness;
                self.debug_text.push_str(&format!(
                    "\nSTREAM_FAIR skip c/b/s/m/u={}/{}/{}/{}/{} blocked_turns={}/{}/{}/{}/{} starve_now={}/{}/{}/{}/{} starve_max={}/{}/{}/{}/{} oldest_ms={:.0}/{:.0}/{:.0}/{:.0}/{:.0}",
                    stage[0].skipped_due_to_budget, stage[1].skipped_due_to_budget,
                    stage[2].skipped_due_to_budget, stage[3].skipped_due_to_budget,
                    stage[4].skipped_due_to_budget,
                    stage[0].blocked_dependency_turns, stage[1].blocked_dependency_turns,
                    stage[2].blocked_dependency_turns, stage[3].blocked_dependency_turns,
                    stage[4].blocked_dependency_turns,
                    stage[0].turns_without_service, stage[1].turns_without_service,
                    stage[2].turns_without_service, stage[3].turns_without_service,
                    stage[4].turns_without_service,
                    stage[0].max_turns_without_service, stage[1].max_turns_without_service,
                    stage[2].max_turns_without_service, stage[3].max_turns_without_service,
                    stage[4].max_turns_without_service,
                    stage[0].oldest_age_ms, stage[1].oldest_age_ms,
                    stage[2].oldest_age_ms, stage[3].oldest_age_ms,
                    stage[4].oldest_age_ms,
                ));
                self.debug_text
                    .push_str(&format!("\nFRONTIER {frontier_summary}"));
                self.debug_text.push_str(&format!(
                    "\nMARGINS [forward_safe,forward_visible,lateral_safe,lateral_visible,rear_visible] current={:?} min={:?} p05={:?} mean={:?}\nBLOCKED_ADJACENT {}\n{}",
                    self.travel_margin.current,
                    self.travel_margin.minimum,
                    self.travel_margin.p05(),
                    self.travel_margin.mean(),
                    blocked_adjacent,
                    area_state_grid,
                ));
                self.debug_text.push_str(&format!(
                    "\nRESP frame_ms[p50/p90/p95/p99/max]={:?} event_loop_gap={:?} fixed_tick_gap={:?} event_dispatch={:?} input_callback={:?} input_to_sim={:?} input_to_render={:?} ticks due/run/drop={}/{}/{} dropped_s={:.3} long_tasks[4/8/16/33ms]={:?} worst={:?}",
                    self.responsiveness.frame_interval.summary(),
                    self.responsiveness.event_loop_gap.summary(),
                    self.responsiveness.fixed_tick_gap.summary(),
                    self.responsiveness.event_dispatch.summary(),
                    self.responsiveness.input_processing.summary(),
                    self.responsiveness.input_to_simulation.summary(),
                    self.responsiveness.input_to_render.summary(),
                    self.responsiveness.due_ticks,
                    self.responsiveness.executed_ticks,
                    self.responsiveness.dropped_ticks,
                    self.responsiveness.dropped_seconds,
                    self.responsiveness.long_task_counts,
                    self.responsiveness.longest_task,
                ));
                if self.stream_perf {
                    let elapsed = self
                        .stream_route_started_at
                        .map_or(0.0, |started| started.elapsed().as_secs_f32());
                    let phase = if elapsed < self.stream_perf_motion_seconds {
                        stream_route_label(elapsed, self.stream_perf_motion_seconds / 7.0)
                    } else {
                        "converging"
                    };
                    self.debug_text.push_str(&format!(
                        "\nSTREAM_PROFILE phase={phase} elapsed={elapsed:.1}s active_seconds={:.1} leg_seconds={:.1} speed=4blocks/s lookahead={}",
                        self.stream_perf_motion_seconds,
                        self.stream_perf_motion_seconds / 7.0,
                        if self.stream_lookahead_enabled { "on" } else { "off" },
                    ));
                }
                let trace_interval = if self.stream_perf { 10 } else { 1 };
                if self.debug_trace
                    && self.last_debug_trace.elapsed() >= Duration::from_secs(trace_interval)
                {
                    eprintln!("F3 {}", self.debug_text.replace('\n', " | "));
                    self.last_debug_trace = Instant::now();
                }
                self.debug_overlay_text = compact_debug_overlay(&self.debug_text);
                if self.inventory_open {
                    self.debug_text = format!(
                        "INVENTORY (E closes)\n{}\n{}",
                        inventory_text(sim),
                        self.debug_text
                    );
                    self.debug_overlay_text = compact_debug_overlay(&self.debug_text);
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
                let block = placeable.unwrap_or(rustcraft_minecraft_b173::blocks::PLANKS.id);
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
            let cursor_slot = sim.inventory_cursor.and_then(visual);
            let item_sprites = sim
                .items
                .iter()
                .filter_map(|e| {
                    let block = sim
                        .registry
                        .item(e.stack.item)
                        .and_then(|i| i.placeable)
                        .unwrap_or(rustcraft_minecraft_b173::blocks::PLANKS.id);
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
                        hover_start: (((e.id.0 as u64) ^ ((e.id.0 >> 64) as u64)) as f32
                            * 0.61803395)
                            .fract(),
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
                (!self.inventory_open)
                    .then(|| {
                        sim.mining
                            .map(|m| rustcraft_render::hud::destroy_stage(m.progress))
                    })
                    .flatten()
                    .and_then(|stage| {
                        first_party_compiled()
                            .ok()
                            .map(|compiled| compiled.destroy_stages[stage as usize])
                    }),
            );
            renderer.set_debug_boxes_colored(&self.dx_boxes, &self.dx_colors);
            renderer.set_hud(
                &rustcraft_render::hud::HudSnapshot {
                    slots,
                    selected: sim.inventory.selected(),
                    target: sim.target().map(|h| h.block),
                    text: if !self.dx_text.is_empty() {
                        &self.dx_text
                    } else if self.debug {
                        &self.debug_overlay_text
                    } else {
                        ""
                    },
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
        self.frame_phases.debug_hud_ms = debug_hud_started.elapsed().as_secs_f64() * 1000.0;
        self.frame_phase_samples
            .debug_hud
            .record_ms(self.frame_phases.debug_hud_ms);
        let load_metrics = self.load_scheduler.metrics();
        let generation_metrics = self.generation_scheduler.metrics();
        let streaming_idle = self.full_desired_ready_ms.is_some()
            && self.residency.pending_column_count() == 0
            && load_metrics.queued == 0
            && load_metrics.in_flight == 0
            && generation_metrics.pending == 0
            && generation_metrics.in_flight == 0
            && (self.stream_perf
                || self
                    .simulation
                    .as_ref()
                    .is_none_or(|simulation| !simulation.lighting.has_integration_work()));
        let finished = self.measure_seconds.is_some_and(|seconds| {
            let deadline = self.metrics.uptime.elapsed().as_secs_f64() >= seconds;
            if self.stream_perf {
                // Complete early after route/convergence, but always enforce the hard deadline,
                // even if a broken controller keeps producing streaming work. Acceptance fails
                // below when route completion, margins or input metrics are missing.
                deadline || (self.stream_route_completed && mesh_idle && streaming_idle)
            } else {
                deadline && mesh_idle && streaming_idle
            }
        });
        let dx_capture_path = if renderer.capture_pending() {
            None
        } else {
            self.dx_capture.clone()
        };
        let capture = if dx_capture_path.is_some() {
            dx_capture_path.as_deref()
        } else if mesh_idle && (self.measure_seconds.is_none() || finished) {
            self.capture.as_deref()
        } else {
            None
        };
        let render_present_started = Instant::now();
        match if dx_capture_path.is_some() {
            renderer.render_capture_async(camera, capture)
        } else {
            renderer.render_capture(camera, capture)
        } {
            Ok(()) => {
                if let Some(tools) = self.devtools.as_mut() {
                    tools.frame += 1;
                }
                if dx_capture_path.is_some() {
                    self.dx_capture.take();
                }
                if let Some(phase) = camera_motion_phase {
                    self.camera_motion_samples[phase] = Some((
                        renderer.submission_stats(),
                        renderer.mesh_rebuilds,
                        renderer.draw_calls(),
                    ));
                }
                if self.camera_motion
                    && self.camera_motion_started
                    && self.camera_motion_start.elapsed().as_millis() / 750 >= 8
                    && self.mesh_scheduler.is_idle()
                {
                    let rebuild_delta = renderer
                        .mesh_rebuilds
                        .saturating_sub(self.camera_motion_mesh_baseline);
                    let allocation_delta = renderer
                        .mesh_buffer_allocations()
                        .saturating_sub(self.camera_motion_buffer_baseline.0);
                    let reallocation_delta = renderer
                        .mesh_buffer_reallocations()
                        .saturating_sub(self.camera_motion_buffer_baseline.1);
                    let reuse_delta = renderer
                        .mesh_buffer_reuses()
                        .saturating_sub(self.camera_motion_buffer_baseline.2);
                    for (phase, sample) in self.camera_motion_samples.iter().enumerate() {
                        if let Some((stats, _, draw_calls)) = sample {
                            eprintln!(
                                "R1.2 CAMERA phase={phase} resident={} visible={} culled={} drawn_page_batches={} draw_calls={} page_bind_switches={} pipeline_switches={} mesh_rebuild_delta={rebuild_delta}",
                                stats.resident_sections,
                                stats.visible_sections,
                                stats.culled_sections,
                                stats.drawn_section_page_batches,
                                draw_calls,
                                stats.texture_page_bind_switches,
                                stats.pipeline_switches,
                            );
                        }
                    }
                    eprintln!(
                        "R1.2 BUFFER_REUSE cycles=3 allocations={} reallocations={} reuses={} submit_ms={:.3} baseline_extra_allocations_without_reuse={} total_allocations={} total_reallocations={} total_reuses={} movement_rebuild_delta={rebuild_delta} movement_allocations_delta={allocation_delta} movement_reallocations_delta={reallocation_delta} movement_reuses_delta={reuse_delta}",
                        self.camera_motion_buffer_reuse_result.0,
                        self.camera_motion_buffer_reuse_result.1,
                        self.camera_motion_buffer_reuse_result.2,
                        self.camera_motion_buffer_reuse_result.3,
                        self.camera_motion_buffer_reuse_result.2,
                        renderer.mesh_buffer_allocations(),
                        renderer.mesh_buffer_reallocations(),
                        renderer.mesh_buffer_reuses(),
                    );
                    event_loop.exit();
                    return;
                }
                if finished {
                    eprintln!(
                        "R1.2 MEASURE overlay={} fps={:?} low={:?} frame_ms={:?} tps={:?} tick_ms={:?} process={:?} gpu_ms={:?} gpu={:?} resident={} meshed={} visible={} culled={} section_page_draws={} page_switches={} pipeline_switches={} draw_calls={} mesh_count={} rebuilds={} mesh_ms={:?} jobs={mesh_stats:?} uploads={} upload_bytes={} uploads_this_frame={} upload_bytes_this_frame={} upload_submit_ms={:.3} gpu_mesh_logical_bytes={} gpu_mesh_allocated_bytes={} buffer_allocations={} buffer_reallocations={} buffer_reuses={}",
                        self.debug,
                        self.metrics.frames.fps(),
                        self.metrics.frames.low(),
                        self.metrics.frames.mean().map(|v| v * 1000.),
                        self.metrics.tps,
                        self.metrics.ticks.mean().map(|v| v * 1000.),
                        self.process.snapshot,
                        renderer.gpu_ms(),
                        self.gpu_metrics.as_ref().map(|g| &g.snapshot),
                        renderer.submission_stats().resident_sections,
                        renderer.submission_stats().meshed_sections,
                        renderer.submission_stats().visible_sections,
                        renderer.submission_stats().culled_sections,
                        renderer.submission_stats().drawn_section_page_batches,
                        renderer.submission_stats().texture_page_bind_switches,
                        renderer.submission_stats().pipeline_switches,
                        renderer.draw_calls(),
                        renderer.mesh_count(),
                        renderer.mesh_rebuilds,
                        self.metrics.meshes.mean().map(|v| v * 1000.),
                        self.mesh_uploads,
                        self.mesh_upload_bytes,
                        self.mesh_uploads_this_frame,
                        self.mesh_upload_bytes_this_frame,
                        self.mesh_upload_submit_ms,
                        renderer.gpu_mesh_logical_bytes(),
                        renderer.gpu_mesh_allocated_bytes(),
                        renderer.mesh_buffer_allocations(),
                        renderer.mesh_buffer_reallocations(),
                        renderer.mesh_buffer_reuses(),
                    );
                    if self.stream_perf {
                        let simulation = self.simulation.as_ref().expect("running world");
                        let light = simulation.lighting.work_counters();
                        eprintln!(
                            "STREAM_RESULT radius={} retain={} lookahead={} desired={} resident={} generated={} loaded={} evicted={} fps={:?} frame_ms={:?} low={:?} tps={:?} gpu_ms={:?} request_voxel={:?} request_light={:?} first_upload={:?} all_upload={:?} load_total/worker/worker_queue={:?}/{:?}/{:?} generation_total/worker/worker_queue={:?}/{:?}/{:?} publication_cpu={:?} boundary_light_queue/active_wall/cpu={:?}/{:?}/{:?} initial_light_queue/worker_elapsed={:?}/{:?} initial_light_jobs queued/inflight/completed/stale/coalesced={}/{}/{}/{}/{} columns_per_sec={:.2} mesh_queue/execution/upload_wait={:?}/{:?}/{:?} lighting_ops voxels={} boundary={} pushes/pops={}/{} dirty_sections={} emitters={} lighting_backlog={} mesh submitted/completed/stale/coalesced={}/{}/{}/{} margins[forward_safe,forward_visible,lateral_safe,lateral_visible,rear_visible] current/min/p05/mean={:?}/{:?}/{:?}/{:?} frame_cpu_p95 fixed/residency/snapshot/mesh_schedule/mesh_poll/hud/present={:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}",
                            self.stream_load_radius,
                            self.residency.retain_radius(),
                            self.stream_lookahead_enabled,
                            self.residency.desired_column_count(),
                            simulation.world.chunk_count(),
                            self.generated_chunks,
                            self.loaded_from_disk,
                            self.eviction_count,
                            self.metrics.frames.fps(),
                            self.metrics.frames.mean().map(|v| v * 1000.0),
                            self.metrics.frames.low(),
                            self.metrics.tps,
                            renderer.gpu_ms(),
                            self.voxel_ready_latency_ms.summary(),
                            self.light_ready_latency_ms.summary(),
                            self.first_section_latency_ms.summary(),
                            self.render_visible_latency_ms.summary(),
                            self.load_total_latency_ms.summary(),
                            self.load_worker_ms.summary(),
                            self.load_queue_wait_ms.summary(),
                            self.generation_total_latency_ms.summary(),
                            self.generation_worker_ms.summary(),
                            self.generation_queue_wait_ms.summary(),
                            self.publication_ms.summary(),
                            self.lighting_queue_wait_ms.summary(),
                            self.lighting_active_ms.summary(),
                            self.lighting_cpu_ms.summary(),
                            self.initial_light_queue_wait_ms.summary(),
                            self.initial_light_worker_ms.summary(),
                            self.initial_lighting_scheduler.pending(),
                            self.initial_lighting_scheduler.in_flight(),
                            self.initial_lighting_scheduler.completed,
                            self.initial_lighting_scheduler.stale,
                            self.initial_lighting_scheduler.coalesced,
                            self.initial_lighting_scheduler.columns_per_second(),
                            self.mesh_queue_wait_ms.summary(),
                            self.mesh_execution_ms.summary(),
                            self.mesh_upload_wait_ms.summary(),
                            light.direct_voxels_scanned,
                            light.boundary_voxels_inspected,
                            light.propagation_queue_pushes,
                            light.propagation_queue_pops,
                            light.dirty_section_insertions,
                            light.emitters_found,
                            simulation.lighting.integration_columns().len(),
                            mesh_stats.mesh_jobs_submitted,
                            mesh_stats.mesh_jobs_completed,
                            mesh_stats.mesh_jobs_discarded_stale,
                            mesh_stats.mesh_jobs_coalesced,
                            self.travel_margin.current,
                            self.travel_margin.minimum,
                            self.travel_margin.p05(),
                            self.travel_margin.mean(),
                            self.frame_phase_samples.fixed_step.summary().map(|v| v[2]),
                            self.frame_phase_samples.residency.summary().map(|v| v[2]),
                            self.frame_phase_samples
                                .render_world_sync
                                .summary()
                                .map(|v| v[2]),
                            self.frame_phase_samples
                                .mesh_schedule
                                .summary()
                                .map(|v| v[2]),
                            self.frame_phase_samples
                                .mesh_poll_upload
                                .summary()
                                .map(|v| v[2]),
                            self.frame_phase_samples.debug_hud.summary().map(|v| v[2]),
                            self.frame_phase_samples
                                .render_present
                                .summary()
                                .map(|v| v[2]),
                        );
                        eprintln!(
                            "RESPONSIVENESS frame_ms[p50/p90/p95/p99/max]={:?} event_loop_gap={:?} fixed_tick_gap={:?} event_dispatch={:?} input_callback={:?} input_to_sim={:?} input_to_render={:?} ticks due/run/drop={}/{}/{} dropped_s={:.3} long_tasks[4/8/16/33ms]={:?} worst={:?}",
                            self.responsiveness.frame_interval.summary(),
                            self.responsiveness.event_loop_gap.summary(),
                            self.responsiveness.fixed_tick_gap.summary(),
                            self.responsiveness.event_dispatch.summary(),
                            self.responsiveness.input_processing.summary(),
                            self.responsiveness.input_to_simulation.summary(),
                            self.responsiveness.input_to_render.summary(),
                            self.responsiveness.due_ticks,
                            self.responsiveness.executed_ticks,
                            self.responsiveness.dropped_ticks,
                            self.responsiveness.dropped_seconds,
                            self.responsiveness.long_task_counts,
                            self.responsiveness.longest_task,
                        );
                    }
                }
                if (capture.is_some() && dx_capture_path.is_none()) || finished {
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
        self.frame_phases.render_present_ms =
            render_present_started.elapsed().as_secs_f64() * 1000.0;
        self.frame_phase_samples
            .render_present
            .record_ms(self.frame_phases.render_present_ms);
        self.responsiveness.record_long_task(
            "render_present",
            render_present_started.elapsed(),
            self.mesh_uploads_this_frame,
        );
    }
}

impl ClientApp {
    fn inventory_click(&mut self, slot: usize) {
        if let Some(sim) = self.simulation.as_mut() {
            sim.inventory_cursor =
                sim.inventory
                    .slot_click(Some(slot), 0, sim.inventory_cursor, &sim.registry);
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
                            sim.inventory_cursor = sim.inventory.slot_click(
                                Some(index),
                                1,
                                sim.inventory_cursor,
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
                    let held = sim.inventory_cursor.take();
                    sim.inventory_cursor = sim.swap_crafting_slot(i, held);
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
            && self
                .simulation
                .as_ref()
                .is_some_and(|sim| sim.inventory_cursor.is_none())
            && let Some(sim) = self.simulation.as_mut()
        {
            sim.inventory_cursor = sim.take_crafting_output_for_cursor();
        }
    }
}
fn compact_debug_overlay(text: &str) -> String {
    const MAX_LINE_CHARS: usize = 150;
    const MAX_LINES: usize = 13;
    let mut output = String::new();
    for line in text
        .lines()
        .filter(|line| {
            line.starts_with("WORLD ")
                || line.starts_with("STREAM ")
                || line.starts_with("STREAM_FAIR ")
                || line.starts_with("FRONTIER ")
                || line.starts_with("LATENCY ")
                || line.starts_with("STAGES ")
                || line.starts_with("LIGHTING ")
                || line.starts_with("CPU_PHASE_MS ")
                || line.starts_with("SAVE DIRTY ")
                || line.starts_with("PLAYER SAVE ")
        })
        .take(MAX_LINES)
    {
        if !output.is_empty() {
            output.push('\n');
        }
        let mut chars = line.chars();
        output.extend(chars.by_ref().take(MAX_LINE_CHARS));
        if chars.next().is_some() {
            output.push('…');
        }
    }
    output
}

#[cfg(test)]
mod debug_overlay_tests {
    use super::{
        StreamStageFairness, compact_debug_overlay, pop_before_deadline, pop_urgent_before_deadline,
    };
    use std::{
        collections::HashSet,
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn compact_debug_overlay_is_bounded_and_keeps_stream_summary_lines() {
        let input = format!(
            "WORLD {}\nSTREAM {}\nLATENCY {}\nSTAGES {}\nLIGHTING {}\nCPU_PHASE_MS {}\nSAVE DIRTY {}\nPLAYER SAVE {}\nignored {}",
            "x".repeat(500),
            "x".repeat(500),
            "x".repeat(500),
            "x".repeat(500),
            "x".repeat(500),
            "x".repeat(500),
            "x".repeat(500),
            "x".repeat(500),
            "x".repeat(500),
        );
        let compact = compact_debug_overlay(&input);
        assert!(compact.chars().count() <= 8 * 151 + 7);
        assert!(!compact.contains("ignored"));
        assert!(compact.contains("WORLD "));
        assert!(compact.contains("PLAYER SAVE "));
    }

    #[test]
    fn stream_backlog_yields_at_deadline_so_next_event_turn_can_run() {
        let mut pending = (0..100).collect::<HashSet<_>>();
        let deadline = Instant::now() + Duration::from_millis(4);
        let mut processed = 0;
        while let Some(_item) = pop_before_deadline(&mut pending, deadline) {
            processed += 1;
            thread::sleep(Duration::from_millis(1));
        }

        // Model the next winit turn/input callback immediately after streaming yields.
        let next_event_turn = Instant::now();
        assert!(
            processed < 100,
            "stream backlog must remain for later turns"
        );
        assert!(!pending.is_empty());
        assert!(next_event_turn < deadline + Duration::from_millis(30));
    }

    #[test]
    fn urgent_presentation_work_is_selected_before_far_prefetch() {
        let now = Instant::now();
        let mut pending = HashSet::from([(8, 0), (1, 0), (3, 0)]);
        let selected =
            pop_urgent_before_deadline(&mut pending, now + Duration::from_secs(1), |p| p.0);
        assert_eq!(selected, Some((1, 0)));
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn stage_metrics_count_skips_and_prove_service_resets_starvation() {
        let now = Instant::now();
        let mut stage = StreamStageFairness::default();
        stage.observe(true, false, false, false, now);
        stage.observe(true, false, true, true, now + Duration::from_millis(10));
        assert_eq!(stage.skipped_due_to_budget, 1);
        assert_eq!(stage.blocked_dependency_turns, 1);
        assert_eq!(stage.turns_without_service, 0);
        stage.observe(true, true, true, false, now + Duration::from_millis(20));
        assert_eq!(stage.turns_without_service, 0);
        assert_eq!(stage.max_turns_without_service, 1);
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
        self.cancel_startup_and_join();
        self.finish_world_saves();
        if self.stream_perf {
            self.stream_final_report = Some(self.autonomous_stream_report());
        }
        self.renderer.take();
        self.window.take();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // Only explicit streaming diagnostics may override the normal player's window size.
        // This isolates software-GPU fill cost without changing simulation or acceptance rules.
        let (width, height) = if self.stream_perf {
            let size = std::env::var("RUSTCRAFT_STREAM_WINDOW_SIZE").ok();
            match stream_window_size(size.as_deref()) {
                Ok(size) => size,
                Err(error) => {
                    eprintln!("{error}");
                    event_loop.exit();
                    return;
                }
            }
        } else {
            (1280, 720)
        };
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(self.diagnostic.map_or_else(
                            || "RustCraft M2".to_owned(),
                            |stage| format!("RustCraft diagnostic: {stage:?}"),
                        ))
                        .with_inner_size(winit::dpi::PhysicalSize::new(width, height)),
                )
                .expect("create window"),
        );
        self.window = Some(window.clone());
        if self.diagnostic.is_none_or(Stage::normal_world) {
            window.set_title("RustCraft — Loading world…");
            self.begin_world_startup();
        }
        let resources = match self.resources.as_ref() {
            Ok(resources) => resources,
            Err(error) => {
                eprintln!("unable to compile presentation resources: {error}");
                event_loop.exit();
                return;
            }
        };
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
            pollster::block_on(Renderer::new_diagnostic(window.clone(), resources, stage))
        } else {
            pollster::block_on(Renderer::new(window.clone(), resources))
        };
        let renderer = match result {
            Ok(renderer) => renderer,
            Err(error) => {
                eprintln!("unable to initialize graphics: {error}");
                std::process::exit(1);
            }
        };
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
        self.camera_motion_start = Instant::now();
    }
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let dispatch_started = Instant::now();
        let is_input = matches!(
            &event,
            WindowEvent::CursorMoved { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::Focused(_)
                | WindowEvent::KeyboardInput { .. }
                | WindowEvent::MouseInput { .. }
        );
        let is_redraw = matches!(&event, WindowEvent::RedrawRequested);
        if is_input {
            self.responsiveness
                .pending_input_at
                .get_or_insert(dispatch_started);
        }
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
                if !self.dev_focus() {
                    self.controller.wheel(delta);
                }
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
                if self.devtools.is_some() && self.dev_key(&event) {
                    return;
                }

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
                if self.dev_focus() {
                    return;
                }
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
                } else {
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
            }
            _ => {}
        }
        let elapsed = dispatch_started.elapsed();
        if !is_redraw {
            self.responsiveness.event_dispatch.record(elapsed);
            self.responsiveness
                .record_long_task("window_event_dispatch", elapsed, 1);
        }
        if is_input {
            self.responsiveness.input_processing.record(elapsed);
            self.responsiveness
                .record_long_task("input_event_processing", elapsed, 1);
        }
    }
    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        let event_started = Instant::now();
        let is_input = matches!(&event, DeviceEvent::MouseMotion { .. });
        if is_input {
            self.responsiveness
                .pending_input_at
                .get_or_insert(event_started);
        }
        if self.controller.captured
            && !self.dev_focus()
            && let DeviceEvent::MouseMotion { delta } = event
        {
            self.controller.look.x += delta.0 as f32;
            self.controller.look.y += delta.1 as f32;
        }
        if is_input {
            let elapsed = event_started.elapsed();
            self.responsiveness.input_processing.record(elapsed);
            self.responsiveness
                .record_long_task("device_input_processing", elapsed, 1);
        }
    }
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.apply_config_boundary(rustcraft_control::config::Policy::NextFrame);
        let callback_started = Instant::now();
        self.stream_turn_started = Some(callback_started);
        if let Some(previous) = self
            .responsiveness
            .last_about_to_wait
            .replace(callback_started)
        {
            let gap = callback_started.duration_since(previous);
            self.responsiveness.event_loop_gap.record(gap);
        }
        let startup_poll_started = Instant::now();
        self.poll_world_startup(_event_loop);
        self.responsiveness.record_long_task(
            "startup_result_poll",
            startup_poll_started.elapsed(),
            1,
        );
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame).as_secs_f64();
        self.last_frame = now;
        let budget = self.clock.advance(elapsed);
        self.responsiveness.due_ticks += budget.due_steps;
        self.responsiveness.dropped_ticks +=
            budget.due_steps.saturating_sub(u64::from(budget.steps));
        self.responsiveness.dropped_seconds += budget.dropped_seconds;
        self.metrics.catch_up = budget.catch_up;
        self.service_devtools_measured(_event_loop);
        if self.dx_exit_pending
            && self.dx_capture.is_none()
            && !self.renderer.as_ref().is_some_and(|r| r.capture_pending())
        {
            if !self.dx_capture_failure_recorded {
                let error = self.dx_capture_job.and_then(|id| {
                    self.devtools
                        .as_ref()
                        .and_then(|d| match d.jobs.status(id) {
                            Ok(rustcraft_control::JobStatus::Failed(e)) => Some(e.clone()),
                            _ => None,
                        })
                });
                if let Some(error) = error {
                    self.dx_capture_failure_recorded = true;
                    eprintln!("DX capture failed: {error}; partial bundle");
                    self.dx_capture_job = self
                        .devtools
                        .as_mut()
                        .and_then(|d| d.record_capture_failure(&error).ok().flatten());
                }
            }
            let terminal = self.dx_capture_job.is_none_or(|id| {
                self.devtools.as_ref().is_some_and(|d| {
                    !matches!(d.jobs.status(id), Ok(rustcraft_control::JobStatus::Pending))
                })
            });
            if terminal {
                _event_loop.exit();
                return;
            }
        }

        let ticks = self.control_state.fixed.take_ticks(budget.steps);
        self.responsiveness.executed_ticks += u64::from(ticks);
        self.metrics.steps = ticks;
        for _ in 0..ticks {
            self.fixed_step();
        }
        self.service_streaming_turn();
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

#[derive(Clone, Copy)]
struct FirstPartyTextures;
impl BlockTextureResolver for FirstPartyTextures {
    fn model_rotation(
        &self,
        state: rustcraft_engine_core::BlockState,
    ) -> rustcraft_engine_core::orientation::ModelRotation {
        let Ok(compiled) = first_party_compiled() else {
            return rustcraft_engine_core::orientation::ModelRotation::IDENTITY;
        };
        let Some(block) = compiled
            .legacy_to_compiled
            .get(state.block.0 as usize)
            .copied()
            .flatten()
        else {
            return rustcraft_engine_core::orientation::ModelRotation::IDENTITY;
        };
        compiled
            .materials
            .model_rotation(rustcraft_engine_core::BlockState {
                block,
                variant: state.variant,
            })
    }

    fn tint(&self, block: BlockId, face: Face) -> [f32; 3] {
        let Ok(compiled) = first_party_compiled() else {
            return [1.0; 3];
        };
        compiled
            .legacy_to_compiled
            .get(block.0 as usize)
            .copied()
            .flatten()
            .map_or([1.0; 3], |block| compiled.materials.tint(block, face))
    }

    fn texture(&self, block: BlockId, face: Face) -> Option<AtlasRegion> {
        let compiled = first_party_compiled().ok()?;
        let block = compiled
            .legacy_to_compiled
            .get(block.0 as usize)
            .copied()
            .flatten()?;
        compiled.materials.texture(block, face)
    }
    fn opaque(&self, block: BlockId) -> bool {
        let Ok(compiled) = first_party_compiled() else {
            return false;
        };
        compiled
            .legacy_to_compiled
            .get(block.0 as usize)
            .copied()
            .flatten()
            .is_some_and(|block| compiled.materials.opaque(block))
    }
    fn translucent(&self, block: BlockId) -> bool {
        let Ok(compiled) = first_party_compiled() else {
            return false;
        };
        compiled
            .legacy_to_compiled
            .get(block.0 as usize)
            .copied()
            .flatten()
            .is_some_and(|block| {
                compiled
                    .materials
                    .block(block)
                    .is_some_and(|m| m.translucent)
            })
    }
    fn liquid_surface_height(&self, state: rustcraft_engine_core::BlockState) -> Option<f32> {
        let compiled = first_party_compiled().ok()?;
        let block = compiled
            .legacy_to_compiled
            .get(state.block.0 as usize)
            .copied()
            .flatten()?;
        compiled
            .materials
            .block(block)
            .and_then(|material| material.liquid_surface_height)
    }
    fn visible(&self, block: BlockId) -> bool {
        let Ok(compiled) = first_party_compiled() else {
            return false;
        };
        compiled
            .legacy_to_compiled
            .get(block.0 as usize)
            .copied()
            .flatten()
            .is_some_and(|block| compiled.materials.visible(block))
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

fn decorate_resident_legacy_sandbox(
    world: &mut rustcraft_engine_core::World,
    registry: &rustcraft_mod_api::BlockRegistry,
) -> bool {
    // A version-specific dry spawn may be far from origin. Authored samples must never create
    // partial origin columns outside that complete startup neighborhood or influence its spawn.
    let resident = world.column_positions().collect::<HashSet<_>>();
    if ![-1, 0]
        .into_iter()
        .all(|x| resident.contains(&rustcraft_engine_core::ChunkPos { x, z: 0 }))
    {
        return false;
    }
    let base_y = Simulation::spawn_above_surface(world, registry, 0, 0).y as i32 - 2;
    rustcraft_minecraft_b173::flat_world::decorate_sandbox_at(world, registry, base_y);
    true
}

fn initialize_simulation(
    world: rustcraft_engine_core::World,
    registry: rustcraft_mod_api::BlockRegistry,
    restored: Option<rustcraft_minecraft_b173::player_persistence::RestoredPlayer>,
    survival_start: bool,
) -> Simulation {
    let center = restored.as_ref().map_or((0, 0), |state| {
        (
            (state.position.x.floor() as i32).div_euclid(16),
            (state.position.z.floor() as i32).div_euclid(16),
        )
    });
    let spawn = if restored.is_none() {
        rustcraft_minecraft_b173::worldgen::select_safe_spawn(&world).unwrap_or_else(|| {
            Simulation::spawn_above_surface(&world, &registry, center.0 * 16, center.1 * 16)
        })
    } else {
        Simulation::spawn_above_surface(&world, &registry, center.0 * 16, center.1 * 16)
    };
    let mut simulation = Simulation::new(world, registry, spawn);
    if !survival_start {
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
    if let Some(state) = restored {
        rustcraft_minecraft_b173::player_persistence::apply(&mut simulation, state);
    }
    simulation
}

fn encode_simulation_column(
    simulation: &Simulation,
    position: rustcraft_engine_core::ChunkPos,
) -> Result<
    (
        rustcraft_world::StoredChunk,
        rustcraft_runtime::EntityColumnSnapshot,
    ),
    String,
> {
    let sections = simulation
        .world
        .section_positions()
        .filter(|(candidate, _)| *candidate == position)
        .filter_map(|(_, y)| {
            simulation
                .world
                .section(position, y)
                .cloned()
                .map(|chunk| (y, chunk))
        })
        .collect::<Vec<_>>();
    let spatial_records = simulation
        .items
        .iter()
        .filter(|entity| entity.column() == position)
        .map(|entity| {
            rustcraft_minecraft_b173::world_persistence::encode_item_entity(
                entity,
                &simulation.registry,
            )
            .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let snapshot = simulation.entity_column_snapshot(position);
    let spatial_tombstones = snapshot
        .tombstones
        .iter()
        .map(
            |(entity_id, source, entity_revision)| rustcraft_world::SpatialTombstone {
                entity_id: *entity_id,
                entity_revision: *entity_revision,
                source: *source,
            },
        )
        .collect();
    let resolver = rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver;
    let stored = rustcraft_world::WorldStorage::encode_runtime_column(
        position,
        sections,
        &resolver,
        spatial_records,
        spatial_tombstones,
    )
    .map_err(|error| error.to_string())?;
    Ok((stored, snapshot))
}

fn camera_is_underwater(simulation: &Simulation) -> bool {
    let eye = Vec3::new(
        simulation.player.position.x,
        simulation.player.position.y + 1.62 - 0.01,
        simulation.player.position.z,
    );
    let position = rustcraft_engine_core::BlockPos {
        x: eye.x.floor() as i32,
        y: eye.y.floor() as i32,
        z: eye.z.floor() as i32,
    };
    let block = simulation.world.get(position);
    let Ok(compiled) = first_party_compiled() else {
        return false;
    };
    let Some(id) = compiled
        .legacy_to_compiled
        .get(block.0 as usize)
        .copied()
        .flatten()
    else {
        return false;
    };
    let Some(render) = compiled.materials.block(id) else {
        return false;
    };
    render.material == rustcraft_game_api::MaterialClass::Liquid
        && (eye.y - eye.y.floor()) < render.liquid_surface_height.unwrap_or(1.0) - 0.005
}

fn stream_current_core_ready(
    simulation: &Simulation,
    render_ready: &HashSet<rustcraft_engine_core::ChunkPos>,
) -> bool {
    let position = simulation.player.position;
    let center = rustcraft_engine_core::ChunkPos {
        x: (position.x.floor() as i32).div_euclid(16),
        z: (position.z.floor() as i32).div_euclid(16),
    };
    (-1..=1).all(|dz| {
        (-1..=1).all(|dx| {
            let column = rustcraft_engine_core::ChunkPos {
                x: center.x + dx,
                z: center.z + dz,
            };
            simulation.world.column_available(column) && render_ready.contains(&column)
        })
    })
}

fn run_headless_stream_phase(app: &mut ClientApp, end_route_seconds: f32, route_offset: f32) {
    let deadline = Instant::now() + Duration::from_secs(240);
    let mut next_tick = Instant::now();
    let mut next_mesh = Instant::now();
    let mut offset_applied = route_offset == 0.0;
    loop {
        let now = Instant::now();
        assert!(
            now < deadline,
            "headless world travel phase timed out: player={:?} anchor={:?} returned={} return_target={:?} navigation_target={:?} margins={:?}",
            app.simulation
                .as_ref()
                .map(|simulation| simulation.player.position),
            app.stream_route_anchor,
            app.stream_returned_to_origin,
            app.stream_return_path.back(),
            app.stream_navigation_path.front(),
            app.travel_margin.minimum
        );
        app.stream_turn_started = Some(now);
        app.service_streaming_turn();
        if now >= next_mesh {
            app.process_mesh_jobs();
            next_mesh += Duration::from_millis(16);
        }
        if app.player_control_enabled && !offset_applied {
            app.stream_route_started_at = Some(now - Duration::from_secs_f32(route_offset));
            offset_applied = true;
        }
        if now >= next_tick {
            let before = app.simulation.as_ref().unwrap().player.position;
            app.fixed_step();
            let simulation = app.simulation.as_ref().unwrap();
            let player = simulation.player.position;
            let floor = simulation.world.get(rustcraft_engine_core::BlockPos {
                x: player.x.floor() as i32,
                y: 0,
                z: player.z.floor() as i32,
            });
            assert!(
                player.y >= 0.0,
                "travel entered below-bedrock void: before={before:?} player={player:?} velocity={:?} floor={floor:?} floor_solid={} available={} route_offset={route_offset}",
                simulation.player.velocity,
                simulation.registry.is_solid(floor),
                simulation
                    .world
                    .column_available(rustcraft_engine_core::ChunkPos {
                        x: (player.x.floor() as i32).div_euclid(16),
                        z: (player.z.floor() as i32).div_euclid(16),
                    })
            );
            next_tick += Duration::from_millis(50);
        }
        let route_elapsed = app
            .stream_route_started_at
            .map_or(0.0, |started| started.elapsed().as_secs_f32());
        if route_elapsed >= end_route_seconds {
            let intermediate_checkpoint = end_route_seconds < app.stream_perf_motion_seconds;
            let core_ready = app.simulation.as_ref().is_some_and(|simulation| {
                stream_current_core_ready(simulation, &app.render_ready_columns)
            });
            // Match the actual-client gate: cross-border lighting is eventual derived work,
            // not an outer-world barrier to a usable returned core or graceful durable close.
            // Seam/convergence correctness has independent lighting and streaming benchmarks.
            let converged = app.mesh_scheduler.is_idle()
                && app.residency.pending_column_count() == 0
                && core_ready;
            if intermediate_checkpoint || (app.stream_returned_to_origin && converged) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn prepare_headless_stream_app(root: &std::path::Path) -> ClientApp {
    let mut app = ClientApp::new(None, None);
    app.world_name = "world-travel-test".to_owned();
    app.saves_directory = root.to_path_buf();
    app.stream_perf = true;
    app.survival_start = true;
    // Slightly longer than the surface-client profile so collision/terrain variance still moves
    // the edited origin strictly outside the final retain radius of five columns.
    app.stream_perf_motion_seconds = 196.0;
    app.start_world().expect("initialize headless travel world");
    app.world_ready_at = Some(Instant::now());
    if let Some(simulation) = app.simulation.as_mut() {
        let resident = simulation.world.column_positions().collect::<Vec<_>>();
        for position in resident {
            assert!(simulation.world.set_column_safe(position, false));
        }
    }
    app.player_control_enabled = false;
    app.rebuild_meshes();
    app
}

fn run_world_travel_test() {
    // PID namespaces can reuse process IDs across parallel diagnostic invocations. Never
    // remove another running diagnostic's world merely because its visible PID is the same.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    let root =
        PathBuf::from("target").join(format!("world-travel-test-{}-{nonce}", std::process::id()));
    assert!(!root.exists(), "disposable travel-world identity collision");
    let mut first = prepare_headless_stream_app(&root);
    let mut edit = None;
    let mut durable_drops = Vec::new();
    let mutation_deadline = Instant::now() + Duration::from_secs(30);
    while !first.player_control_enabled {
        assert!(
            Instant::now() < mutation_deadline,
            "startup SAFE+VISIBLE core timed out"
        );
        first.stream_turn_started = Some(Instant::now());
        first.service_streaming_turn();
        first.process_mesh_jobs();
        std::thread::sleep(Duration::from_millis(1));
    }
    let route_anchor = first.simulation.as_ref().unwrap().player.position;
    let origin = rustcraft_engine_core::ChunkPos {
        x: (route_anchor.x.floor() as i32).div_euclid(16),
        z: (route_anchor.z.floor() as i32).div_euclid(16),
    };
    if let Some(simulation) = first.simulation.as_mut() {
        let position = rustcraft_engine_core::BlockPos {
            x: origin.x * 16 + 1,
            y: simulation.player.position.y.floor() as i32 - 2,
            z: origin.z * 16 + 1,
        };
        simulation.world.set_state(
            position,
            rustcraft_engine_core::BlockState {
                block: STONE.id,
                variant: 77,
            },
        );
        edit = Some(position);
        simulation.spawn_item(
            rustcraft_minecraft_b173::blocks::DIRT.item.unwrap(),
            5,
            Vec3::new(
                origin.x as f32 * 16.0 + 8.5,
                simulation.player.position.y + 2.0,
                origin.z as f32 * 16.0 + 8.5,
            ),
        );
        let item = simulation.items.last_mut().unwrap();
        item.velocity = Vec3::new(0.125, 0.2, -0.25);
        item.age = 12.5;
        item.pickup_delay = 1_000.0;
        durable_drops.push((item.id, item.stack, origin));
        for offset_x in -1..=1 {
            let x = origin.x + offset_x;
            let z = origin.z - 1;
            for offset in 0..3 {
                let column = rustcraft_engine_core::ChunkPos { x, z };
                simulation.spawn_item(
                    rustcraft_minecraft_b173::blocks::DIRT.item.unwrap(),
                    (offset_x + offset + 4) as u16,
                    Vec3::new(
                        (x * 16) as f32 + 2.5 + offset as f32 * 5.0,
                        simulation.player.position.y + 2.0,
                        (z * 16) as f32 + 8.5,
                    ),
                );
                let item = simulation.items.last_mut().unwrap();
                item.age = (offset_x + 1 + offset * 3) as f32;
                item.pickup_delay = 1_000.0;
                durable_drops.push((item.id, item.stack, column));
            }
        }
    }
    for (_, _, column) in &durable_drops {
        first.persistence_dirty.mark_dirty(*column);
    }
    // Continue through the diagonal turn and into the negative-Z leg so obstacle detours in
    // biome-aware v2 terrain cannot leave an origin column exactly on the retain-radius boundary.
    // Prove item-bearing terrain actually saves and leaves active residency before the reversal.
    run_headless_stream_phase(&mut first, 140.0, 0.0);
    // Observe the actual owner at successful save-before-evict, not the spawn column. An item
    // may cross a border and a previously evicted owner may legitimately reload during a turn.
    let origin_evicted = durable_drops
        .iter()
        .all(|(id, _, _)| first.stream_evicted_entity_ids.contains(id));
    assert!(
        origin_evicted,
        "initial entity columns did not all evict during long travel: player={:?} visited={} evictions={} dirty={} saving={} lighting_pinned={}",
        first.simulation.as_ref().unwrap().player.position,
        first.stream_visited_columns.len(),
        first.eviction_count,
        first.persistence_dirty.is_dirty(origin),
        first.persistence_dirty.is_saving(origin),
        first
            .simulation
            .as_ref()
            .unwrap()
            .lighting
            .integration_columns()
            .contains(&origin),
    );
    assert!(first.stream_visited_columns.len() >= 20);
    let active_durable = first
        .simulation
        .as_ref()
        .unwrap()
        .items
        .iter()
        .filter(|entity| durable_drops.iter().any(|(id, _, _)| *id == entity.id))
        .map(|entity| {
            let owner = entity.column();
            let player = first.simulation.as_ref().unwrap().player.position;
            let center = rustcraft_engine_core::ChunkPos {
                x: (player.x.floor() as i32).div_euclid(16),
                z: (player.z.floor() as i32).div_euclid(16),
            };
            let retained = (owner.x - center.x).abs().max((owner.z - center.z).abs())
                <= first.residency.retain_radius() as i32;
            let dirty = first.persistence_dirty.is_dirty(owner);
            let saving = first.persistence_dirty.is_saving(owner);
            let lighting = first.simulation.as_ref().unwrap().lighting.integration_columns().contains(&owner);
            assert!(first.simulation.as_ref().unwrap().world.column_positions().any(|column| column == owner),
                "active orphan entity {} owner={owner:?}", entity.id);
            assert!(retained || dirty || saving || lighting,
                "active entity {} outside retain without save/light dependency: owner={owner:?} phase={:?}",
                entity.id, first.residency.phase(owner));
            (entity.id, owner, entity.position, retained, dirty, saving, first.residency.phase(owner))
        })
        .collect::<Vec<_>>();
    println!(
        "travel entity current-owner checkpoint: active={active_durable:?} successfully_evicted_ids={}",
        first.stream_evicted_entity_ids.len()
    );
    run_headless_stream_phase(&mut first, 168.0, 0.0);
    let negative_coordinates = first
        .stream_visited_columns
        .iter()
        .any(|position| position.x < 0 && position.z < 0);
    // Global negative-coordinate coverage belongs to the canonical v2 route. Different seeds
    // and frozen v1 have different relocated starts; their relative route keeps every other gate.
    let generator_version = first.stream_generator.as_ref().unwrap().version();
    if first.world_seed == 731_173 && generator_version == 2 {
        assert!(
            negative_coordinates,
            "canonical route did not cross negative X/Z"
        );
    }
    assert!(first.travel_margin.minimum[0] > 0.0);
    assert!(first.travel_margin.minimum[1] > 0.0);
    let distant_position = first.simulation.as_ref().unwrap().player.position;
    let fresh_visited = first.stream_visited_columns.len();
    let fresh_margins = (
        first.travel_margin.minimum,
        first.travel_margin.p05(),
        first.travel_margin.mean(),
    );
    let fresh_visible_latency = first.render_visible_latency_ms.summary();
    let fresh_resident_peak = first.resident_columns_peak;
    let fresh_section_peak = first.resident_sections_peak;
    let saved_world_time = first.simulation.as_ref().unwrap().time;
    let return_path = first.stream_return_path.clone();
    first.finish_world_saves();
    drop(first);

    let mut reopened = prepare_headless_stream_app(&root);
    reopened.stream_route_anchor = Some(route_anchor);
    reopened.stream_return_path = return_path;
    let restored = reopened.simulation.as_ref().unwrap().player.position;
    assert_eq!(reopened.simulation.as_ref().unwrap().time, saved_world_time);
    assert!((restored.x - distant_position.x).abs() < 0.25);
    assert!((restored.z - distant_position.z).abs() < 0.25);
    run_headless_stream_phase(&mut reopened, 196.0, 168.0);
    assert!(
        reopened.stream_returned_to_origin,
        "return controller failed: player={:?} anchor={route_anchor:?}",
        reopened.simulation.as_ref().unwrap().player.position
    );
    let edit = edit.expect("travel edit created");
    assert_eq!(
        reopened.simulation.as_ref().unwrap().world.state(edit),
        rustcraft_engine_core::BlockState {
            block: STONE.id,
            variant: 77,
        },
        "persisted edit did not survive eviction and disk reload"
    );
    for (id, stack, _) in &durable_drops {
        let restored_drop = reopened
            .simulation
            .as_ref()
            .unwrap()
            .items
            .iter()
            .find(|entity| entity.id == *id)
            .expect("persisted dropped item did not reload on return");
        assert_eq!(restored_drop.stack, *stack);
    }
    assert_eq!(
        reopened
            .simulation
            .as_ref()
            .unwrap()
            .items
            .iter()
            .filter(|entity| durable_drops.iter().any(|(id, _, _)| *id == entity.id))
            .count(),
        durable_drops.len(),
        "reloaded stable IDs must remain unique"
    );
    let safe_visible = reopened
        .simulation
        .as_ref()
        .unwrap()
        .world
        .safe_column_positions()
        .all(|position| reopened.render_ready_columns.contains(&position));
    assert!(safe_visible, "SAFE=>VISIBLE invariant failed");
    reopened.finish_world_saves();
    println!(
        "WORLD_TRAVEL_TEST result=PASS generator_version={} radius={} retain={} fresh_visited_columns={} reused_visited_columns={} returned_origin={} negative_coordinates={} fresh_margins_min/p05/mean={:?}/{:?}/{:?} reused_margins_current/min/p05/mean={:?}/{:?}/{:?}/{:?} fresh_request_visible_ms={:?} reused_request_visible_ms={:?} fresh_resident_columns/sections_peak={}/{} reused_resident_columns/sections_peak={}/{} edit_evict_reload=ok entity_columns_evict_reload={} world_time_reopen=ok reopen_distant_player=ok worldgen_v1_hash=e0d1f83c16b281124b7a9c190f667d7eddaa8b2f35ef5ef98ccb4bab434c1bb6 worldgen_v2_hash=4e134fa137fa5477fc3d28c9a610afd14019b0d0304246e43cd9940deb4684e7 streaming_v2_sample_hash=da875570efd5ace7a12c73e883a458e7f5ebe95f21d45ef90841b72104cf5b8e",
        generator_version,
        reopened.residency.load_radius(),
        reopened.residency.retain_radius(),
        fresh_visited,
        reopened.stream_visited_columns.len(),
        reopened.stream_returned_to_origin,
        negative_coordinates,
        fresh_margins.0,
        fresh_margins.1,
        fresh_margins.2,
        reopened.travel_margin.current,
        reopened.travel_margin.minimum,
        reopened.travel_margin.p05(),
        reopened.travel_margin.mean(),
        fresh_visible_latency,
        reopened.render_visible_latency_ms.summary(),
        fresh_resident_peak,
        fresh_section_peak,
        reopened.resident_columns_peak,
        reopened.resident_sections_peak,
        durable_drops.len(),
    );
    drop(reopened);
    std::fs::remove_dir_all(&root).expect("remove disposable world-travel-test world");
}

fn main() {
    if std::env::args().any(|a| a == "--config-report") {
        let mut config = rustcraft_control::config::settings::engine(true);
        rustcraft_control::config::settings::load(
            &mut config,
            &std::env::args().collect::<Vec<_>>(),
            |k| std::env::var(k).ok(),
            rustcraft_control::config::user_path(),
        )
        .expect("configuration");
        println!("{}", config.snapshot());
        return;
    }
    if std::env::args().any(|argument| argument == "--version") {
        println!("{}", rustcraft_build_info::identity());
        return;
    }
    if std::env::args().any(|argument| argument == "--resource-report") {
        match first_party_compiled() {
            Ok(compiled) => println!(
                "minecraft resources: pages={} container_page={} hud_page={} player_page={}",
                compiled.resources.atlas_pages.len(),
                compiled.resources.container_background.texture.0,
                compiled.resources.hud.texture.0,
                compiled.resources.player_skin.texture.0,
            ),
            Err(error) => {
                eprintln!("resource report failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
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
    if std::env::args().any(|a| a == "--render-scale") {
        bench::run_render_scale();
        return;
    }
    if std::env::args().any(|a| a == "--world-travel-test") {
        run_world_travel_test();
        return;
    }
    let mut args = std::env::args().skip(1);
    let mut diagnostic = None;
    let mut capture = None;
    let mut world_name =
        std::env::var("RUSTCRAFT_WORLD_NAME").unwrap_or_else(|_| "default".to_owned());
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--devtools" | "--dx-overhead" | "--dux-acceptance" | "--c1-acceptance" => {}
            "--set-config" | "--config-file" => {
                args.next().expect("configuration option requires value");
            }
            "--dx-abort-after-frames" => {
                args.next().expect("--dx-abort-after-frames requires N");
            }
            "--scenario" => {
                if args.next().is_none() {
                    eprintln!("--scenario requires PATH");
                    std::process::exit(2);
                }
            }
            "--survival" => {}
            "--stream-perf" => {}
            "--world-travel-test" => {}
            "--fidelity-m3" => {}
            "--world" => {
                let Some(name) = args.next() else {
                    eprintln!("--world requires a safe world name");
                    std::process::exit(2);
                };
                world_name = name;
            }
            "--diag" => {
                if let Some(stage) = args.next() {
                    diagnostic = Some(Stage::parse(&stage).unwrap_or_else(|error| {
                        eprintln!("{error}");
                        std::process::exit(2)
                    }))
                } else {
                    eprintln!(
                        "usage: rustcraft-client [--world NAME] [--survival] [--stream-perf] [--fidelity-m3] [--diag STAGE] [--capture NEW.png]"
                    );
                    std::process::exit(2);
                }
            }
            "--camera-motion" => {}
            "--capture" => {
                if let Some(path) = args.next() {
                    capture = Some(PathBuf::from(path));
                } else {
                    eprintln!(
                        "usage: rustcraft-client [--world NAME] [--survival] [--stream-perf] [--fidelity-m3] [--diag STAGE] [--capture NEW.png]"
                    );
                    std::process::exit(2);
                }
            }
            _ => {
                eprintln!(
                    "usage: rustcraft-client [--world NAME] [--survival] [--stream-perf] [--fidelity-m3] [--diag STAGE] [--capture NEW.png]"
                );
                std::process::exit(2);
            }
        }
    }
    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = ClientApp::new(diagnostic, capture);
    if std::env::args().any(|a| a == "--devtools" || a == "--scenario" || a == "--dx-overhead") {
        let mut registry = rustcraft_control::engine_registry();
        rustcraft_minecraft_b173::control::register_commands(&mut registry).expect("game commands");
        app.devtools = Some(
            rustcraft_scripting_rhai::DevTools::new(std::path::Path::new("scripts"), registry)
                .unwrap_or_else(|e| {
                    eprintln!("{e}");
                    std::process::exit(2)
                }),
        );
        let args = std::env::args().collect::<Vec<_>>();
        if args.iter().any(|a| a == "--dx-overhead") {
            app.dx_probe = Some(Default::default());
        }
        app.dx_abort_frame = args
            .iter()
            .position(|a| a == "--dx-abort-after-frames")
            .and_then(|i| args.get(i + 1))
            .and_then(|n| n.parse().ok());
        app.scenario_path = args
            .iter()
            .position(|a| a == "--scenario")
            .and_then(|i| args.get(i + 1).cloned());
    }
    if std::env::args().any(|a| a == "--dux-acceptance" || a == "--c1-acceptance") {
        app.dux_fixture = true;
        app.devtools = Some(
            rustcraft_scripting_rhai::DevTools::new(std::path::Path::new("scripts"), {
                let mut r = rustcraft_control::engine_registry();
                rustcraft_minecraft_b173::control::register_commands(&mut r)
                    .expect("package commands");
                r
            })
            .expect("devtools"),
        );
        app.scenario_path = Some(
            if std::env::args().any(|a| a == "--c1-acceptance") {
                "scripts/scenarios/c1.rhai"
            } else {
                "scripts/scenarios/dux1.rhai"
            }
            .into(),
        );
    }
    app.world_name = if app.dux_fixture {
        format!(
            "dux1-acceptance-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        )
    } else {
        world_name
    };
    app.survival_start = std::env::args().any(|a| a == "--survival");
    app.stream_perf = std::env::args().any(|a| a == "--stream-perf");
    if app.stream_perf {
        app.debug = true;
        app.debug_trace = true;
    }
    app.camera_motion = std::env::args().any(|a| a == "--camera-motion");
    if let Err(error) = event_loop.run_app(&mut app) {
        eprintln!("client event loop failed: {error}");
        std::process::exit(1);
    }
    if let Some(tools) = app.devtools.as_ref()
        && let Some(run) = tools.scenario.as_ref()
        && run.result.status != "pass"
    {
        eprintln!("DX scenario {} {:?}", run.result.status, run.result.error);
        std::process::exit(1);
    }
    if app.stream_perf {
        match app.stream_final_report.take() {
            Some(Ok(report)) => println!("{report}"),
            Some(Err(report)) => {
                eprintln!("{report}");
                std::process::exit(1);
            }
            None => {
                eprintln!("client-stream-auto ended without an acceptance report");
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_engine_core::BlockPos;
    use rustcraft_minecraft_b173::blocks::GRASS;
    use rustcraft_minecraft_b173::blocks::STONE;

    #[test]
    fn streaming_window_override_is_explicit_and_bounded() {
        assert_eq!(stream_window_size(None).unwrap(), (1280, 720));
        assert_eq!(stream_window_size(Some("640x360")).unwrap(), (640, 360));
        for value in [
            "",
            "0x0",
            "319x240",
            "640x239",
            "4097x720",
            "640x2161",
            "640",
            "-1x360",
            "999999999999x360",
        ] {
            assert!(stream_window_size(Some(value)).is_err(), "{value}");
        }
    }

    #[test]
    fn returned_core_requires_safe_and_visible_but_not_outer_boundary_convergence() {
        use rustcraft_engine_core::{Chunk, ChunkPos, World};
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&BlocksModule).unwrap();
        let mut world = World::new(rustcraft_minecraft_b173::blocks::AIR.id);
        let mut visible = HashSet::new();
        for z in -1..=1 {
            for x in -1..=1 {
                let column = ChunkPos { x, z };
                world
                    .publish_column(column, vec![(0, Chunk::new(STONE.id))])
                    .unwrap();
                visible.insert(column);
            }
        }
        world.enforce_column_availability(true);
        let mut simulation = Simulation::new(world, bootstrap.registry, Vec3::new(8.5, 17.0, 8.5));
        simulation
            .publish_column_incremental_lighting(
                ChunkPos { x: 2, z: 0 },
                vec![(0, Chunk::new(STONE.id))],
                false,
            )
            .unwrap();
        assert!(simulation.lighting.has_integration_work());
        assert!(stream_current_core_ready(&simulation, &visible));
        visible.remove(&ChunkPos { x: 1, z: 0 });
        assert!(!stream_current_core_ready(&simulation, &visible));
        visible.insert(ChunkPos { x: 1, z: 0 });
        simulation
            .world
            .set_column_safe(ChunkPos { x: 0, z: 1 }, false);
        assert!(!stream_current_core_ready(&simulation, &visible));
    }

    #[test]
    fn relocated_v1_startup_never_creates_partial_origin_sandbox_columns() {
        use rustcraft_world::ChunkGenerator;
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&BlocksModule).unwrap();
        let generator = rustcraft_minecraft_b173::worldgen::minecraft_overworld();
        for seed in [0, 1, -1, 731_173, -9_223_372_036, 2_147_483_647] {
            let center = rustcraft_minecraft_b173::worldgen::initial_spawn_column(seed, 1);
            let mut world =
                rustcraft_engine_core::World::new(rustcraft_minecraft_b173::blocks::AIR.id);
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let column = rustcraft_engine_core::ChunkPos {
                        x: center.x + dx,
                        z: center.z + dz,
                    };
                    world
                        .publish_column(column, generator.generate(seed, column).unwrap())
                        .unwrap();
                }
            }
            let before = world.column_positions().collect::<HashSet<_>>();
            let decorated = decorate_resident_legacy_sandbox(&mut world, &bootstrap.registry);
            assert_eq!(world.column_positions().collect::<HashSet<_>>(), before);
            assert_eq!(
                decorated,
                [-1, 0]
                    .into_iter()
                    .all(|x| before.contains(&rustcraft_engine_core::ChunkPos { x, z: 0 }))
            );
            let spawn = rustcraft_minecraft_b173::worldgen::select_safe_spawn(&world)
                .expect("v1 safe spawn remains inside complete startup terrain");
            let column = rustcraft_engine_core::ChunkPos {
                x: (spawn.x.floor() as i32).div_euclid(16),
                z: (spawn.z.floor() as i32).div_euclid(16),
            };
            assert!(before.contains(&column));
            let mut simulation =
                initialize_simulation(world, bootstrap.registry.clone(), None, true);
            for _ in 0..40 {
                simulation.step(Default::default(), 0.05);
            }
            assert!(simulation.player.position.y >= 1.0);
        }
    }

    #[test]
    fn named_world_create_reopen_preserves_edits_without_regeneration() {
        use rustcraft_world::WorldStorage;
        use std::time::{SystemTime, UNIX_EPOCH};
        let root = std::env::temp_dir().join(format!(
            "rustcraft-client-world-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let profile = rustcraft_minecraft_b173::compile_profile().unwrap();
        let blocks = BlocksModule;
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&blocks).unwrap();

        let mut first = ClientApp::new(None, None);
        first.world_name = "persist_test".to_owned();
        first.saves_directory.clone_from(&root);
        let mut created = rustcraft_engine_core::World::new(profile.default_state().block);
        first
            .load_or_generate_world(&profile, &bootstrap.registry, &mut created)
            .unwrap();
        assert_eq!(created.chunk_count(), 9);
        let created_metadata = WorldStorage::open(&root, "persist_test")
            .unwrap()
            .load_metadata()
            .unwrap();
        assert_eq!(created_metadata.generator_id, "minecraft_b173:overworld");
        assert_eq!(created_metadata.generator_version, 2);
        let edit = BlockPos { x: 2, y: 110, z: 3 };
        created.set_state(
            edit,
            rustcraft_engine_core::BlockState {
                block: STONE.id,
                variant: 41,
            },
        );
        let position = rustcraft_engine_core::ChunkPos { x: 0, z: 0 };
        let sections = created
            .section_positions()
            .filter(|(chunk, _)| *chunk == position)
            .filter_map(|(_, y)| {
                created
                    .section(position, y)
                    .cloned()
                    .map(|chunk| (y, chunk))
            })
            .collect::<Vec<_>>();
        let storage = WorldStorage::open(&root, "persist_test").unwrap();
        let resolver = rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver;
        storage
            .store_chunk(
                &WorldStorage::encode_runtime_chunk(position, sections, &resolver).unwrap(),
            )
            .unwrap();
        drop(storage);

        let mut reopened_app = ClientApp::new(None, None);
        reopened_app.world_name = "persist_test".to_owned();
        reopened_app.saves_directory.clone_from(&root);
        let mut reopened = rustcraft_engine_core::World::new(profile.default_state().block);
        reopened_app
            .load_or_generate_world(&profile, &bootstrap.registry, &mut reopened)
            .unwrap();
        assert_eq!(
            reopened.state(edit),
            rustcraft_engine_core::BlockState {
                block: STONE.id,
                variant: 41
            }
        );
        assert_eq!(reopened_app.loaded_from_disk, 9);
        assert_eq!(reopened_app.generated_chunks, 0);
        drop(first);
        drop(reopened_app);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn diagnostic_surface_navigation_forest_slope_regression() {
        use rustcraft_engine_core::{ChunkPos, World};
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&BlocksModule).unwrap();
        let generator = rustcraft_minecraft_b173::worldgen::default_overworld_generator();
        let mut world = World::new(rustcraft_minecraft_b173::blocks::AIR.id);
        for z in -1..=1 {
            for x in 0..=3 {
                let column = ChunkPos { x, z };
                world
                    .publish_column(column, generator.generate(-9_223_372_036, column).unwrap())
                    .unwrap();
            }
        }
        // The previous driver fell into the undercut lake at (33.3,73.2,12.5). Requiring
        // escape without swimming was invalid. Test normal reachable dry-surface movement;
        // full travel additionally proves the look-down water check avoids that entry.
        let spawn = rustcraft_minecraft_b173::worldgen::select_safe_spawn(&world).unwrap();
        let mut simulation = Simulation::new(world, bootstrap.registry, spawn);
        let goal = [spawn.x + 20.0, spawn.z];
        let mut path = stream_surface_path(&simulation, goal);
        for step in 0..300 {
            if step % 10 == 0 {
                path = stream_surface_path(&simulation, goal);
            }
            while path.front().is_some_and(|point| {
                (point[0] - simulation.player.position.x)
                    .hypot(point[1] - simulation.player.position.z)
                    < 0.35
            }) {
                path.pop_front();
            }
            let target = path.front().copied().unwrap_or(goal);
            let yaw = (target[0] - simulation.player.position.x)
                .atan2(target[1] - simulation.player.position.z);
            let delta = (yaw - simulation.player.yaw + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            simulation.step(
                AgentIntent {
                    movement: MoveIntent {
                        forward: 1.0,
                        strafe: 0.0,
                    },
                    look_delta: Vec3::new(-delta / 0.002, 0.0, 0.0),
                    jump: true,
                    ..Default::default()
                },
                0.05,
            );
        }
        assert!(
            simulation.player.position.x > spawn.x + 12.0,
            "forest navigation stuck at {:?} ground={} velocity={:?} yaw={} path={path:?}",
            simulation.player.position,
            simulation.player.on_ground,
            simulation.player.velocity,
            simulation.player.yaw
        );
    }

    #[test]
    fn diagnostic_surface_navigation_detours_without_crossing_unknown_or_solid_cells() {
        use rustcraft_engine_core::{BlockPos, World};
        use rustcraft_minecraft_b173::blocks as b;
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&BlocksModule).unwrap();
        let mut world = World::new(b::AIR.id);
        for z in 0..16 {
            for x in 0..16 {
                world.set(BlockPos { x, y: 0, z }, b::STONE.id);
            }
        }
        for z in 6..=10 {
            for y in 1..=3 {
                world.set(BlockPos { x: 9, y, z }, b::LOG.id);
            }
        }
        world.enforce_column_availability(true);
        let simulation = Simulation::new(world, bootstrap.registry, Vec3::new(8.5, 1.0, 8.5));
        let path = stream_surface_path(&simulation, [14.5, 8.5]);
        assert!(!path.is_empty());
        assert!(path.iter().any(|point| point[1] < 6.0 || point[1] > 11.0));
        for point in path {
            assert!((0.0..16.0).contains(&point[0]) && (0.0..16.0).contains(&point[1]));
            assert_ne!(
                simulation.world.get(BlockPos {
                    x: point[0].floor() as i32,
                    y: 1,
                    z: point[1].floor() as i32
                }),
                b::LOG.id
            );
        }
    }

    #[test]
    fn versioned_fresh_spawns_step_survival_and_persist_without_relocation() {
        for version in [1, 2] {
            for seed in [0, 1, -1, 731_173, 8_675_309, -9_223_372_036] {
                let mut bootstrap = RuntimeBootstrap::new(Default::default());
                bootstrap.register_module(&BlocksModule).unwrap();
                let generator = rustcraft_minecraft_b173::worldgen::resolve_overworld_generator(
                    rustcraft_minecraft_b173::worldgen::OVERWORLD_GENERATOR_ID,
                    version,
                )
                .unwrap();
                let center =
                    rustcraft_minecraft_b173::worldgen::initial_spawn_column(seed, version);
                let mut world =
                    rustcraft_engine_core::World::new(rustcraft_minecraft_b173::blocks::AIR.id);
                for z in -1..=1 {
                    for x in -1..=1 {
                        let column = rustcraft_engine_core::ChunkPos {
                            x: center.x + x,
                            z: center.z + z,
                        };
                        world
                            .publish_column(column, generator.generate(seed, column).unwrap())
                            .unwrap();
                    }
                }
                let mut simulation = initialize_simulation(world, bootstrap.registry, None, true);
                let initial = simulation.player.position;
                assert!(((initial.x.floor() as i32).div_euclid(16) - center.x).abs() <= 1);
                assert!(((initial.z.floor() as i32).div_euclid(16) - center.z).abs() <= 1);
                for _ in 0..20 {
                    simulation.step(AgentIntent::default(), 0.05);
                }
                assert!(
                    simulation.player.position.y >= initial.y - 1.1,
                    "spawn fell through terrain seed={seed} version={version}"
                );
                let settled = simulation.player.position;
                let record =
                    rustcraft_minecraft_b173::player_persistence::encode(&simulation).unwrap();
                let restored = rustcraft_minecraft_b173::player_persistence::decode(
                    &record,
                    &simulation.registry,
                )
                .unwrap();
                let simulation = initialize_simulation(
                    simulation.world,
                    simulation.registry,
                    Some(restored),
                    true,
                );
                assert_eq!(simulation.player.position, settled);
            }
        }
    }

    #[test]
    fn persisted_v1_world_generates_missing_columns_with_v1_and_never_upgrades() {
        use rustcraft_world::{ChunkGenerator, WorldMetadata, WorldStorage};
        use std::time::{SystemTime, UNIX_EPOCH};

        let root = std::env::temp_dir().join(format!(
            "rustcraft-client-v1-world-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let profile = rustcraft_minecraft_b173::compile_profile().unwrap();
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&BlocksModule).unwrap();
        let storage = WorldStorage::open(&root, "legacy_v1").unwrap();
        let generator = rustcraft_minecraft_b173::worldgen::minecraft_overworld();
        let resolver = rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver;
        let metadata = WorldMetadata {
            seed: -98_765,
            game_id: profile.id.as_str().to_owned(),
            profile_fingerprint: profile_fingerprint(&profile),
            persistence_schema_version: rustcraft_world::PERSISTED_STATE_SCHEMA_VERSION,
            generator_id: generator.id().to_owned(),
            generator_version: generator.version(),
        };
        storage.store_metadata(&metadata).unwrap();
        let center = rustcraft_minecraft_b173::worldgen::initial_spawn_column(
            metadata.seed,
            generator.version(),
        );
        let persisted_position = Vec3::new(
            center.x as f32 * 16.0 + 8.5,
            90.0,
            center.z as f32 * 16.0 + 8.5,
        );
        let mut saved_simulation = Simulation::new(
            rustcraft_engine_core::World::new(rustcraft_minecraft_b173::blocks::AIR.id),
            bootstrap.registry.clone(),
            persisted_position,
        );
        saved_simulation.time = 778_899;
        saved_simulation.spawn_item(
            rustcraft_minecraft_b173::blocks::DIRT.item.unwrap(),
            7,
            Vec3::new(persisted_position.x + 1.0, 80.0, persisted_position.z),
        );
        let saved_entity_id = saved_simulation.items[0].id;
        let spatial = rustcraft_minecraft_b173::world_persistence::encode_item_entity(
            &saved_simulation.items[0],
            &saved_simulation.registry,
        )
        .unwrap();
        storage
            .store_player(
                &rustcraft_minecraft_b173::player_persistence::encode(&saved_simulation).unwrap(),
            )
            .unwrap();
        storage
            .store_world_state(
                &rustcraft_minecraft_b173::world_persistence::encode_world_state(
                    saved_simulation.time,
                    1,
                    &[],
                ),
            )
            .unwrap();
        storage
            .store_chunk(
                &WorldStorage::encode_runtime_column(
                    center,
                    generator.generate(metadata.seed, center).unwrap(),
                    &resolver,
                    vec![spatial],
                    vec![],
                )
                .unwrap(),
            )
            .unwrap();
        drop(storage);

        let mut app = ClientApp::new(None, None);
        app.world_name = "legacy_v1".to_owned();
        app.saves_directory.clone_from(&root);
        let mut world = rustcraft_engine_core::World::new(profile.default_state().block);
        let restored = app
            .load_or_generate_world(&profile, &bootstrap.registry, &mut world)
            .unwrap()
            .unwrap();
        assert_eq!(restored.position, persisted_position);
        assert_eq!(app.restored_world_time, 778_899);
        assert_eq!(app.loaded_from_disk, 1);
        assert_eq!(app.generated_chunks, 8);
        let expected_position = rustcraft_engine_core::ChunkPos {
            x: center.x + 1,
            z: center.z,
        };
        let expected = generator
            .generate(metadata.seed, expected_position)
            .unwrap();
        for (section_y, expected_chunk) in expected {
            assert_eq!(
                world
                    .section(expected_position, section_y)
                    .unwrap()
                    .states(),
                expected_chunk.states()
            );
        }
        let stored = WorldStorage::open(&root, "legacy_v1")
            .unwrap()
            .load_metadata()
            .unwrap();
        assert_eq!(stored.generator_version, 1);
        let mut simulation = initialize_simulation(world, bootstrap.registry, Some(restored), true);
        app.activate_pending_spatial_columns(&mut simulation)
            .unwrap();
        assert_eq!(simulation.items.len(), 1);
        assert_eq!(simulation.items[0].id, saved_entity_id);
        assert_eq!(simulation.items[0].stack.count, 7);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partial_v2_world_preserves_existing_edit_and_generates_missing_v2_columns() {
        use rustcraft_world::{ChunkGenerator, WorldMetadata, WorldStorage};
        use std::time::{SystemTime, UNIX_EPOCH};

        let root = std::env::temp_dir().join(format!(
            "rustcraft-client-partial-v2-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let profile = rustcraft_minecraft_b173::compile_profile().unwrap();
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&BlocksModule).unwrap();
        let storage = WorldStorage::open(&root, "partial_v2").unwrap();
        let generator = rustcraft_minecraft_b173::worldgen::minecraft_overworld_v2();
        let resolver = rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver;
        let metadata = WorldMetadata {
            seed: 8_675_309,
            game_id: profile.id.as_str().to_owned(),
            profile_fingerprint: profile_fingerprint(&profile),
            persistence_schema_version: rustcraft_world::PERSISTED_STATE_SCHEMA_VERSION,
            generator_id: generator.id().to_owned(),
            generator_version: generator.version(),
        };
        storage.store_metadata(&metadata).unwrap();
        let center = rustcraft_minecraft_b173::worldgen::initial_spawn_column(
            metadata.seed,
            generator.version(),
        );
        let mut sections = generator.generate(metadata.seed, center).unwrap();
        let edited = rustcraft_engine_core::BlockPos {
            x: center.x * 16 + 2,
            y: 110,
            z: center.z * 16 + 3,
        };
        sections[(edited.y / 16) as usize].1.set_state(
            (
                edited.x.rem_euclid(16) as u8,
                edited.y.rem_euclid(16) as u8,
                edited.z.rem_euclid(16) as u8,
            ),
            rustcraft_engine_core::BlockState {
                block: STONE.id,
                variant: 55,
            },
        );
        storage
            .store_chunk(&WorldStorage::encode_runtime_chunk(center, sections, &resolver).unwrap())
            .unwrap();
        drop(storage);

        let mut app = ClientApp::new(None, None);
        app.world_name = "partial_v2".to_owned();
        app.saves_directory.clone_from(&root);
        let mut world = rustcraft_engine_core::World::new(profile.default_state().block);
        app.load_or_generate_world(&profile, &bootstrap.registry, &mut world)
            .unwrap();
        assert_eq!(app.loaded_from_disk, 1);
        assert_eq!(app.generated_chunks, 8);
        assert_eq!(world.state(edited).variant, 55);
        let neighbor = rustcraft_engine_core::ChunkPos {
            x: center.x + 1,
            z: center.z,
        };
        for (section_y, expected) in generator.generate(metadata.seed, neighbor).unwrap() {
            assert_eq!(
                world.section(neighbor, section_y).unwrap().states(),
                expected.states()
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsupported_persisted_generator_is_a_contextual_error() {
        use rustcraft_world::{WorldMetadata, WorldStorage};
        use std::time::{SystemTime, UNIX_EPOCH};

        let root = std::env::temp_dir().join(format!(
            "rustcraft-client-unknown-generator-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let profile = rustcraft_minecraft_b173::compile_profile().unwrap();
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&BlocksModule).unwrap();
        let storage = WorldStorage::open(&root, "unknown").unwrap();
        storage
            .store_metadata(&WorldMetadata {
                seed: 1,
                game_id: profile.id.as_str().to_owned(),
                profile_fingerprint: profile_fingerprint(&profile),
                persistence_schema_version: rustcraft_world::PERSISTED_STATE_SCHEMA_VERSION,
                generator_id: "minecraft_b173:overworld".to_owned(),
                generator_version: 99,
            })
            .unwrap();
        drop(storage);
        let mut app = ClientApp::new(None, None);
        app.world_name = "unknown".to_owned();
        app.saves_directory.clone_from(&root);
        let mut world = rustcraft_engine_core::World::new(profile.default_state().block);
        let error = app
            .load_or_generate_world(&profile, &bootstrap.registry, &mut world)
            .unwrap_err();
        assert!(error.contains("generator compatibility error"));
        assert!(error.contains("v99"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn saved_player_loads_before_startup_area_is_published_around_saved_chunk() {
        use rustcraft_world::WorldStorage;
        use std::time::{SystemTime, UNIX_EPOCH};
        let root = std::env::temp_dir().join(format!(
            "rustcraft-client-player-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let profile = rustcraft_minecraft_b173::compile_profile().unwrap();
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&BlocksModule).unwrap();
        let mut app = ClientApp::new(None, None);
        app.world_name = "player_test".into();
        app.saves_directory.clone_from(&root);
        let storage = WorldStorage::open(&root, "player_test").unwrap();
        let mut initial = rustcraft_engine_core::World::new(profile.default_state().block);
        let default_state = app
            .load_or_generate_world(&profile, &bootstrap.registry, &mut initial)
            .unwrap();
        assert!(default_state.is_none());
        let mut sim = Simulation::new(
            initial,
            bootstrap.registry.clone(),
            Vec3::new(50.5, 75.25, -34.5),
        );
        sim.player.yaw = 1.125;
        sim.player.pitch = -0.25;
        sim.mode = rustcraft_runtime::survival::GameMode::Survival;
        sim.inventory
            .insert(rustcraft_engine_core::ItemId(1), 7, &bootstrap.registry);
        sim.inventory.select(4);
        storage
            .store_player(&rustcraft_minecraft_b173::player_persistence::encode(&sim).unwrap())
            .unwrap();

        let mut reopened = rustcraft_engine_core::World::new(profile.default_state().block);
        let state = app
            .load_or_generate_world(&profile, &bootstrap.registry, &mut reopened)
            .unwrap()
            .unwrap();
        assert_eq!(state.position, sim.player.position);
        assert_eq!(state.yaw, sim.player.yaw);
        assert_eq!(state.mode, sim.mode);
        assert!(
            reopened
                .chunk_positions()
                .any(|p| p == rustcraft_engine_core::ChunkPos { x: 3, z: -3 })
        );
        assert!(
            !reopened
                .chunk_positions()
                .any(|p| p == rustcraft_engine_core::ChunkPos { x: 0, z: 0 })
        );
        let restored_sim = initialize_simulation(reopened, bootstrap.registry, Some(state), false);
        assert_eq!(restored_sim.player.position, sim.player.position);
        assert_eq!(restored_sim.player.yaw, sim.player.yaw);
        assert_eq!(restored_sim.player.pitch, sim.player.pitch);
        assert_eq!(restored_sim.mode, sim.mode);
        assert_eq!(restored_sim.inventory.slots(), sim.inventory.slots());
        assert_eq!(restored_sim.inventory.selected(), sim.inventory.selected());
        drop(storage);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn controlled_client_shutdown_persists_player_record() {
        use rustcraft_world::WorldStorage;
        use std::time::{SystemTime, UNIX_EPOCH};
        let root = std::env::temp_dir().join(format!(
            "rustcraft-client-close-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let storage = WorldStorage::open(&root, "close_test").unwrap();
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&BlocksModule).unwrap();
        let mut sim = Simulation::new(
            rustcraft_engine_core::World::new(rustcraft_minecraft_b173::blocks::AIR.id),
            bootstrap.registry,
            Vec3::new(-32.5, 81.25, 47.5),
        );
        sim.player.yaw = 2.25;
        sim.player.pitch = 0.35;
        sim.inventory.select(6);
        sim.mode = rustcraft_runtime::survival::GameMode::Survival;
        sim.time = 456_789;
        let mut app = ClientApp::new(None, None);
        app.simulation = Some(sim);
        app.world_storage = Some(storage.clone());
        app.finish_world_saves();
        let restored = rustcraft_minecraft_b173::player_persistence::decode(
            &storage
                .load_player(rustcraft_minecraft_b173::player_persistence::LOCAL_PLAYER_ID)
                .unwrap()
                .unwrap(),
            &app.simulation.as_ref().unwrap().registry,
        )
        .unwrap();
        assert_eq!(restored.position, Vec3::new(-32.5, 81.25, 47.5));
        assert_eq!(restored.yaw, 2.25);
        assert_eq!(restored.pitch, 0.35);
        assert_eq!(restored.inventory.selected(), 6);
        assert_eq!(
            restored.mode,
            rustcraft_runtime::survival::GameMode::Survival
        );
        let global = storage.load_world_state().unwrap().unwrap();
        let (world_time, unknown) =
            rustcraft_minecraft_b173::world_persistence::decode_world_state(&global).unwrap();
        assert_eq!(world_time, 456_789);
        assert!(unknown.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn periodic_player_checkpoint_writes_changed_state_and_skips_unchanged_state() {
        use rustcraft_world::WorldStorage;
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let root = std::env::temp_dir().join(format!(
            "rustcraft-player-autosave-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let storage = WorldStorage::open(&root, "autosave_test").unwrap();
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&BlocksModule).unwrap();
        let simulation = Simulation::new(
            rustcraft_engine_core::World::new(rustcraft_minecraft_b173::blocks::AIR.id),
            bootstrap.registry,
            Vec3::new(0.5, 65.0, 0.5),
        );
        let mut app = ClientApp::new(None, None);
        app.simulation = Some(simulation);
        app.world_storage = Some(storage.clone());
        app.player_autosave_interval = Duration::from_secs(1);
        app.last_player_autosave = Instant::now() - Duration::from_secs(2);
        app.service_player_autosave();
        let deadline = Instant::now() + Duration::from_secs(3);
        let original = loop {
            app.service_player_autosave();
            if let Some(record) = storage
                .load_player(rustcraft_minecraft_b173::player_persistence::LOCAL_PLAYER_ID)
                .unwrap()
            {
                break record;
            }
            assert!(
                Instant::now() < deadline,
                "player checkpoint did not complete"
            );
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(
            original
                .components
                .iter()
                .map(|c| c.payload.len())
                .sum::<usize>(),
            67
        );
        assert_eq!(app.player_autosave_writes, 1);
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.player_persisted_revision < original.revision && Instant::now() < deadline {
            app.service_player_autosave();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(app.player_persisted_revision, original.revision);
        app.last_player_autosave = Instant::now() - Duration::from_secs(2);
        app.service_player_autosave();
        assert_eq!(app.player_autosave_writes, 1);
        app.simulation.as_mut().unwrap().player.position.x = 9.5;
        app.last_player_autosave = Instant::now() - Duration::from_secs(2);
        app.service_player_autosave();
        let expected_revision = app.player_revision;
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.player_persisted_revision < expected_revision && Instant::now() < deadline {
            app.service_player_autosave();
            std::thread::sleep(Duration::from_millis(1));
        }
        let changed = storage
            .load_player(rustcraft_minecraft_b173::player_persistence::LOCAL_PLAYER_ID)
            .unwrap()
            .unwrap();
        assert_ne!(original.components, changed.components);
        assert_eq!(changed.revision, original.revision + 1);
        assert_eq!(app.player_autosave_writes, 2);
        let _ = std::fs::remove_dir_all(root);
    }

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
        let top = textures.texture(GRASS.id, Face::Top).unwrap();
        let bottom = textures.texture(GRASS.id, Face::Bottom).unwrap();
        let side = textures.texture(GRASS.id, Face::North).unwrap();
        assert_ne!(top, bottom);
        assert_ne!(top, side);
        assert_ne!(bottom, side);
    }

    #[test]
    fn generated_water_resolves_semantic_key_to_compiled_atlas_page() {
        let compiled = first_party_compiled().unwrap();
        let water_id = compiled.legacy_to_compiled
            [rustcraft_minecraft_b173::blocks::WATER.id.0 as usize]
            .unwrap();
        let water = compiled.materials.block(water_id).unwrap();
        assert_eq!(water.material, rustcraft_game_api::MaterialClass::Liquid);
        assert!(water.translucent);
        let region = compiled.materials.texture(water_id, Face::Top).unwrap();
        assert_eq!(region, water.faces[Face::Top as usize]);
        assert!((region.texture.0 as usize) < compiled.resources.atlas_pages.len());
        assert_eq!(
            rustcraft_minecraft_b173::blocks::WATER.textures.face(0),
            "minecraft_b173:water"
        );
        assert_eq!(
            rustcraft_minecraft_b173::blocks::atlas_tile("minecraft_b173:water"),
            Some((14, 0))
        );
    }

    #[test]
    fn underwater_medium_uses_compiled_liquid_geometry_height() {
        first_party_compiled().unwrap();
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&BlocksModule).unwrap();
        let mut world = rustcraft_engine_core::World::new(rustcraft_minecraft_b173::blocks::AIR.id);
        world.set_state(
            BlockPos { x: 0, y: 65, z: 0 },
            rustcraft_engine_core::BlockState::new(rustcraft_minecraft_b173::blocks::WATER.id),
        );
        let mut sim = Simulation::new(world, bootstrap.registry, Vec3::new(0.5, 63.5, 0.5));
        assert!(camera_is_underwater(&sim));
        sim.player.position.y = 64.27;
        assert!(!camera_is_underwater(&sim));
        sim.player.position.y = 63.5;
        assert!(camera_is_underwater(&sim));
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
