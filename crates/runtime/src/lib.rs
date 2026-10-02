//! Headless simulation orchestration shared by the server, tests and future clients.

use rustcraft_agent_api::{AgentIntent, Controller};
use rustcraft_bot_api::{BOT_API_VERSION, NearbyBlockObservation, Observation, SelfObservation};
use rustcraft_content::ContentManifest;
use rustcraft_engine_core::{Aabb, BlockId, BlockPos, ChunkPos, Vec3, World, split_block};
use rustcraft_mod_api::{BlockRegistry, GameplayModule, ModuleId, RegistrationError};
use std::collections::{HashMap, HashSet};
pub mod inventory;
pub mod lighting;
pub mod metrics;
pub mod survival;
use inventory::Inventory;
use inventory::ItemStack;
use lighting::{InitialLightingResult, Lighting, dirty_neighbors};
use survival::{GameMode, ItemEntity, RecipeRegistry, tool_speed};

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
    pub crafting_grid: [Option<ItemStack>; 4],
    /// Authoritative inventory transaction stack held by the UI cursor.
    pub inventory_cursor: Option<ItemStack>,
    pub mining: Option<MiningState>,
    next_entity: u64,
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
    pub fn new(mut world: World, registry: BlockRegistry, spawn: Vec3) -> Self {
        let dirty_chunks = world.chunk_positions().collect();
        let dirty_sections = world.section_positions().collect();
        let lighting = Lighting::initialize(&mut world, &registry);
        let mut recipes = RecipeRegistry::default();
        recipes.add_defaults(&registry);
        Self {
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
            crafting_grid: [None; 4],
            inventory_cursor: None,
            mining: None,
            next_entity: 1,
        }
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
    pub fn step(&mut self, intent: AgentIntent, dt: f32) {
        // Transitional M0-M3 adapter. Platform/controller input is generic; this legacy
        // simulation still owns the Minecraft mapping until its systems move to the game package.
        let primary_action = intent.primary_action || intent.attack;
        let secondary_action = intent.secondary_action || intent.use_action;
        if let Some(slot) = intent.select_hotbar {
            self.inventory.select(slot as usize);
        }
        self.inventory.scroll(i32::from(intent.scroll_hotbar));
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
        for entity in &mut self.items {
            entity.tick(&self.world, &self.registry, dt);
        }
        self.merge_items();
        self.pickup_items();
        if self.mode == GameMode::Survival && primary_action {
            self.mine_tick(dt);
        } else if self.mode == GameMode::Survival {
            self.mining = None;
        }
        if let Some(position) = intent.break_block
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
        if let Some(place) = intent.place_block
            && self
                .target()
                .is_some_and(|hit| hit.normal != [0; 3] && hit.adjacent == place.position)
        {
            let _ = self.place_block(place.position, place.block);
        }
        if intent.craft {
            let _ = self.take_crafting_output();
        }
        self.time = self.time.saturating_add(1);
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
                .registry
                .get(self.world.get(position))
                .and_then(|b| b.drop);
            self.world.set(position, self.world.empty_block());
            self.mark_dirty(position);
            if self.mode == GameMode::Survival
                && let Some(item) = drop
            {
                self.spawn_item(
                    item,
                    1,
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
        self.items.push(ItemEntity {
            id: self.next_entity,
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
        self.next_entity += 1;
    }
    fn merge_items(&mut self) {
        for i in 0..self.items.len() {
            for j in ((i + 1)..self.items.len()).rev() {
                if self.items[i].stack.item == self.items[j].stack.item
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
                    self.items[i].stack.count += take;
                    self.items[j].stack.count -= take;
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
            let e = &self.items[i];
            let d = e.position - self.player.position;
            if e.pickup_delay <= 0. && d.x * d.x + d.y * d.y + d.z * d.z < radius * radius {
                let rem = self.inventory.insert_partial(e.stack, &self.registry);
                if rem.count < e.stack.count {
                    if rem.count == 0 {
                        self.items.remove(i);
                        continue;
                    }
                    self.items[i].stack = rem;
                }
            }
            i += 1;
        }
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
        let speed = tool_speed(&self.registry, self.inventory.held(), block_id);
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
        state.progress += dt * speed / b.hardness.max(0.05);
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
    pub fn observe(&self, radius: i32) -> Observation {
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
            self_state: SelfObservation {
                position: self.player.position,
                velocity: self.player.velocity,
            },
            nearby_blocks: nearby,
            inventory: self
                .inventory
                .slots()
                .iter()
                .map(|slot| {
                    slot.and_then(|s| {
                        self.registry
                            .item(s.item)
                            .map(|d| rustcraft_bot_api::StackObservation {
                                item_key: d.name.into(),
                                count: s.count,
                            })
                    })
                })
                .collect(),
            selected_hotbar: self.inventory.selected() as u8,
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
                    item_key: self
                        .registry
                        .item(e.stack.item)
                        .map_or("unknown".into(), |d| d.name.into()),
                    count: e.stack.count,
                    position: e.position,
                })
                .collect(),
            mining_progress: self.mining.map(|m| m.progress),
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

pub fn run_controller<C: Controller>(
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
            block: STONE.id,
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
                select_hotbar: Some(1),
                use_action: true,
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
        assert_eq!(o.selected_hotbar, 1);
        assert_eq!(o.inventory[1].as_ref().unwrap().item_key, "mod:wood");
        assert!(
            o.items
                .iter()
                .any(|i| i.key == "mod:nonplaceable" && i.capabilities == ["future"])
        );
        assert!(o.blocks.iter().any(|b| b.key == "mod:wood"));
        s.step(
            AgentIntent {
                attack: true,
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
                select_hotbar: Some(1),
                use_action: true,
                ..Default::default()
            },
            0.,
        );
        assert_eq!(s.world.get(adjacent), BlockId(0));
        assert_eq!(s.inventory.held().unwrap().count, 1);
        s.step(
            AgentIntent {
                select_hotbar: Some(8),
                use_action: true,
                ..Default::default()
            },
            0.,
        );
        assert_eq!(s.world.get(adjacent), BlockId(0));
    }
}
