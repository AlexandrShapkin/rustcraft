//! Headless simulation orchestration shared by the server, tests and future clients.

use rustcraft_agent_api::Controller;
use rustcraft_bot_api::{BOT_API_VERSION, NearbyBlockObservation, Observation, SelfObservation};
use rustcraft_content::ContentManifest;
use rustcraft_engine_core::{
    Aabb, BlockId, BlockPos, ChunkPos, EntityId, Vec3, World, split_block,
};
use rustcraft_mod_api::legacy_actions::MinecraftActions;
#[cfg(test)]
use rustcraft_mod_api::legacy_actions::PlayerIntent as AgentIntent;
use rustcraft_mod_api::{BlockRegistry, GameplayModule, ModuleId, RegistrationError};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
pub mod inventory;
pub mod lighting;
pub mod metrics;
pub mod survival;
use inventory::Inventory;
use inventory::ItemStack;
use lighting::{InitialLightingResult, Lighting, dirty_neighbors};
use survival::{GameMode, ItemEntity, RecipeRegistry};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Player {
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub half_width: f32,
    pub height: f32,
    pub on_ground: bool,
}

impl Player {
    #[must_use]
    pub fn bounds(&self) -> Aabb {
        Aabb::new(
            Vec3::new(
                self.position.x - self.half_width,
                self.position.y,
                self.position.z - self.half_width,
            ),
            Vec3::new(
                self.position.x + self.half_width,
                self.position.y + self.height,
                self.position.z + self.half_width,
            ),
        )
    }
}

#[derive(Debug)]
pub struct Simulation {
    pub world: World,
    /// Authoritative semantic profile; translation to retained local IDs is explicit.
    content_profile: Option<rustcraft_game_api::CompiledGameProfile>,
    pub last_action_error: Option<String>,
    pub registry: BlockRegistry,
    pub player: Player,
    pub time: u64,
    dirty_chunks: HashSet<ChunkPos>,
    dirty_sections: HashSet<rustcraft_engine_core::SectionPos>,
    pending_light_sections: HashMap<ChunkPos, Vec<i32>>,
    prelit_columns: HashSet<ChunkPos>,
    persistence_dirty_chunks: HashSet<ChunkPos>,
    pub inventory: Inventory,
    pub lighting: Lighting,
    pub mode: GameMode,
    pub items: Vec<ItemEntity>,
    pub recipes: RecipeRegistry,
    work_rules: HashMap<BlockId, ResolvedWork>,
    pub crafting_grid: [Option<ItemStack>; 4],
    /// Authoritative inventory transaction stack held by the UI cursor.
    pub inventory_cursor: Option<ItemStack>,
    pub mining: Option<MiningState>,
    entity_namespace: u64,
    next_entity: u64,
    entity_durable: HashMap<EntityId, DurableEntityState>,
    entity_tombstones: HashMap<ChunkPos, BTreeMap<EntityId, (ChunkPos, u64)>>,
    pickup_receipts: BTreeMap<EntityId, PickupReceiptState>,
    frozen_entity_columns: HashSet<ChunkPos>,
}

#[derive(Debug)]
struct ResolvedWork {
    duration: f32,
    multipliers: HashMap<rustcraft_engine_core::ItemId, f32>,
    reward: Option<(rustcraft_engine_core::ItemId, u16)>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct DurableEntityState {
    owner: ChunkPos,
    revision: u64,
    entity: ItemEntity,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PickupReceiptState {
    receipt: PickupReceipt,
    before: Option<ItemEntity>,
    player_durable: bool,
}

/// The world-side half of an item pickup. It is persisted atomically with the inventory; after
/// that player checkpoint succeeds, the referenced column can safely forget the entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickupReceipt {
    pub entity_id: EntityId,
    pub source: ChunkPos,
    /// Zero denotes a legacy full-removal receipt.
    pub before_revision: u64,
    pub after_revision: u64,
    pub before_count: u16,
    pub accepted_count: u16,
    pub remaining_count: u16,
}

/// Entity metadata captured with one immutable column save snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityColumnSnapshot {
    pub position: ChunkPos,
    pub entities: Vec<(EntityId, u64)>,
    pub records: Vec<ItemEntity>,
    pub tombstones: Vec<(EntityId, ChunkPos, u64)>,
}

static ENTITY_SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn new_entity_namespace() -> u64 {
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos() as u64);
    let sequence = ENTITY_SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    (wall.rotate_left(17) ^ u64::from(std::process::id()).rotate_left(41) ^ sequence).max(1)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MiningState {
    pub target: BlockPos,
    pub progress: f32,
    pub active: bool,
}

impl Simulation {
    #[must_use]
    pub fn spawn_above_surface(world: &World, registry: &BlockRegistry, x: i32, z: i32) -> Vec3 {
        let max_y = world
            .section_positions()
            .map(|(_, section_y)| section_y.saturating_mul(16).saturating_add(15))
            .max()
            .unwrap_or(64);
        for y in (-64..=max_y).rev() {
            let position = BlockPos { x, y, z };
            if registry.is_solid(world.get(position)) {
                return Vec3::new(x as f32 + 0.5, y as f32 + 2.0, z as f32 + 0.5);
            }
        }
        Vec3::new(x as f32 + 0.5, 3.0, z as f32 + 0.5)
    }
    #[must_use]
    pub fn new(world: World, registry: BlockRegistry, spawn: Vec3) -> Self {
        Self::new_with_entity_namespace(world, registry, spawn, new_entity_namespace())
    }

    /// Deterministic constructor for persistence/recovery tests. Production callers use `new`.
    #[must_use]
    pub fn new_with_entity_namespace(
        mut world: World,
        registry: BlockRegistry,
        spawn: Vec3,
        entity_namespace: u64,
    ) -> Self {
        let dirty_chunks = world.chunk_positions().collect();
        let dirty_sections = world.section_positions().collect();
        let lighting = Lighting::initialize(&mut world, &registry);
        let recipes = RecipeRegistry::from_definitions(&registry);
        let item = |key: &rustcraft_content::ResourceId| {
            registry.resolve_item_key(key).expect("validated rule item")
        };
        let work_rules = registry
            .work()
            .iter()
            .map(|rule| {
                let block = registry
                    .resolve_block_key(&rule.target)
                    .expect("validated rule block");
                (
                    block,
                    ResolvedWork {
                        duration: rule.duration_seconds,
                        multipliers: rule
                            .item_multipliers
                            .iter()
                            .map(|(key, rate)| (item(key), *rate))
                            .collect(),
                        reward: rule.reward.as_ref().map(|(key, count)| (item(key), *count)),
                    },
                )
            })
            .collect();
        Self {
            content_profile: registry.profile().cloned(),
            last_action_error: None,
            world,
            registry,
            player: Player {
                position: spawn,
                velocity: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
                half_width: 0.3,
                height: 1.8,
                on_ground: false,
            },
            time: 0,
            dirty_chunks,
            dirty_sections,
            pending_light_sections: HashMap::new(),
            prelit_columns: HashSet::new(),
            persistence_dirty_chunks: HashSet::new(),
            inventory: Inventory::default(),
            lighting,
            mode: GameMode::Development,
            items: Vec::new(),
            recipes,
            work_rules,
            crafting_grid: [None; 4],
            inventory_cursor: None,
            mining: None,
            entity_namespace: entity_namespace.max(1),
            next_entity: 1,
            entity_durable: HashMap::new(),
            entity_tombstones: HashMap::new(),
            pickup_receipts: BTreeMap::new(),
            frozen_entity_columns: HashSet::new(),
        }
    }
    /// Validate the local compatibility registry against an authoritative profile before admission.
    /// Compiled numeric handles are never treated as historical runtime IDs.
    pub fn bind_content_profile(
        &mut self,
        profile: rustcraft_game_api::CompiledGameProfile,
    ) -> Result<(), String> {
        for definition in self.registry.definitions() {
            let key = rustcraft_game_api::BlockKey::parse(definition.name)
                .map_err(|_| "invalid local block key")?;
            if profile.block_id(&key).is_none() {
                return Err(format!(
                    "profile has no local content mapping: {}",
                    definition.name
                ));
            }
        }
        self.content_profile = Some(profile);
        Ok(())
    }
    /// Semantic request admission. Resolves once at the boundary, then uses existing dense checks.
    pub fn place_semantic(
        &mut self,
        place: &rustcraft_agent_api::PlaceIntent,
    ) -> Result<(), String> {
        let profile = self
            .content_profile
            .as_ref()
            .ok_or("authoritative profile unavailable")?;
        let compiled_id = profile
            .block_id(&place.block)
            .ok_or_else(|| format!("unknown profile block: {}", place.block.as_str()))?;
        let key = &profile
            .block(compiled_id)
            .ok_or("invalid compiled profile mapping")?
            .key;
        let block = self
            .registry
            .by_name(key.as_str())
            .ok_or("local content mapping unavailable")?
            .id;
        if !self
            .target()
            .is_some_and(|hit| hit.normal != [0; 3] && hit.adjacent == place.position)
        {
            return Err("placement target/adjacency denied".into());
        }
        if !self.place_block(place.position, block) {
            return Err("placement held-item/collision/quantity denied".into());
        }
        Ok(())
    }

    pub fn set_mode(&mut self, mode: GameMode) {
        self.mode = mode;
        if mode == GameMode::Survival {
            self.inventory = Inventory::default();
        }
    }
    pub fn set_survival(&mut self) {
        self.set_mode(GameMode::Survival);
    }
    pub fn take_dirty_chunks(&mut self) -> Vec<ChunkPos> {
        self.dirty_chunks.drain().collect()
    }
    /// Authoritative voxel mutations only; unlike render dirtiness this excludes boundary-neighbor
    /// invalidations that do not alter the neighboring column's persisted state.
    pub fn take_persistence_dirty_chunks(&mut self) -> Vec<ChunkPos> {
        self.persistence_dirty_chunks.drain().collect()
    }
    pub fn take_dirty_sections(&mut self) -> Vec<rustcraft_engine_core::SectionPos> {
        self.dirty_sections.drain().collect()
    }

    /// Atomically publish a fully loaded/generated column into the authoritative world, extend
    /// derived lighting, and invalidate local/boundary render snapshots. `persist_new` marks new
    /// generated data dirty independently from its renderer state.
    pub fn publish_column(
        &mut self,
        position: ChunkPos,
        sections: Vec<(i32, rustcraft_engine_core::Chunk)>,
        persist_new: bool,
    ) -> Result<(), &'static str> {
        let section_ys = sections.iter().map(|(y, _)| *y).collect::<Vec<_>>();
        self.world.publish_column(position, sections)?;
        self.lighting.integrate_column(
            &mut self.world,
            &self.registry,
            position,
            section_ys.clone(),
            &mut self.dirty_sections,
        );
        self.mark_column_render_dirty(position, section_ys);
        if persist_new {
            self.persistence_dirty_chunks.insert(position);
        }
        Ok(())
    }

    /// Publish voxel data immediately but spread derived-light integration over bounded ticks.
    /// A bounded queue lets nearby ready columns become available without waiting for another
    /// column's light propagation; each integration still converges incrementally.
    pub fn publish_column_incremental_lighting(
        &mut self,
        position: ChunkPos,
        sections: Vec<(i32, rustcraft_engine_core::Chunk)>,
        persist_new: bool,
    ) -> Result<(), &'static str> {
        let section_ys = sections.iter().map(|(y, _)| *y).collect::<Vec<_>>();
        if !section_ys.is_empty() && !self.lighting.can_queue_column_integration(position) {
            return Err("column lighting queue is full");
        }
        self.world.publish_column(position, sections)?;
        if section_ys.is_empty() {
            self.mark_column_render_dirty(position, section_ys);
            if persist_new {
                self.persistence_dirty_chunks.insert(position);
            }
            return Ok(());
        }
        if !self
            .lighting
            .queue_column_integration(position, section_ys.clone())
        {
            return Err("column lighting queue is full");
        }
        if persist_new {
            self.persistence_dirty_chunks.insert(position);
        }
        self.pending_light_sections
            .insert(position, section_ys.clone());
        Ok(())
    }

    /// Atomically publish a column whose bulk initial lighting was built off-thread. Only the
    /// small resident-neighbor boundary reconciliation remains in the incremental lighting path.
    pub fn publish_initial_lit_column(
        &mut self,
        mut result: InitialLightingResult,
        persist_new: bool,
    ) -> Result<(), &'static str> {
        let position = result.position;
        let section_ys = result.sections.iter().map(|(y, _)| *y).collect::<Vec<_>>();
        self.world
            .publish_column(position, std::mem::take(&mut result.sections))?;
        for (section_y, lights) in result.light_sections {
            self.world
                .replace_section_lights(position, section_y, lights)
                .map_err(|_| "initial lighting returned an invalid section length")?;
        }
        self.lighting
            .adopt_initial_column(position, section_ys.clone(), result.direct_sections);
        if persist_new {
            self.persistence_dirty_chunks.insert(position);
        }
        self.pending_light_sections
            .insert(position, section_ys.clone());
        // Bulk initial lighting is already complete and locally self-consistent. Make this
        // column available to presentation now; cross-column boundary correction will invalidate
        // only sections whose light actually changes when that bounded work finishes.
        self.prelit_columns.insert(position);
        self.mark_column_render_dirty(position, section_ys);
        Ok(())
    }

    /// Advance streamed-column lighting by no more than the requested voxel work units.
    /// Bulk-initialized columns may already have a locally-lit first mesh; the dirty set here is
    /// the precise boundary-light patch to remesh after reconciliation converges.
    pub fn advance_column_lighting(&mut self, budget: usize) -> Option<ChunkPos> {
        let position = self.lighting.integrating_column()?;
        if self.lighting.advance_column_integration(
            &mut self.world,
            &self.registry,
            &mut self.dirty_sections,
            budget,
        ) {
            let section_ys = self
                .pending_light_sections
                .remove(&position)
                .unwrap_or_default();
            if !self.prelit_columns.remove(&position) {
                self.mark_column_render_dirty(position, section_ys);
            }
            Some(position)
        } else {
            None
        }
    }

    fn mark_column_render_dirty(&mut self, position: ChunkPos, section_ys: Vec<i32>) {
        for y in section_ys {
            self.dirty_sections.insert((position, y));
            for neighbor in [
                ChunkPos {
                    x: position.x - 1,
                    z: position.z,
                },
                ChunkPos {
                    x: position.x + 1,
                    z: position.z,
                },
                ChunkPos {
                    x: position.x,
                    z: position.z - 1,
                },
                ChunkPos {
                    x: position.x,
                    z: position.z + 1,
                },
            ] {
                if self.world.column_available(neighbor) {
                    self.dirty_sections.insert((neighbor, y));
                }
            }
        }
    }

    /// Remove a no-longer-retained clean column and invalidate its own plus adjacent section
    /// snapshots so page meshes cannot outlive world residency.
    pub fn remove_column(
        &mut self,
        position: ChunkPos,
    ) -> Vec<(i32, rustcraft_engine_core::Chunk)> {
        let removed = self.world.remove_column(position);
        self.prelit_columns.remove(&position);
        self.pending_light_sections.remove(&position);
        self.lighting.remove_column_from_world(
            &mut self.world,
            &self.registry,
            position,
            &mut self.dirty_sections,
        );
        for (y, _) in &removed {
            self.dirty_sections.insert((position, *y));
            for neighbor in [
                ChunkPos {
                    x: position.x - 1,
                    z: position.z,
                },
                ChunkPos {
                    x: position.x + 1,
                    z: position.z,
                },
                ChunkPos {
                    x: position.x,
                    z: position.z - 1,
                },
                ChunkPos {
                    x: position.x,
                    z: position.z + 1,
                },
            ] {
                if self.world.column_available(neighbor) {
                    self.dirty_sections.insert((neighbor, *y));
                }
            }
        }
        removed
    }

    /// Evict voxel residency immediately and schedule neighbor light cleanup incrementally.
    pub fn remove_column_incremental_lighting(
        &mut self,
        position: ChunkPos,
    ) -> Result<Vec<(i32, rustcraft_engine_core::Chunk)>, &'static str> {
        if !self.lighting.queue_column_removal(position) {
            return Err("column is being lit or lighting cleanup queue is full");
        }
        self.prelit_columns.remove(&position);
        self.pending_light_sections.remove(&position);
        let removed = self.world.remove_column(position);
        if removed.is_empty() {
            self.lighting.remove_column(position);
        }
        for (y, _) in &removed {
            self.dirty_sections.insert((position, *y));
            for neighbor in [
                ChunkPos {
                    x: position.x - 1,
                    z: position.z,
                },
                ChunkPos {
                    x: position.x + 1,
                    z: position.z,
                },
                ChunkPos {
                    x: position.x,
                    z: position.z - 1,
                },
                ChunkPos {
                    x: position.x,
                    z: position.z + 1,
                },
            ] {
                if self.world.column_available(neighbor) {
                    self.dirty_sections.insert((neighbor, *y));
                }
            }
        }
        Ok(removed)
    }
    pub fn dirty_section_count(&self) -> usize {
        self.dirty_sections.len()
    }
    pub fn target(&self) -> Option<rustcraft_engine_core::raycast::RayHit> {
        rustcraft_engine_core::raycast::cast(
            &self.world,
            Vec3::new(
                self.player.position.x,
                self.player.position.y + 1.62,
                self.player.position.z,
            ),
            Vec3::new(
                self.player.yaw.sin() * self.player.pitch.cos(),
                -self.player.pitch.sin(),
                self.player.yaw.cos() * self.player.pitch.cos(),
            ),
            5.,
            |id| {
                self.registry.get(id).is_some_and(|definition| {
                    definition.targetable
                        && definition.material != rustcraft_mod_api::Material::Invisible
                })
            },
        )
    }
    fn mark_dirty(&mut self, position: BlockPos) {
        dirty_neighbors(&mut self.dirty_sections, position);
        self.lighting.update(
            &mut self.world,
            &self.registry,
            position,
            &mut self.dirty_sections,
        );
        let (chunk, local) = split_block(position);
        self.dirty_chunks.insert(chunk);
        self.persistence_dirty_chunks.insert(chunk);
        if local.0 == 0 {
            self.dirty_chunks.insert(ChunkPos {
                x: chunk.x - 1,
                z: chunk.z,
            });
        }
        if local.0 == 15 {
            self.dirty_chunks.insert(ChunkPos {
                x: chunk.x + 1,
                z: chunk.z,
            });
        }
        if local.2 == 0 {
            self.dirty_chunks.insert(ChunkPos {
                x: chunk.x,
                z: chunk.z - 1,
            });
        }
        if local.2 == 15 {
            self.dirty_chunks.insert(ChunkPos {
                x: chunk.x,
                z: chunk.z + 1,
            });
        }
    }
    /// Controlled developer/system mutation preserving lighting and all dirty domains.
    /// Admission is validated by the composition adapter; work is one voxel per command.
    pub fn apply_world_commands(&mut self, commands: &mut rustcraft_game_api::CommandBuffer) {
        for command in commands.drain() {
            match command {
                rustcraft_game_api::WorldCommand::SetBlock { position, state } => {
                    self.world.set_state(position, state);
                    self.mark_dirty(position);
                }
            }
        }
    }

    pub fn step<G: Into<MinecraftActions>>(
        &mut self,
        intent: rustcraft_agent_api::AgentIntent<G>,
        dt: f32,
    ) {
        let intent = intent.map_game(Into::into);
        // Transitional M0-M3 adapter. Platform/controller input is generic; this legacy
        // simulation still owns the Minecraft mapping until its systems move to the game package.
        let primary_action = intent.primary_action || intent.game.attack;
        let secondary_action = intent.secondary_action || intent.game.use_action;
        if let Some(slot) = intent.game.select_hotbar {
            self.inventory.select(slot as usize);
        }
        self.inventory.scroll(i32::from(intent.game.scroll_hotbar));
        // Positive look deltas turn right/down. With +Y up and yaw=0 facing
        // +Z, screen-right is -X for a right-handed camera.
        self.player.yaw -= intent.look_delta.x * 0.002;
        self.player.pitch = (self.player.pitch + intent.look_delta.y * 0.002).clamp(-1.5, 1.5);
        let speed = 4.0;
        let mut velocity = self.player.velocity;
        let forward = Vec3::new(self.player.yaw.sin(), 0.0, self.player.yaw.cos());
        let right = Vec3::new(-self.player.yaw.cos(), 0.0, self.player.yaw.sin());
        velocity.x = (forward.x * intent.movement.forward + right.x * intent.movement.strafe)
            .clamp(-1.0, 1.0)
            * speed;
        velocity.z = (forward.z * intent.movement.forward + right.z * intent.movement.strafe)
            .clamp(-1.0, 1.0)
            * speed;
        if intent.jump && self.player.on_ground {
            velocity.y = 5.0;
            self.player.on_ground = false;
        }
        velocity.y -= 9.81 * dt;
        let (bounds, moved) = self.world.move_and_collide(
            self.player.bounds(),
            Vec3::new(velocity.x * dt, velocity.y * dt, velocity.z * dt),
            |id| self.registry.is_solid(id),
        );
        self.player.position = Vec3::new(
            (bounds.min.x + bounds.max.x) * 0.5,
            bounds.min.y,
            (bounds.min.z + bounds.max.z) * 0.5,
        );
        self.player.velocity = Vec3::new(
            if moved.x != velocity.x * dt {
                0.0
            } else {
                velocity.x
            },
            if moved.y != velocity.y * dt {
                0.0
            } else {
                velocity.y
            },
            if moved.z != velocity.z * dt {
                0.0
            } else {
                velocity.z
            },
        );
        self.player.on_ground = velocity.y < 0.0
            && self.world.collides(
                self.player.bounds().translated(Vec3::new(0.0, -0.001, 0.0)),
                |id| self.registry.is_solid(id),
            );
        if self.player.on_ground {
            self.player.velocity.y = 0.0;
        }
        let mut crossed_into = Vec::new();
        for entity in &mut self.items {
            if self.frozen_entity_columns.contains(&entity.column())
                || self.pickup_receipts.contains_key(&entity.id)
            {
                continue;
            }
            let owner_before = entity.column();
            let before = (
                entity.position,
                entity.velocity,
                entity.age,
                entity.pickup_delay,
            );
            entity.tick(&self.world, &self.registry, dt);
            if before
                != (
                    entity.position,
                    entity.velocity,
                    entity.age,
                    entity.pickup_delay,
                )
            {
                entity.persistence_revision = entity.persistence_revision.saturating_add(1);
            }
            let owner_after = entity.column();
            if owner_after != owner_before {
                crossed_into.push(owner_after);
            }
        }
        self.persistence_dirty_chunks.extend(crossed_into);
        let expired = self
            .items
            .iter()
            .filter(|entity| {
                entity.age >= 300.0
                    && !self.pickup_receipts.contains_key(&entity.id)
                    && !self
                        .entity_tombstones
                        .values()
                        .any(|markers| markers.contains_key(&entity.id))
                    && !self
                        .entity_durable
                        .get(&entity.id)
                        .is_some_and(|durable| durable.owner != entity.column())
            })
            .map(|entity| (entity.id, entity.column()))
            .collect::<Vec<_>>();
        if !expired.is_empty() {
            self.items
                .retain(|entity| !expired.iter().any(|(id, _)| *id == entity.id));
            for (_, source) in expired {
                self.persistence_dirty_chunks.insert(source);
            }
        }
        self.merge_items();
        self.pickup_items();
        if self.mode == GameMode::Survival && primary_action {
            self.mine_tick(dt);
        } else if self.mode == GameMode::Survival {
            self.mining = None;
        }
        if let Some(position) = intent.game.break_block
            && self.target().is_some_and(|hit| hit.block == position)
        {
            let _ = self.break_block(position);
        }
        if self.mode == GameMode::Development
            && primary_action
            && let Some(hit) = self.target()
        {
            let _ = self.break_block(hit.block);
        }
        if secondary_action
            && let Some(hit) = self.target()
            && hit.normal != [0; 3]
            && let Some(stack) = self.inventory.held()
            && let Some(block) = self.registry.item(stack.item).and_then(|i| i.placeable)
        {
            let _ = self.place_block(hit.adjacent, block);
        }
        self.last_action_error = intent
            .game
            .place_block
            .as_ref()
            .and_then(|place| self.place_semantic(place).err());
        if intent.game.craft {
            let _ = self.take_crafting_output();
        }
        self.time = self.time.saturating_add(1);
        // Moving entities are continuously authoritative but are checkpointed at the same bounded
        // cadence as terrain autosaves, rather than issuing filesystem work every fixed tick.
        if self.time.is_multiple_of(40) {
            self.persistence_dirty_chunks
                .extend(self.items.iter().map(ItemEntity::column));
        }
    }
    #[must_use]
    pub fn break_block(&mut self, position: BlockPos) -> bool {
        if !self.world.column_available(split_block(position).0) {
            return false;
        }
        if !self
            .registry
            .get(self.world.get(position))
            .is_some_and(|b| b.breakable)
        {
            false
        } else {
            let drop = self
                .work_rules
                .get(&self.world.get(position))
                .and_then(|rule| rule.reward);
            self.world.set(position, self.world.empty_block());
            self.mark_dirty(position);
            if self.mode == GameMode::Survival
                && let Some((item, count)) = drop
            {
                self.spawn_item(
                    item,
                    count,
                    Vec3::new(
                        position.x as f32 + 0.5,
                        position.y as f32 + 0.7,
                        position.z as f32 + 0.5,
                    ),
                );
            }
            true
        }
    }
    #[must_use]
    pub fn place_block(&mut self, position: BlockPos, block: BlockId) -> bool {
        if !self.world.column_available(split_block(position).0) {
            return false;
        }
        let Some(definition) = self.registry.get(block) else {
            return false;
        };
        let held_block = self
            .inventory
            .held()
            .and_then(|s| self.registry.item(s.item))
            .and_then(|i| i.placeable);
        if self.world.get(position) != self.world.empty_block()
            || held_block != Some(block)
            || block == self.world.empty_block()
        {
            return false;
        }
        let block_bounds = Aabb::new(
            Vec3::new(position.x as f32, position.y as f32, position.z as f32),
            Vec3::new(
                position.x as f32 + 1.0,
                position.y as f32 + 1.0,
                position.z as f32 + 1.0,
            ),
        );
        if definition.solid && block_bounds.intersects(self.player.bounds()) {
            return false;
        }
        self.world.set(position, block);
        self.inventory.remove(self.inventory.selected(), 1);
        self.mark_dirty(position);
        true
    }
    pub fn spawn_item(&mut self, item: rustcraft_engine_core::ItemId, count: u16, position: Vec3) {
        let id = EntityId::from_parts(self.entity_namespace, self.next_entity);
        self.next_entity = self.next_entity.saturating_add(1).max(1);
        let column = ChunkPos {
            x: (position.x.floor() as i32).div_euclid(16),
            z: (position.z.floor() as i32).div_euclid(16),
        };
        self.items.push(ItemEntity {
            id,
            persistence_revision: 1,
            stack: ItemStack {
                item,
                count,
                damage: 0,
            },
            position,
            velocity: Vec3::new(0.0, 0.2, 0.0),
            age: 0.0,
            pickup_delay: 0.25,
        });
        self.persistence_dirty_chunks.insert(column);
    }
    fn merge_items(&mut self) {
        for i in 0..self.items.len() {
            for j in ((i + 1)..self.items.len()).rev() {
                if !self.pickup_receipts.contains_key(&self.items[i].id)
                    && !self.pickup_receipts.contains_key(&self.items[j].id)
                    && ![&self.items[i], &self.items[j]].iter().any(|entity| {
                        if self
                            .entity_tombstones
                            .values()
                            .any(|markers| markers.contains_key(&entity.id))
                        {
                            return true;
                        }
                        self.entity_durable
                            .get(&entity.id)
                            .is_some_and(|durable| durable.owner != entity.column())
                    })
                    && self.items[i].stack.item == self.items[j].stack.item
                    && self.items[i].column() == self.items[j].column()
                    && (self.items[i].position - self.items[j].position)
                        .x
                        .hypot((self.items[i].position - self.items[j].position).z)
                        < 1.0
                {
                    let max = self
                        .registry
                        .item(self.items[i].stack.item)
                        .map_or(64, |d| d.max_stack);
                    let room = max - self.items[i].stack.count;
                    let take = room.min(self.items[j].stack.count);
                    if take == 0 {
                        continue;
                    }
                    self.items[i].stack.count += take;
                    self.items[i].persistence_revision =
                        self.items[i].persistence_revision.saturating_add(1);
                    self.items[j].stack.count -= take;
                    self.items[j].persistence_revision =
                        self.items[j].persistence_revision.saturating_add(1);
                    self.persistence_dirty_chunks.insert(self.items[i].column());
                    self.persistence_dirty_chunks.insert(self.items[j].column());
                    if self.items[j].stack.count == 0 {
                        self.items.remove(j);
                    }
                }
            }
        }
    }
    fn pickup_items(&mut self) {
        let radius = 1.5;
        let mut i = 0;
        while i < self.items.len() {
            let before = self.items[i];
            if self.pickup_receipts.contains_key(&before.id)
                || self.pickup_receipts.len() >= 512
                || before.persistence_revision == u64::MAX
                || self
                    .entity_tombstones
                    .values()
                    .any(|markers| markers.contains_key(&before.id))
                || self
                    .entity_durable
                    .get(&before.id)
                    .is_some_and(|durable| durable.owner != before.column())
            {
                i += 1;
                continue;
            }
            let d = before.position - self.player.position;
            if before.pickup_delay <= 0. && d.x * d.x + d.y * d.y + d.z * d.z < radius * radius {
                let rem = self.inventory.insert_partial(before.stack, &self.registry);
                if rem.count < before.stack.count {
                    let after_revision = before.persistence_revision.saturating_add(1);
                    self.pickup_receipts.insert(
                        before.id,
                        PickupReceiptState {
                            receipt: PickupReceipt {
                                entity_id: before.id,
                                source: before.column(),
                                before_revision: before.persistence_revision,
                                after_revision,
                                before_count: before.stack.count,
                                accepted_count: before.stack.count - rem.count,
                                remaining_count: rem.count,
                            },
                            before: Some(before),
                            player_durable: false,
                        },
                    );
                    self.persistence_dirty_chunks.insert(before.column());
                    if rem.count == 0 {
                        self.items.remove(i);
                        continue;
                    }
                    self.items[i].stack = rem;
                    self.items[i].persistence_revision = after_revision;
                }
            }
            i += 1;
        }
    }

    /// Durable pickup/despawn/merge receipts currently awaiting an atomic player checkpoint.
    #[must_use]
    pub fn pickup_receipts(&self) -> Vec<PickupReceipt> {
        self.pickup_receipts
            .values()
            .map(|state| state.receipt)
            .collect()
    }

    /// Restore the player-side transition before activating spatial records.
    pub fn restore_pickup_receipts(&mut self, receipts: impl IntoIterator<Item = PickupReceipt>) {
        for receipt in receipts {
            self.pickup_receipts.insert(
                receipt.entity_id,
                PickupReceiptState {
                    receipt,
                    before: None,
                    player_durable: true,
                },
            );
        }
    }

    /// Acknowledge only the exact transition captured by the durable player revision.
    pub fn commit_pickup_receipts(&mut self, durable_receipts: &[PickupReceipt]) {
        for receipt in durable_receipts {
            if let Some(state) = self.pickup_receipts.get_mut(&receipt.entity_id)
                && state.receipt == *receipt
            {
                state.player_durable = true;
                self.persistence_dirty_chunks.insert(state.receipt.source);
            }
        }
    }

    /// Column records that were durably rewritten without a consumed entity complete its transfer.
    /// The returned boolean means the player record changed and should be checkpointed again.
    pub fn note_entity_column_persisted(&mut self, snapshot: &EntityColumnSnapshot) -> bool {
        let saved = snapshot.entities.iter().copied().collect::<HashMap<_, _>>();
        for (id, source, revision) in &snapshot.tombstones {
            self.entity_tombstones
                .entry(snapshot.position)
                .or_default()
                .insert(*id, (*source, *revision));
        }
        let mut old_owners = Vec::new();
        for entity in &snapshot.records {
            let id = entity.id;
            let revision = entity.persistence_revision;
            if self
                .entity_durable
                .get(&id)
                .is_some_and(|previous| previous.revision > revision)
            {
                continue;
            }
            if let Some(previous) = self.entity_durable.insert(
                id,
                DurableEntityState {
                    owner: snapshot.position,
                    revision,
                    entity: *entity,
                },
            ) && previous.owner != snapshot.position
            {
                old_owners.push(previous.owner);
            }
            if let Some(active) = self.items.iter().find(|active| active.id == id)
                && (active.column() != snapshot.position || active.persistence_revision > revision)
            {
                self.persistence_dirty_chunks.insert(active.column());
            }
        }
        self.persistence_dirty_chunks.extend(old_owners);

        let mut tombstone_columns_to_rewrite = Vec::new();
        for (owner, markers) in &mut self.entity_tombstones {
            let before = markers.len();
            markers.retain(|id, (source, revision)| {
                !(*source == snapshot.position
                    && (saved
                        .get(id)
                        .is_some_and(|saved_revision| saved_revision > revision)
                        || (!saved.contains_key(id)
                            && self
                                .entity_durable
                                .get(id)
                                .is_some_and(|durable| durable.owner != *source))))
            });
            if markers.len() != before {
                tombstone_columns_to_rewrite.push(*owner);
            }
        }
        self.entity_tombstones
            .retain(|_, markers| !markers.is_empty());
        self.persistence_dirty_chunks
            .extend(tombstone_columns_to_rewrite);

        let removed_receipts = self
            .pickup_receipts
            .iter()
            .filter(|(id, state)| {
                state.player_durable
                    && state.receipt.source == snapshot.position
                    && if state.receipt.remaining_count == 0 {
                        !saved.contains_key(id)
                    } else {
                        saved
                            .get(id)
                            .is_some_and(|revision| *revision >= state.receipt.after_revision)
                    }
            })
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for id in &removed_receipts {
            self.pickup_receipts.remove(id);
            if !self.items.iter().any(|entity| entity.id == *id) {
                self.entity_durable.remove(id);
            }
        }
        self.entity_durable.retain(|id, durable| {
            durable.owner != snapshot.position
                || saved
                    .get(id)
                    .is_some_and(|revision| *revision >= durable.revision)
        });
        // An evicted destination may have been kept only for an unfinished source cleanup.
        // Once the last recovery reference retires, disk records remain the durable owner.
        let referenced = self
            .items
            .iter()
            .map(|e| e.id)
            .chain(self.pickup_receipts.keys().copied())
            .chain(
                self.entity_tombstones
                    .values()
                    .flat_map(|m| m.keys().copied()),
            )
            .collect::<HashSet<_>>();
        self.entity_durable
            .retain(|id, state| self.world.column_resident(state.owner) || referenced.contains(id));
        !removed_receipts.is_empty()
    }

    /// Activate a fully decoded column as one unit. Newest revision wins; tombstones and durable
    /// pickup receipts suppress stale source copies after an interrupted transfer.
    pub fn activate_entity_column(
        &mut self,
        position: ChunkPos,
        mut entities: Vec<ItemEntity>,
        tombstones: &[(EntityId, ChunkPos, u64)],
    ) -> Result<(), &'static str> {
        if entities.iter().any(|entity| entity.column() != position) {
            return Err("spatial entity position does not match owning column");
        }
        for (id, source, revision) in tombstones {
            self.entity_tombstones
                .entry(position)
                .or_default()
                .insert(*id, (*source, *revision));
        }
        entities.retain_mut(|entity| {
            let Some(state) = self.pickup_receipts.get(&entity.id) else {
                return true;
            };
            let receipt = state.receipt;
            if receipt.remaining_count == 0 {
                return false;
            }
            if entity.persistence_revision <= receipt.before_revision {
                entity.stack.count = receipt.remaining_count;
                entity.persistence_revision = receipt.after_revision;
                self.persistence_dirty_chunks.insert(position);
            }
            true
        });
        for entity in entities {
            if let Some(index) = self.items.iter().position(|active| active.id == entity.id) {
                if self.items[index].persistence_revision >= entity.persistence_revision {
                    continue;
                }
                self.items.swap_remove(index);
            }
            let suppressed = self.entity_tombstones.values().any(|markers| {
                markers.get(&entity.id).is_some_and(|(source, revision)| {
                    *source == position && *revision >= entity.persistence_revision
                })
            });
            if suppressed {
                continue;
            }
            self.entity_durable.insert(
                entity.id,
                DurableEntityState {
                    owner: position,
                    revision: entity.persistence_revision,
                    entity,
                },
            );
            self.items.push(entity);
        }
        if self
            .entity_tombstones
            .values()
            .any(|markers| markers.values().any(|(source, _)| *source == position))
            || self
                .pickup_receipts
                .values()
                .any(|state| state.receipt.source == position)
        {
            self.persistence_dirty_chunks.insert(position);
        }
        Ok(())
    }

    /// Retain pre-transfer records until the destination domain acknowledges durability.
    #[must_use]
    pub fn persistent_entity_records(&self, position: ChunkPos) -> Vec<ItemEntity> {
        let mut records = self
            .items
            .iter()
            .filter(|entity| entity.column() == position)
            .map(|entity| (entity.id, *entity))
            .collect::<BTreeMap<_, _>>();
        for (id, state) in &self.pickup_receipts {
            if !state.player_durable {
                records.remove(id);
                if let Some(before) = state.before
                    && before.column() == position
                {
                    records.insert(*id, before);
                }
            }
        }
        for (id, durable) in &self.entity_durable {
            if durable.owner == position
                && self
                    .items
                    .iter()
                    .any(|entity| entity.id == *id && entity.column() != position)
            {
                records.insert(*id, durable.entity);
            }
            // Retain a durable fallback if the runtime record was removed by pickup.
            if durable.owner == position
                && self
                    .pickup_receipts
                    .get(id)
                    .is_some_and(|state| !state.player_durable)
            {
                records.entry(*id).or_insert(durable.entity);
            }
        }
        records.into_values().collect()
    }

    #[must_use]
    pub fn entity_column_snapshot(&self, position: ChunkPos) -> EntityColumnSnapshot {
        let mut tombstones = self
            .entity_tombstones
            .get(&position)
            .cloned()
            .unwrap_or_default();
        for entity in self
            .items
            .iter()
            .filter(|entity| entity.column() == position)
        {
            if let Some(durable) = self.entity_durable.get(&entity.id)
                && durable.owner != position
            {
                tombstones.insert(entity.id, (durable.owner, entity.persistence_revision));
            }
        }
        let records = self.persistent_entity_records(position);
        EntityColumnSnapshot {
            position,
            entities: records
                .iter()
                .map(|entity| (entity.id, entity.persistence_revision))
                .collect(),
            records,
            tombstones: tombstones
                .into_iter()
                .map(|(id, (source, revision))| (id, source, revision))
                .collect(),
        }
    }

    #[must_use]
    pub fn entity_column_positions(&self) -> HashSet<ChunkPos> {
        self.items.iter().map(ItemEntity::column).collect()
    }

    /// Source terrain must remain resident while a later acknowledgement still needs a rewrite.
    #[must_use]
    pub fn entity_transfer_pending(&self, position: ChunkPos) -> bool {
        self.pickup_receipts
            .values()
            .any(|state| state.receipt.source == position)
            || self.entity_durable.iter().any(|(id, durable)| {
                durable.owner == position
                    && self
                        .items
                        .iter()
                        .any(|entity| entity.id == *id && entity.column() != position)
            })
            || self
                .entity_tombstones
                .values()
                .any(|markers| markers.values().any(|(source, _)| *source == position))
    }

    pub fn freeze_entity_column(&mut self, position: ChunkPos) -> bool {
        let newly_frozen = self.frozen_entity_columns.insert(position);
        let has_entities = self.items.iter().any(|entity| entity.column() == position);
        if newly_frozen && has_entities {
            self.persistence_dirty_chunks.insert(position);
        }
        newly_frozen && has_entities
    }

    pub fn unfreeze_entity_column(&mut self, position: ChunkPos) {
        self.frozen_entity_columns.remove(&position);
    }

    /// Scalar ownership only; no references or entity payloads escape into diagnostic history.
    pub fn entity_lifetime_counts(&self) -> [usize; 6] {
        [
            self.items.len(),
            self.entity_durable.len(),
            self.entity_tombstones.values().map(BTreeMap::len).sum(),
            self.pickup_receipts.len(),
            self.frozen_entity_columns.len(),
            self.entity_durable.capacity(),
        ]
    }

    pub fn evict_entity_column(&mut self, position: ChunkPos) {
        self.items.retain(|entity| entity.column() != position);
        // Stable saved owner metadata is reconstructed on activation. Keep recovery/transfer
        // references and active entities until their existing persistence acknowledgements retire.
        self.entity_durable.retain(|id, state| {
            state.owner != position
                || self.items.iter().any(|e| e.id == *id)
                || self.pickup_receipts.contains_key(id)
                || self
                    .entity_tombstones
                    .values()
                    .any(|markers| markers.contains_key(id))
        });
        self.frozen_entity_columns.remove(&position);
    }
    fn mine_tick(&mut self, dt: f32) {
        let Some(hit) = self.target() else {
            self.mining = None;
            return;
        };
        let block_id = self.world.get(hit.block);
        let Some(b) = self.registry.get(block_id) else {
            self.mining = None;
            return;
        };
        if !b.breakable {
            self.mining = None;
            return;
        }
        let Some(rule) = self.work_rules.get(&block_id) else {
            self.mining = None;
            return;
        };
        let speed = self
            .inventory
            .held()
            .and_then(|held| rule.multipliers.get(&held.item))
            .copied()
            .unwrap_or(1.);
        let same = self.mining.is_some_and(|m| m.target == hit.block);
        let mut state = self.mining.unwrap_or(MiningState {
            target: hit.block,
            progress: 0.,
            active: true,
        });
        if !same {
            state.progress = 0.;
            state.target = hit.block;
        }
        let mut progress = rustcraft_game_api::WorkProgress(state.progress);
        progress.advance(dt, rule.duration, speed);
        state.progress = progress.0;
        if state.progress >= 1. {
            let _ = self.break_block(hit.block);
            self.inventory.damage_selected(1, &self.registry);
            self.mining = None;
        } else {
            self.mining = Some(state);
        }
    }
    pub fn crafting_output(&self) -> Option<ItemStack> {
        self.recipes.find(&self.crafting_grid).map(|r| r.output)
    }
    pub fn take_crafting_output(&mut self) -> bool {
        self.take_crafting_output_stack().is_some()
    }
    pub fn take_crafting_output_stack(&mut self) -> Option<ItemStack> {
        let recipe = self.recipes.find(&self.crafting_grid)?;
        if self
            .inventory
            .insert(recipe.output.item, recipe.output.count, &self.registry)
            > 0
        {
            return None;
        };
        for slot in &mut self.crafting_grid {
            if slot.is_some() {
                slot.as_mut().unwrap().count -= 1;
                if slot.as_ref().unwrap().count == 0 {
                    *slot = None;
                }
            }
        }
        Some(recipe.output)
    }
    pub fn take_crafting_output_for_cursor(&mut self) -> Option<ItemStack> {
        let recipe = self.recipes.find(&self.crafting_grid)?;
        for slot in &mut self.crafting_grid {
            if slot.is_some() {
                slot.as_mut().unwrap().count -= 1;
                if slot.as_ref().unwrap().count == 0 {
                    *slot = None;
                }
            }
        }
        Some(recipe.output)
    }
    pub fn swap_crafting_slot(
        &mut self,
        slot: usize,
        stack: Option<ItemStack>,
    ) -> Option<ItemStack> {
        let Some(current) = self.crafting_grid.get_mut(slot) else {
            return stack;
        };
        std::mem::replace(current, stack)
    }
    pub fn craft_first_available(&mut self, recipe_id: &str) -> bool {
        let Some(recipe) = self
            .recipes
            .recipes
            .iter()
            .find(|r| r.id == recipe_id)
            .cloned()
        else {
            return false;
        };
        let needed = recipe.inputs.iter().flatten().copied().collect::<Vec<_>>();
        let mut slots = Vec::new();
        for id in needed {
            let Some(slot) = self
                .inventory
                .slots()
                .iter()
                .position(|s| s.is_some_and(|x| x.item == id))
            else {
                return false;
            };
            slots.push(slot);
        }
        for slot in slots {
            self.inventory.remove(slot, 1);
        }
        self.inventory
            .insert(recipe.output.item, recipe.output.count, &self.registry)
            == 0
    }
    #[must_use]
    pub fn observe(
        &self,
        radius: i32,
    ) -> Observation<rustcraft_bot_api::legacy::MinecraftObservation> {
        let center = BlockPos {
            x: self.player.position.x.floor() as i32,
            y: self.player.position.y.floor() as i32,
            z: self.player.position.z.floor() as i32,
        };
        let mut nearby = Vec::new();
        for y in center.y - radius..=center.y + radius {
            for z in center.z - radius..=center.z + radius {
                for x in center.x - radius..=center.x + radius {
                    let position = BlockPos { x, y, z };
                    let id = self.world.get(position);
                    if let Some(definition) = self.registry.get(id)
                        && id != self.world.empty_block()
                    {
                        nearby.push(NearbyBlockObservation {
                            position,
                            block_key: definition.name.to_owned(),
                        });
                    }
                }
            }
        }
        Observation {
            api_version: BOT_API_VERSION,
            last_action_error: self.last_action_error.clone(),
            self_state: SelfObservation {
                position: self.player.position,
                velocity: self.player.velocity,
            },
            nearby_blocks: nearby,
            items: self
                .registry
                .items()
                .iter()
                .map(|i| rustcraft_bot_api::ItemObservation {
                    key: i.name.into(),
                    max_stack: i.max_stack,
                    placeable_block: i
                        .placeable
                        .and_then(|b| self.registry.get(b).map(|d| d.name.into())),
                    capabilities: i.capabilities.iter().map(|s| (*s).into()).collect(),
                })
                .collect(),
            blocks: self
                .registry
                .definitions()
                .iter()
                .map(|b| rustcraft_bot_api::BlockObservation {
                    key: b.name.into(),
                    solid: b.solid,
                    opaque: b.material == rustcraft_mod_api::Material::Opaque,
                    breakable: b.breakable,
                    emission: b.emission,
                    sky_opacity: b.sky_opacity,
                    light_opacity: b.light_opacity,
                })
                .collect(),
            nearby_items: self
                .items
                .iter()
                .map(|e| rustcraft_bot_api::ItemEntityObservation {
                    id: e.id,
                    item_key: self
                        .registry
                        .item(e.stack.item)
                        .map_or("unknown".into(), |d| d.name.into()),
                    count: e.stack.count,
                    position: e.position,
                })
                .collect(),
            game: rustcraft_bot_api::legacy::MinecraftObservation {
                inventory: self
                    .inventory
                    .slots()
                    .iter()
                    .map(|slot| {
                        slot.and_then(|s| {
                            self.registry.item(s.item).map(|d| {
                                rustcraft_bot_api::StackObservation {
                                    item_key: d.name.into(),
                                    count: s.count,
                                }
                            })
                        })
                    })
                    .collect(),
                selected_hotbar: self.inventory.selected() as u8,
                mining_progress: self.mining.map(|m| m.progress),
            },
        }
    }
}

#[derive(Debug, Default)]
pub struct RuntimeBootstrap {
    modules: Vec<ModuleId>,
    content: ContentManifest,
    pub registry: BlockRegistry,
}

impl RuntimeBootstrap {
    #[must_use]
    pub fn new(content: ContentManifest) -> Self {
        Self {
            modules: Vec::new(),
            content,
            registry: BlockRegistry::default(),
        }
    }
    pub fn register_module(
        &mut self,
        module: &impl GameplayModule,
    ) -> Result<(), RegistrationError> {
        module.register(&mut self.registry)?;
        self.modules.push(module.id());
        Ok(())
    }
    #[must_use]
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }
    #[must_use]
    pub fn content(&self) -> &ContentManifest {
        &self.content
    }
}

pub fn run_controller<G: Into<MinecraftActions>, C: Controller<G>>(
    simulation: &mut Simulation,
    controller: &mut C,
    ticks: usize,
    dt: f32,
) {
    for _ in 0..ticks {
        simulation.step(controller.next_intent(), dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_agent_api::{MoveIntent, PlaceIntent};
    const STONE: rustcraft_mod_api::BlockDefinition =
        rustcraft_mod_api::BlockDefinition::cube(1, "test:stone", "test:stone");
    #[test]
    fn movement_gravity_and_collision_stop_on_floor() {
        let mut registry = BlockRegistry::default();
        registry.register(STONE).unwrap();
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 0, y: 0, z: 0 }, STONE.id);
        let mut sim = Simulation::new(world, registry, Vec3::new(0.5, 3.0, 0.5));
        for _ in 0..100 {
            sim.step(AgentIntent::default(), 0.02);
        }
        assert!(sim.player.on_ground);
        assert!(
            (sim.player.position.y - 1.0).abs() < 0.01,
            "y={}",
            sim.player.position.y
        );
    }

    #[test]
    fn streamed_world_frontier_blocks_movement_and_interactions_until_publish() {
        let mut registry = BlockRegistry::default();
        registry.register(STONE).unwrap();
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 15, y: 0, z: 0 }, STONE.id);
        world.enforce_column_availability(true);
        let mut sim = Simulation::new(world, registry, Vec3::new(15.4, 1.0, 0.5));
        for _ in 0..20 {
            sim.step(
                AgentIntent {
                    movement: MoveIntent {
                        forward: 0.0,
                        strafe: -1.0,
                    },
                    ..Default::default()
                },
                0.05,
            );
        }
        assert!(sim.player.position.x + sim.player.half_width <= 16.01);
        assert!(!sim.break_block(BlockPos { x: 16, y: 0, z: 0 }));
        assert!(
            sim.publish_column(ChunkPos { x: 1, z: 0 }, Vec::new(), false)
                .is_ok()
        );
        sim.step(
            AgentIntent {
                movement: MoveIntent {
                    forward: 0.0,
                    strafe: -1.0,
                },
                ..Default::default()
            },
            0.05,
        );
        assert!(sim.player.position.x + sim.player.half_width <= 16.01);
        assert!(sim.world.set_column_safe(ChunkPos { x: 1, z: 0 }, true));
        sim.step(
            AgentIntent {
                movement: MoveIntent {
                    forward: 0.0,
                    strafe: -1.0,
                },
                ..Default::default()
            },
            0.05,
        );
        assert!(sim.player.position.x > 15.4);
    }

    #[test]
    fn queued_lighting_columns_cannot_be_evicted_before_convergence() {
        let mut registry = BlockRegistry::default();
        registry.register(STONE).unwrap();
        let mut world = World::new(BlockId(0));
        let first = ChunkPos { x: 0, z: 0 };
        let second = ChunkPos { x: 1, z: 0 };
        let third = ChunkPos { x: 2, z: 0 };
        world
            .publish_column(
                first,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        let mut sim = Simulation::new(world, registry, Vec3::new(0.5, 2.0, 0.5));
        for column in [second, third] {
            sim.publish_column_incremental_lighting(
                column,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
                false,
            )
            .unwrap();
        }
        assert!(sim.lighting.has_integration_work());
        assert!(sim.remove_column_incremental_lighting(second).is_err());
        assert!(
            !sim.remove_column_incremental_lighting(first)
                .unwrap()
                .is_empty()
        );
        assert!(
            !sim.world
                .section_positions()
                .any(|(column, _)| column == first)
        );
        assert!(sim.lighting.has_integration_work());
    }

    #[test]
    fn streamed_column_is_not_remeshed_until_bounded_lighting_finishes() {
        let mut world = World::new(BlockId(0));
        let first = ChunkPos { x: 0, z: 0 };
        world
            .publish_column(
                first,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        let mut sim = Simulation::new(world, BlockRegistry::default(), Vec3::new(0.5, 2.0, 0.5));
        sim.take_dirty_sections();
        let second = ChunkPos { x: 1, z: 0 };
        sim.publish_column_incremental_lighting(
            second,
            vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            false,
        )
        .unwrap();
        assert!(sim.take_dirty_sections().is_empty());
        let mut completed = false;
        for _ in 0..32 {
            if sim.advance_column_lighting(16_384).is_some() {
                completed = true;
                break;
            }
        }
        assert!(completed, "column lighting should converge in bounded work");
        let dirty = sim.take_dirty_sections();
        assert!(dirty.contains(&(second, 0)));
        assert!(
            dirty.len() <= 20,
            "one-section column invalidation: {dirty:?}"
        );
    }

    #[test]
    fn non_targetable_liquid_is_ray_pass_through_and_non_solid() {
        use rustcraft_mod_api::{BlockDefinition, Material};
        let water = BlockDefinition {
            solid: false,
            targetable: false,
            material: Material::Liquid,
            sky_opacity: 1,
            light_opacity: 1,
            ..BlockDefinition::cube(2, "test:water", "test:water")
        };
        let mut registry = BlockRegistry::default();
        registry.register(STONE).unwrap();
        registry.register(water).unwrap();
        let mut water_only = World::new(BlockId(0));
        water_only.set(BlockPos { x: 0, y: 2, z: 1 }, water.id);
        let water_only_sim =
            Simulation::new(water_only, registry.clone(), Vec3::new(0.5, 1.0, 0.5));
        assert!(water_only_sim.target().is_none());

        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 0, y: 2, z: 1 }, water.id);
        world.set(BlockPos { x: 0, y: 2, z: 2 }, STONE.id);
        let sim = Simulation::new(world, registry.clone(), Vec3::new(0.5, 1.0, 0.5));
        assert_eq!(sim.target().unwrap().block, BlockPos { x: 0, y: 2, z: 2 });

        let mut water_world = World::new(BlockId(0));
        water_world.set(BlockPos { x: 0, y: 0, z: 0 }, water.id);
        let mut falling = Simulation::new(water_world, registry, Vec3::new(0.5, 2.0, 0.5));
        for _ in 0..30 {
            falling.step(AgentIntent::default(), 0.02);
        }
        assert!(
            falling.player.position.y < 1.0,
            "y={}",
            falling.player.position.y
        );
    }
    #[test]
    fn explicit_break_and_place_update_world() {
        let mut registry = BlockRegistry::default();
        registry.register(STONE).unwrap();
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 0, y: 0, z: 0 }, STONE.id);
        let mut sim = Simulation::new(world, registry, Vec3::new(4.0, 2.0, 4.0));
        assert!(sim.break_block(BlockPos { x: 0, y: 0, z: 0 }));
        sim.registry
            .register_item(rustcraft_mod_api::ItemDefinition {
                id: rustcraft_engine_core::ItemId(1),
                name: "test:stone",
                max_stack: 64,
                placeable: Some(STONE.id),
                capabilities: &[],
                tool: None,
            })
            .unwrap();
        sim.inventory
            .insert(rustcraft_engine_core::ItemId(1), 1, &sim.registry);
        assert!(sim.place_block(BlockPos { x: 1, y: 0, z: 0 }, STONE.id));
    }

    #[test]
    fn unbreakable_definition_rejects_break_without_progress_state() {
        let bedrock = rustcraft_mod_api::BlockDefinition {
            breakable: false,
            ..STONE
        };
        let mut registry = BlockRegistry::default();
        registry.register(bedrock).unwrap();
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 0, y: 0, z: 0 }, bedrock.id);
        let mut sim = Simulation::new(world, registry, Vec3::new(0.5, 2.0, 0.5));
        assert!(!sim.break_block(BlockPos { x: 0, y: 0, z: 0 }));
        assert_eq!(sim.world.get(BlockPos { x: 0, y: 0, z: 0 }), bedrock.id);
        assert!(sim.mining.is_none());
    }

    #[test]
    fn boundary_block_changes_dirty_neighbor_chunk() {
        let mut registry = BlockRegistry::default();
        registry.register(STONE).unwrap();
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 15, y: 0, z: 0 }, STONE.id);
        let mut simulation = Simulation::new(world, registry, Vec3::new(4.0, 2.0, 4.0));
        let _ = simulation.take_dirty_chunks();
        assert!(simulation.break_block(BlockPos { x: 15, y: 0, z: 0 }));
        let dirty = simulation.take_dirty_chunks();
        assert!(dirty.contains(&ChunkPos { x: 0, z: 0 }));
        assert!(dirty.contains(&ChunkPos { x: 1, z: 0 }));
        assert_eq!(
            simulation.take_persistence_dirty_chunks(),
            vec![ChunkPos { x: 0, z: 0 }],
            "render-neighbor invalidation must not dirty adjacent persistence"
        );
    }
    #[test]
    fn spawn_is_derived_above_surface() {
        let mut registry = BlockRegistry::default();
        registry.register(STONE).unwrap();
        let mut world = World::new(BlockId(0));
        world.set(BlockPos { x: 0, y: 4, z: 0 }, STONE.id);
        assert_eq!(
            Simulation::spawn_above_surface(&world, &registry, 0, 0),
            Vec3::new(0.5, 6.0, 0.5)
        );
    }
    #[test]
    fn intent_moves_player() {
        let mut registry = BlockRegistry::default();
        registry.register(STONE).unwrap();
        let mut sim = Simulation::new(World::new(BlockId(0)), registry, Vec3::ZERO);
        sim.step(
            AgentIntent {
                movement: MoveIntent {
                    forward: 1.0,
                    strafe: 0.0,
                },
                ..Default::default()
            },
            0.1,
        );
        assert!(sim.player.position.z > 0.0);
        let _ = PlaceIntent {
            position: BlockPos { x: 0, y: 0, z: 0 },
            block: rustcraft_game_api::BlockKey::parse(STONE.name).unwrap(),
        };
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::*;
    use rustcraft_engine_core::ItemId;
    use rustcraft_mod_api::{BlockDefinition, ItemDefinition};
    fn scene() -> Simulation {
        let mut r = BlockRegistry::default();
        for (id, name) in [(1, "mod:stone"), (2, "mod:wood")] {
            r.register(BlockDefinition::cube(id, name, "mod:texture"))
                .unwrap();
            r.register_item(ItemDefinition {
                id: ItemId(id),
                name,
                max_stack: 4,
                placeable: Some(BlockId(id)),
                capabilities: &[],
                tool: None,
            })
            .unwrap();
        }
        r.register_item(ItemDefinition {
            id: ItemId(3),
            name: "mod:nonplaceable",
            max_stack: 1,
            placeable: None,
            capabilities: &["future"],
            tool: None,
        })
        .unwrap();
        let mut w = World::new(BlockId(0));
        w.set(BlockPos { x: 15, y: 16, z: 3 }, BlockId(1));
        Simulation::new(w, r, Vec3::new(15.5, 15., 0.5))
    }
    fn placement_profile(
        reverse: bool,
        omit_wood: bool,
    ) -> rustcraft_game_api::CompiledGameProfile {
        use rustcraft_content::{NamespacedId, PackageId};
        use rustcraft_game_api::*;
        let mut authored = GameRegistry::default();
        let package = |n: &str| PackageId::parse(n).unwrap();
        for (owner, keys) in [
            ("mod:base", vec!["mod:air", "mod:stone", "mod:wood"]),
            ("extra:base", vec!["extra:a", "extra:b"]),
        ] {
            authored.register_package(package(owner)).unwrap();
            for name in keys {
                if omit_wood && name == "mod:wood" {
                    continue;
                }
                authored
                    .register_block(VoxelDefinition {
                        key: BlockKey::parse(name).unwrap(),
                        collision: CollisionDescriptor::Empty,
                        targetable: true,
                        material: MaterialClass::Opaque,
                        textures: FaceResources::All(TextureKey::parse("mod:texture").unwrap()),
                        light: LightDescriptor::default(),
                        base_rotation: rustcraft_engine_core::orientation::ModelRotation::IDENTITY,
                        orientation: rustcraft_engine_core::orientation::OrientationProperty::None,
                        face_tints: [[u16::MAX; 3]; 6],
                        capabilities: vec![],
                    })
                    .unwrap();
            }
        }
        let mut packages = vec![package("mod:base"), package("extra:base")];
        if reverse {
            packages.reverse();
        }
        authored
            .compile(&GameProfile {
                id: NamespacedId::parse("mod:profile").unwrap(),
                packages,
                resources: vec![],
                systems: vec![],
                manifest: Default::default(),
                default_block: BlockKey::parse("mod:air").unwrap(),
            })
            .unwrap()
    }
    #[test]
    fn a1_semantic_placement_resolves_reordered_profiles_and_rejects_invalid_mapping() {
        use rustcraft_agent_api::PlaceIntent;
        use rustcraft_game_api::BlockKey;
        let key = BlockKey::parse("mod:wood").unwrap();
        assert_ne!(
            placement_profile(false, false).block_id(&key),
            placement_profile(true, false).block_id(&key)
        );
        for reverse in [false, true] {
            let mut s = scene();
            let place = PlaceIntent {
                position: s.target().unwrap().adjacent,
                block: key.clone(),
            };
            assert!(
                s.place_semantic(&place)
                    .unwrap_err()
                    .contains("profile unavailable")
            );
            assert!(
                s.bind_content_profile(placement_profile(reverse, true))
                    .is_err()
            );
            s.bind_content_profile(placement_profile(reverse, false))
                .unwrap();
            let unknown = PlaceIntent {
                block: BlockKey::parse("mod:unknown").unwrap(),
                ..place.clone()
            };
            assert!(
                s.place_semantic(&unknown)
                    .unwrap_err()
                    .contains("unknown profile block")
            );
            s.step(
                AgentIntent {
                    game: MinecraftActions {
                        place_block: Some(unknown),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                0.,
            );
            assert!(
                s.observe(1)
                    .last_action_error
                    .unwrap()
                    .contains("unknown profile block")
            );
            s.inventory.insert(ItemId(2), 2, &s.registry);
            s.step(
                AgentIntent {
                    game: MinecraftActions {
                        place_block: Some(place.clone()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                0.,
            );
            assert!(s.last_action_error.is_none());
            assert_eq!(s.world.get(place.position), BlockId(2));
            assert_eq!(s.inventory.held().unwrap().count, 1);
            assert!(s.place_semantic(&place).is_err());
            assert_eq!(s.inventory.held().unwrap().count, 1);
        }
    }

    #[test]
    fn rsm1_saved_entity_metadata_retires_with_owner() {
        let mut s = scene();
        for x in 0..1000 {
            let column = ChunkPos { x, z: 0 };
            s.spawn_item(ItemId(1), 1, Vec3::new(x as f32 * 16. + 1., 2., 1.));
            let saved = s.entity_column_snapshot(column);
            s.note_entity_column_persisted(&saved);
            s.evict_entity_column(column);
        }
        let c = s.entity_lifetime_counts();
        assert_eq!(s.entity_lifetime_counts()[..5], [0; 5]);
        println!(
            "RSM1_ENTITY_ACCEPTED active={} durable={} tombstones={} receipts={} frozen={} capacity={}",
            c[0], c[1], c[2], c[3], c[4], c[5]
        );
    }

    #[test]
    fn rsm1_evicted_transfer_keeps_recovery_until_source_acknowledgement() {
        let mut s = scene();
        let source = ChunkPos { x: 0, z: 0 };
        let destination = ChunkPos { x: 1, z: 0 };
        s.spawn_item(ItemId(1), 1, Vec3::new(15., 2., 1.));
        let id = s.items[0].id;
        let saved = s.entity_column_snapshot(source);
        s.note_entity_column_persisted(&saved);
        s.items[0].position.x = 17.;
        s.items[0].persistence_revision += 1;
        let saved = s.entity_column_snapshot(destination);
        s.note_entity_column_persisted(&saved);
        s.world.remove_column(destination);
        s.evict_entity_column(destination);
        assert!(s.entity_durable.contains_key(&id));
        assert_eq!(
            s.entity_lifetime_counts()[2],
            1,
            "unfinished source cleanup retains transfer marker"
        );
        let cleaned = s.entity_column_snapshot(source);
        s.note_entity_column_persisted(&cleaned);
        assert!(!s.entity_durable.contains_key(&id));
        assert_eq!(s.entity_lifetime_counts()[2], 0);
    }
    #[test]
    fn selection_target_face_count_and_boundary_invalidation() {
        let mut s = scene();
        s.inventory.insert(ItemId(1), 4, &s.registry);
        s.inventory.insert(ItemId(2), 2, &s.registry);
        s.take_dirty_sections();
        let hit = s.target().unwrap();
        assert_eq!(hit.normal, [0, 0, -1]);
        s.step(
            AgentIntent {
                game: MinecraftActions {
                    select_hotbar: Some(1),
                    use_action: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            0.,
        );
        assert_eq!(s.world.get(hit.adjacent), BlockId(2));
        assert_eq!(s.inventory.held().unwrap().count, 1);
        let dirty = s.take_dirty_sections();
        assert!(dirty.contains(&(ChunkPos { x: 0, z: 0 }, 1)));
        assert!(dirty.contains(&(ChunkPos { x: 1, z: 0 }, 1)));
        assert!(dirty.contains(&(ChunkPos { x: 0, z: 0 }, 0)));
        let o = s.observe(4);
        assert_eq!(o.game.selected_hotbar, 1);
        assert_eq!(o.game.inventory[1].as_ref().unwrap().item_key, "mod:wood");
        assert!(
            o.items
                .iter()
                .any(|i| i.key == "mod:nonplaceable" && i.capabilities == ["future"])
        );
        assert!(o.blocks.iter().any(|b| b.key == "mod:wood"));
        s.step(
            AgentIntent {
                game: MinecraftActions {
                    attack: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            0.,
        );
        assert_eq!(s.world.get(hit.adjacent), BlockId(0));
    }
    #[test]
    fn invalid_placement_never_spends_inventory() {
        let mut s = scene();
        let adjacent = s.target().unwrap().adjacent;
        assert!(!s.place_block(adjacent, BlockId(1))); // empty
        s.inventory.insert(ItemId(1), 2, &s.registry);
        assert!(!s.place_block(adjacent, BlockId(2))); // wrong selected item
        assert!(!s.place_block(BlockPos { x: 15, y: 16, z: 3 }, BlockId(1))); // occupied
        assert!(!s.place_block(BlockPos { x: 15, y: 15, z: 0 }, BlockId(1))); // body
        assert_eq!(s.inventory.held().unwrap().count, 2);
        s.inventory.insert(ItemId(3), 1, &s.registry);
        s.step(
            AgentIntent {
                game: MinecraftActions {
                    select_hotbar: Some(1),
                    use_action: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            0.,
        );
        assert_eq!(s.world.get(adjacent), BlockId(0));
        assert_eq!(s.inventory.held().unwrap().count, 1);
        s.step(
            AgentIntent {
                game: MinecraftActions {
                    select_hotbar: Some(8),
                    use_action: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            0.,
        );
        assert_eq!(s.world.get(adjacent), BlockId(0));
    }

    #[test]
    fn stable_entity_identity_and_cross_column_recovery_choose_one_newest_copy() {
        let mut s = scene();
        s.entity_namespace = 44;
        s.next_entity = 1;
        s.spawn_item(ItemId(1), 2, Vec3::new(15.75, 18.0, 0.5));
        let id = s.items[0].id;
        assert_eq!(id, EntityId::from_parts(44, 1));
        let old = s.items[0];
        s.items.clear();
        s.activate_entity_column(ChunkPos { x: 0, z: 0 }, vec![old], &[])
            .unwrap();
        let mut moved = old;
        moved.position.x = 16.25;
        moved.persistence_revision += 1;
        s.activate_entity_column(
            ChunkPos { x: 1, z: 0 },
            vec![moved],
            &[(id, ChunkPos { x: 0, z: 0 }, moved.persistence_revision)],
        )
        .unwrap();
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.items[0], moved);
        s.activate_entity_column(ChunkPos { x: 0, z: 0 }, vec![old], &[])
            .unwrap();
        assert_eq!(s.items.len(), 1, "stale source copy was suppressed");
        assert_eq!(s.items[0], moved);

        // Destination persisted, source already cleaned, final destination tombstone not pruned.
        let mut after_source_cleanup = scene();
        after_source_cleanup.items.clear();
        after_source_cleanup
            .activate_entity_column(
                ChunkPos { x: 1, z: 0 },
                vec![moved],
                &[(id, ChunkPos { x: 0, z: 0 }, moved.persistence_revision)],
            )
            .unwrap();
        after_source_cleanup
            .activate_entity_column(ChunkPos { x: 0, z: 0 }, Vec::new(), &[])
            .unwrap();
        assert_eq!(after_source_cleanup.items, vec![moved]);

        // A stale destination must not override a newer source merely because it has an old
        // tombstone. Revision comparison makes both load orders deterministic.
        let mut newer_source = old;
        newer_source.persistence_revision = moved.persistence_revision + 1;
        for destination_first in [false, true] {
            let mut recovered = scene();
            recovered.items.clear();
            if destination_first {
                recovered
                    .activate_entity_column(
                        ChunkPos { x: 1, z: 0 },
                        vec![moved],
                        &[(id, ChunkPos { x: 0, z: 0 }, moved.persistence_revision)],
                    )
                    .unwrap();
                recovered
                    .activate_entity_column(ChunkPos { x: 0, z: 0 }, vec![newer_source], &[])
                    .unwrap();
            } else {
                recovered
                    .activate_entity_column(ChunkPos { x: 0, z: 0 }, vec![newer_source], &[])
                    .unwrap();
                recovered
                    .activate_entity_column(
                        ChunkPos { x: 1, z: 0 },
                        vec![moved],
                        &[(id, ChunkPos { x: 0, z: 0 }, moved.persistence_revision)],
                    )
                    .unwrap();
            }
            assert_eq!(recovered.items, vec![newer_source]);
        }
    }

    #[test]
    fn pickup_receipt_orders_inventory_before_world_deletion() {
        let mut s = scene();
        s.player.position = Vec3::new(0.5, 2.0, 0.5);
        s.spawn_item(ItemId(1), 2, Vec3::new(0.5, 2.0, 0.5));
        s.items[0].pickup_delay = 0.0;
        let id = s.items[0].id;
        s.pickup_items();
        assert!(s.items.is_empty());
        assert_eq!(s.inventory.slots()[0].unwrap().count, 2);
        assert_eq!(s.pickup_receipts()[0].entity_id, id);
        assert!(
            s.take_persistence_dirty_chunks()
                .contains(&ChunkPos { x: 0, z: 0 })
        );
        let premature = s.entity_column_snapshot(ChunkPos { x: 0, z: 0 });
        assert!(!s.note_entity_column_persisted(&premature));
        assert_eq!(s.pickup_receipts().len(), 1);

        // Simulate the inventory+receipt checkpoint acknowledgement, followed by the source
        // column checkpoint. A stale source record remains suppressed throughout.
        s.commit_pickup_receipts(&s.pickup_receipts());
        let snapshot = s.entity_column_snapshot(ChunkPos { x: 0, z: 0 });
        assert!(s.note_entity_column_persisted(&snapshot));
        assert!(s.pickup_receipts().is_empty());
    }

    #[test]
    fn near_despawn_item_expires_once_instead_of_freezing_forever() {
        let mut s = scene();
        s.spawn_item(ItemId(1), 1, Vec3::new(0.5, 20.0, 0.5));
        s.items[0].age = 299.99;
        s.step(AgentIntent::default(), 0.02);
        assert!(s.items.is_empty());
        assert!(
            s.take_persistence_dirty_chunks()
                .contains(&ChunkPos { x: 0, z: 0 })
        );
    }
}

#[cfg(test)]
mod a1_observation_tests {
    use super::*;
    #[test]
    fn bot_merge_retires_consumed_identity_without_reusing_list_indices() {
        let mut registry = BlockRegistry::default();
        registry
            .register_item(rustcraft_mod_api::ItemDefinition {
                id: rustcraft_engine_core::ItemId(1),
                name: "fixture:stack",
                max_stack: 64,
                placeable: None,
                capabilities: &[],
                tool: None,
            })
            .unwrap();
        let mut sim =
            Simulation::new_with_entity_namespace(World::new(BlockId(0)), registry, Vec3::ZERO, 42);
        for _ in 0..2 {
            sim.spawn_item(rustcraft_engine_core::ItemId(1), 1, Vec3::new(5., 3., 5.));
        }
        let ids: Vec<_> = sim.observe(0).nearby_items.iter().map(|e| e.id).collect();
        sim.merge_items();
        let merged = sim.observe(0).nearby_items;
        assert_eq!(merged.len(), 1);
        assert!(ids.contains(&merged[0].id));
        assert_eq!(merged[0].count, 2);
        sim.items.clear();
        assert!(sim.observe(0).nearby_items.is_empty());
        sim.spawn_item(rustcraft_engine_core::ItemId(1), 1, Vec3::ZERO);
        assert!(!ids.contains(&sim.observe(0).nearby_items[0].id));
    }
    #[test]
    fn bot_entity_identity_survives_movement_reorder_and_column_reactivation() {
        let mut registry = BlockRegistry::default();
        registry
            .register(rustcraft_mod_api::BlockDefinition::cube(
                1,
                "fixture:stone",
                "fixture:texture",
            ))
            .unwrap();
        registry
            .register_item(rustcraft_mod_api::ItemDefinition {
                id: rustcraft_engine_core::ItemId(1),
                name: "fixture:stone",
                max_stack: 64,
                placeable: Some(BlockId(1)),
                capabilities: &[],
                tool: None,
            })
            .unwrap();
        let mut sim =
            Simulation::new_with_entity_namespace(World::new(BlockId(0)), registry, Vec3::ZERO, 42);
        sim.spawn_item(rustcraft_engine_core::ItemId(1), 1, Vec3::new(0., 3., 0.));
        sim.spawn_item(rustcraft_engine_core::ItemId(1), 1, Vec3::new(1., 3., 0.));
        let first = sim.observe(2).nearby_items;
        assert_ne!(first[0].id, first[1].id);
        sim.items[0].position.x = 17.;
        sim.items.reverse();
        let moved = sim.observe(2).nearby_items;
        assert_eq!(moved[1].id, first[0].id);
        assert_eq!(moved[1].position.x, 17.);
        let column = sim.items[1].column();
        let snapshot = sim.entity_column_snapshot(column);
        let mut reopened = Simulation::new_with_entity_namespace(
            World::new(BlockId(0)),
            sim.registry.clone(),
            Vec3::ZERO,
            42,
        );
        reopened
            .activate_entity_column(column, snapshot.records, &snapshot.tombstones)
            .unwrap();
        assert_eq!(reopened.observe(2).nearby_items[0].id, first[0].id);
    }
}
