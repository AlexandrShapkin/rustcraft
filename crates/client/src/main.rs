mod render_tests;
use rustcraft_agent_api::{AgentIntent, Controller, MoveIntent};
use rustcraft_engine_core::{BlockId, Vec3};
mod bench;
mod debug;
mod gpu_metrics;
use rustcraft_minecraft_b173::blocks::BlocksModule;
use rustcraft_render::diagnostic::Stage;
use rustcraft_render::{
    AtlasRegion, BlockTextureResolver, Camera, Face, RenderWorld, Renderer, RendererResources,
    RgbaTexture, TextureSampling,
};
use rustcraft_render_profile::{CompiledTextureRegistry, CompiledVoxelRenderRegistry};
use rustcraft_runtime::{RuntimeBootstrap, Simulation};
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
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

static FIRST_PARTY_COMPILED: OnceLock<Result<FirstPartyCompiled, String>> = OnceLock::new();

fn first_party_compiled() -> Result<&'static FirstPartyCompiled, String> {
    FIRST_PARTY_COMPILED
        .get_or_init(compile_first_party_resources)
        .as_ref()
        .map_err(Clone::clone)
}

fn compile_first_party_resources() -> Result<FirstPartyCompiled, String> {
    let terrain = std::env::var_os("RUSTCRAFT_TERRAIN_TEXTURE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("reference/assets/terrain.png")
        });
    let package = rustcraft_minecraft_b173::legacy_resource_package(&terrain)
        .map_err(|error| format!("legacy minecraft resource import: {error}"))?;
    let cache_directory = std::env::var_os("RUSTCRAFT_RESOURCE_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/resource-cache/minecraft-b173"));
    let mut atlas_policy = rustcraft_content::resources::AtlasPolicy::default();
    if let Some(value) = std::env::var_os("RUSTCRAFT_MAX_ATLAS_DIMENSION") {
        let maximum = value
            .to_str()
            .ok_or("RUSTCRAFT_MAX_ATLAS_DIMENSION is not UTF-8")?
            .parse::<u32>()
            .map_err(|error| format!("invalid RUSTCRAFT_MAX_ATLAS_DIMENSION: {error}"))?;
        atlas_policy = atlas_policy
            .bounded_by_device_dimension(maximum)
            .map_err(|error| format!("device-safe atlas policy: {error}"))?;
    }
    let compiled = rustcraft_content::resources::compile_resources(
        &[package],
        atlas_policy,
        rustcraft_content::resources::ResourceLimits::default(),
        Some(&cache_directory),
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
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    simulation: Option<Simulation>,
    presentation: Option<RenderWorld>,
    world_storage: Option<rustcraft_world::WorldStorage>,
    persistence_dirty: rustcraft_world::PersistenceDirtyTracker,
    save_scheduler: rustcraft_world::SaveScheduler,
    world_name: String,
    saves_directory: PathBuf,
    worldgen_metrics: rustcraft_world::GenerationMetrics,
    loaded_from_disk: usize,
    generated_chunks: usize,
    controller: LocalHumanController,
    clock: rustcraft_runtime::metrics::FixedStepClock,
    last_frame: Instant,
    resources: Result<RendererResources, String>,
    diagnostic: Option<Stage>,
    capture: Option<PathBuf>,
    debug: bool,
    metrics: rustcraft_runtime::metrics::Metrics,
    process: rustcraft_runtime::metrics::ProcessSampler,
    last_render: Instant,
    debug_text: String,
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
    player_autosave_writes: u64,
    gpu_metrics: Option<gpu_metrics::Provider>,
    measure_seconds: Option<f64>,
    survival_start: bool,
    inventory_open: bool,
    cursor_position: [f32; 2],
    mesh_scheduler: rustcraft_render::meshing::MeshScheduler,
    mesh_upload_section_budget: usize,
    mesh_upload_byte_budget: usize,
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
}
impl ClientApp {
    fn new(diagnostic: Option<Stage>, capture: Option<PathBuf>) -> Self {
        let mesh_workers = std::env::var("RUSTCRAFT_MESH_WORKERS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map_or(1, usize::from)
                    .saturating_sub(1)
                    .clamp(1, 8)
            })
            .clamp(1, 32);
        Self {
            window: None,
            renderer: None,
            simulation: None,
            presentation: None,
            world_storage: None,
            persistence_dirty: Default::default(),
            save_scheduler: rustcraft_world::SaveScheduler::new(1, 8),
            world_name: "default".to_owned(),
            saves_directory: std::env::var_os("RUSTCRAFT_SAVES_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("saves")),
            worldgen_metrics: Default::default(),
            loaded_from_disk: 0,
            generated_chunks: 0,
            controller: LocalHumanController::default(),
            clock: Default::default(),
            last_frame: Instant::now(),
            resources: minecraft_renderer_resources(),
            diagnostic,
            capture,
            debug: std::env::var_os("RUSTCRAFT_F3").is_some(),
            metrics: Default::default(),
            process: Default::default(),
            last_render: Instant::now(),
            debug_text: String::new(),
            last_snapshot: Instant::now() - std::time::Duration::from_secs(1),
            last_player_autosave: Instant::now(),
            player_autosave_interval: Duration::from_secs(
                std::env::var("RUSTCRAFT_PLAYER_AUTOSAVE_SECONDS")
                    .ok()
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or(2)
                    .clamp(1, 2),
            ),
            player_components: None,
            unknown_player_components: Vec::new(),
            player_revision: 0,
            player_persisted_revision: 0,
            player_dirty: false,
            latest_player_record: None,
            player_encode_ms_last: 0.0,
            player_checkpoint_ms_last: 0.0,
            player_save_scheduler: rustcraft_world::PlayerSaveScheduler::new(),
            player_autosave_writes: 0,
            gpu_metrics: None,
            measure_seconds: std::env::var("RUSTCRAFT_MEASURE_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok()),
            survival_start: false,
            inventory_open: false,
            cursor_position: [0.; 2],
            mesh_scheduler: rustcraft_render::meshing::MeshScheduler::new(
                mesh_workers,
                mesh_workers * 2,
                FirstPartyTextures,
            ),
            mesh_upload_section_budget: std::env::var("RUSTCRAFT_MESH_UPLOAD_SECTIONS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(4),
            mesh_upload_byte_budget: std::env::var("RUSTCRAFT_MESH_UPLOAD_BYTES")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(8 * 1024 * 1024),
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
        let simulation =
            initialize_simulation(world, bootstrap.registry, restored, self.survival_start);
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
        let generator: Arc<dyn ChunkGenerator> =
            Arc::new(rustcraft_minecraft_b173::worldgen::minecraft_overworld());
        let storage = WorldStorage::open(&self.saves_directory, &self.world_name)
            .map_err(|error| error.to_string())?;
        let seed = match std::env::var("RUSTCRAFT_WORLD_SEED") {
            Ok(value) => value
                .parse::<i64>()
                .map_err(|_| "RUSTCRAFT_WORLD_SEED must be a signed 64-bit integer".to_owned())?,
            Err(std::env::VarError::NotPresent) => 731_173,
            Err(error) => return Err(format!("RUSTCRAFT_WORLD_SEED: {error}")),
        };
        let expected = WorldMetadata {
            seed,
            game_id: profile.id.as_str().to_owned(),
            profile_fingerprint: profile_fingerprint(profile),
            persistence_schema_version: rustcraft_world::PERSISTED_STATE_SCHEMA_VERSION,
            generator_id: generator.id().to_owned(),
            generator_version: generator.version(),
        };
        let new_world = !storage.has_metadata();
        let saved = if new_world {
            storage.store_metadata(&expected).map_err(|error| {
                format!(
                    "create world metadata at {}: {error}",
                    storage.root().display()
                )
            })?;
            None
        } else {
            let found = storage.load_metadata().map_err(|error| {
                format!("world metadata at {}: {error}", storage.root().display())
            })?;
            Some(found)
        };

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

        let (center_x, center_z) = restored.as_ref().map_or((0, 0), |state| {
            (
                (state.position.x.floor() as i32).div_euclid(16),
                (state.position.z.floor() as i32).div_euclid(16),
            )
        });
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
        let resolver = rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver;
        let mut missing = Vec::new();
        let mut loaded_from_disk = 0;
        for position in positions.iter().copied() {
            if storage.chunk_exists(position) {
                let sections =
                    storage
                        .load_runtime_chunk(position, &resolver)
                        .map_err(|error| {
                            format!(
                                "load chunk ({},{}) in world {}: {error}",
                                position.x,
                                position.z,
                                storage.root().display()
                            )
                        })?;
                world
                    .publish_column(position, sections)
                    .map_err(str::to_owned)?;
                loaded_from_disk += 1;
            } else {
                missing.push(position);
            }
        }

        if let Some(found) = &saved {
            validate_world_compatibility(found, &expected, !missing.is_empty()).map_err(|error| {
                format!(
                    "world compatibility error at {}: {error}; saved seed={}, active requested seed={}. Existing saved worlds are not deleted; use a new ignored development world to test incompatible content.",
                    storage.root().display(), found.seed, seed
                )
            })?;
        }
        let generation_seed = saved
            .as_ref()
            .map_or(expected.seed, |metadata| metadata.seed);
        let mut scheduler = GenerationScheduler::new(2, 8);
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
        self.loaded_from_disk = loaded_from_disk;
        self.generated_chunks = generated;
        self.worldgen_metrics = scheduler.metrics();
        let decorated = new_world && self.diagnostic.is_none();
        if decorated {
            let base_y = Simulation::spawn_above_surface(world, registry, 0, 0).y as i32 - 2;
            rustcraft_minecraft_b173::flat_world::decorate_sandbox_at(world, registry, base_y);
        }
        let persist_positions = if decorated { positions } else { missing };
        for position in persist_positions {
            let sections = world
                .section_positions()
                .filter(|(p, _)| *p == position)
                .filter_map(|(_, y)| world.section(position, y).cloned().map(|chunk| (y, chunk)))
                .collect::<Vec<_>>();
            let stored = WorldStorage::encode_runtime_chunk(position, sections, &resolver)
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
                self.last_player_autosave = Instant::now();
                self.player_autosave_writes = self.player_autosave_writes.saturating_add(1);
            }
            Err(error) => eprintln!("player checkpoint submission failed: {error}"),
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
    fn fixed_step(&mut self) {
        let tick_started = Instant::now();
        let (persistence_dirty, dirty) = {
            let Some(simulation) = self.simulation.as_mut() else {
                return;
            };
            let intent = if self.inventory_open {
                rustcraft_agent_api::AgentIntent::default()
            } else {
                self.controller.next_intent()
            };
            simulation.step(intent, 0.05);
            (
                simulation.take_persistence_dirty_chunks(),
                simulation.take_dirty_sections(),
            )
        };
        self.metrics.tick(tick_started.elapsed().as_secs_f64());
        self.service_world_saves(persistence_dirty);
        self.service_player_autosave();
        if let (Some(presentation), Some(simulation)) =
            (self.presentation.as_mut(), self.simulation.as_ref())
        {
            presentation.sync_sections(&simulation.world, dirty.iter().copied());
        }
        self.rebuild_dirty_meshes(dirty);
    }

    fn service_world_saves(&mut self, dirty: Vec<rustcraft_engine_core::ChunkPos>) {
        use rustcraft_world::WorldStorage;
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
        let resolver = rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver;
        let result = WorldStorage::encode_runtime_chunk(position, sections, &resolver)
            .map_err(|error| error.to_string())
            .and_then(|chunk| {
                self.save_scheduler
                    .submit(storage.clone(), token, chunk)
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = result {
            eprintln!(
                "unable to queue world save for ({},{}): {error}",
                position.x, position.z
            );
            self.persistence_dirty.complete_save(token, false);
        }
    }

    fn finish_world_saves(&mut self) {
        use rustcraft_world::WorldStorage;
        let Some(storage) = self.world_storage.clone() else {
            return;
        };
        if let Some(simulation) = self.simulation.as_mut() {
            for position in simulation.take_persistence_dirty_chunks() {
                self.persistence_dirty.mark_dirty(position);
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
                let deadline = Instant::now() + Duration::from_secs(60);
                while self.player_persisted_revision < record.revision && Instant::now() < deadline
                {
                    for completion in self.player_save_scheduler.take_completed() {
                        if completion.result.is_ok() {
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
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        while self.persistence_dirty.metrics().saves_in_flight > 0 && Instant::now() < deadline {
            for completion in self.save_scheduler.take_completed() {
                let success = completion.result.is_ok();
                if let Err(error) = completion.result {
                    eprintln!("world save failed during shutdown: {error}");
                }
                self.persistence_dirty
                    .complete_save(completion.token, success);
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
            let resolver = rustcraft_minecraft_b173::worldgen::MinecraftLegacyBlockResolver;
            let result = WorldStorage::encode_runtime_chunk(position, sections, &resolver)
                .and_then(|chunk| storage.store_chunk_measured(&chunk).map(|_| ()));
            self.persistence_dirty.complete_save(token, result.is_ok());
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
        let mut snapshots = presentation.chunks().cloned().collect::<Vec<_>>();
        snapshots.sort_by_key(|chunk| (chunk.position.x, chunk.position.z, chunk.section_y));
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
            if let Some(snapshot) = presentation
                .chunks()
                .find(|chunk| chunk.position == position && chunk.section_y == section_y)
                .cloned()
            {
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
        let camera = self
            .simulation
            .as_ref()
            .map_or(Vec3::ZERO, |simulation| simulation.player.position);
        let completed = self.mesh_scheduler.take_ready(
            camera,
            self.mesh_upload_section_budget,
            self.mesh_upload_byte_budget,
        );
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        for result in completed {
            self.metrics.mesh(result.mesh_ms / 1000.0);
            let upload =
                renderer.upload_chunk_pages(result.section.0, result.section.1, &result.pages);
            self.mesh_uploads += 1;
            self.mesh_upload_bytes += upload.logical_bytes as u64;
            self.mesh_upload_submit_ms += upload.submit_ms;
            self.mesh_uploads_this_frame += 1;
            self.mesh_upload_bytes_this_frame += upload.logical_bytes;
        }
    }
    fn render(&mut self, event_loop: &ActiveEventLoop) {
        self.metrics
            .frames
            .push(self.last_render.elapsed().as_secs_f64());
        self.last_render = Instant::now();
        self.metrics.update();
        self.process_mesh_jobs();
        let mesh_stats = self.mesh_scheduler.stats();
        let mesh_idle = self.mesh_scheduler.is_idle();
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
        if self.debug || self.measure_seconds.is_some() {
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
                self.debug_text.push_str(&format!(
                    "\nWORLD {} SEED {} RESIDENT CHUNKS {} SECTIONS {} GENERATED {} LOADED {}\nGEN PENDING {} INFLIGHT {} COMPLETE {} STALE {} COALESCED {} TOTAL {} MS\nSAVE DIRTY {} QUEUED {} INFLIGHT {} SAVED {} FAILED {}\nPLAYER SAVE revision={} persisted={} dirty={} requests={} coalesced={} pending={} inflight={} successes={} failures={} encode_ms={:.3} checkpoint_ms={:.3}",
                    self.world_name,
                    std::env::var("RUSTCRAFT_WORLD_SEED").unwrap_or_else(|_| "731173".to_owned()),
                    sim.world.chunk_count(), sim.world.section_count(), self.generated_chunks, self.loaded_from_disk,
                    self.worldgen_metrics.pending, self.worldgen_metrics.in_flight, self.worldgen_metrics.completed,
                    self.worldgen_metrics.stale_discarded, self.worldgen_metrics.coalesced, self.worldgen_metrics.generation_total_ms,
                    persistence.dirty_chunks, saves.queued, saves.in_flight, persistence.saved, persistence.failed,
                    self.player_revision, self.player_persisted_revision, self.player_dirty,
                    player_saves.requests, player_saves.coalesced, player_saves.pending,
                    player_saves.in_flight, player_saves.successes, player_saves.failures,
                    self.player_encode_ms_last, self.player_checkpoint_ms_last,
                ));
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
            .is_some_and(|s| self.metrics.uptime.elapsed().as_secs_f64() >= s)
            && mesh_idle;
        let capture = if mesh_idle && (self.measure_seconds.is_none() || finished) {
            self.capture.as_deref()
        } else {
            None
        };
        match renderer.render_capture(camera, capture) {
            Ok(()) => {
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
        self.finish_world_saves();
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
        self.camera_motion_start = Instant::now();
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
    let spawn = Simulation::spawn_above_surface(&world, &registry, center.0 * 16, center.1 * 16);
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
    let Some(compiled) = FIRST_PARTY_COMPILED.get().and_then(|r| r.as_ref().ok()) else {
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
fn main() {
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
    let mut args = std::env::args().skip(1);
    let mut diagnostic = None;
    let mut capture = None;
    let mut world_name =
        std::env::var("RUSTCRAFT_WORLD_NAME").unwrap_or_else(|_| "default".to_owned());
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--survival" => {}
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
                        "usage: rustcraft-client [--world NAME] [--survival] [--fidelity-m3] [--diag STAGE] [--capture NEW.png]"
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
                        "usage: rustcraft-client [--world NAME] [--survival] [--fidelity-m3] [--diag STAGE] [--capture NEW.png]"
                    );
                    std::process::exit(2);
                }
            }
            _ => {
                eprintln!(
                    "usage: rustcraft-client [--world NAME] [--survival] [--fidelity-m3] [--diag STAGE] [--capture NEW.png]"
                );
                std::process::exit(2);
            }
        }
    }
    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = ClientApp::new(diagnostic, capture);
    app.world_name = world_name;
    app.survival_start = std::env::args().any(|a| a == "--survival");
    app.camera_motion = std::env::args().any(|a| a == "--camera-motion");
    let _ = event_loop.run_app(&mut app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_engine_core::BlockPos;
    use rustcraft_minecraft_b173::blocks::GRASS;
    use rustcraft_minecraft_b173::blocks::STONE;

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
            65
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
