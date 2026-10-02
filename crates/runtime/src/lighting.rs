//! Two packed channels in World; direct sky seeds plus deterministic local relaxation.
//! Removal converges because every propagated edge costs at least one light level.
use rustcraft_engine_core::{
    BlockId, BlockPos, Chunk, ChunkPos, SectionPos, VoxelLight, World, block_index, split_block,
};
use rustcraft_mod_api::BlockRegistry;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Instant;

// Bound post-publication reconciliation independently from the initial-light worker queue.
// Radius-3 residency has 29 desired columns, so eight slots could pin completed, locally-lit
// columns behind unrelated boundary work and prevent them from becoming render-ready.
// Boundary reconciliation is eventual and must not prevent locally complete columns from being
// published. This bound covers the selected radius-4 working set and the full radius-6 sweep,
// while remaining finite under rapid turns/reversals.
const MAX_COLUMN_LIGHTING_JOBS: usize = 256;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LightingWorkCounters {
    pub emitters_found: u64,
    pub direct_voxels_scanned: u64,
    pub boundary_voxels_inspected: u64,
    pub propagation_queue_pushes: u64,
    pub propagation_queue_pops: u64,
    pub light_writes: u64,
    pub dirty_section_insertions: u64,
    pub columns_started: u64,
    pub columns_completed: u64,
}

pub fn neighbors(p: BlockPos) -> [BlockPos; 6] {
    [
        BlockPos { x: p.x + 1, ..p },
        BlockPos { x: p.x - 1, ..p },
        BlockPos { y: p.y + 1, ..p },
        BlockPos { y: p.y - 1, ..p },
        BlockPos { z: p.z + 1, ..p },
        BlockPos { z: p.z - 1, ..p },
    ]
}
pub fn section(p: BlockPos) -> SectionPos {
    (
        ChunkPos {
            x: p.x.div_euclid(16),
            z: p.z.div_euclid(16),
        },
        p.y.div_euclid(16),
    )
}
pub fn dirty_neighbors(dirty: &mut HashSet<SectionPos>, p: BlockPos) {
    dirty.insert(section(p));
    for n in neighbors(p) {
        dirty.insert(section(n));
    }
}

fn mark_light_dirty(dirty: &mut HashSet<SectionPos>, p: BlockPos) {
    let (column, (x, y, z)) = split_block(p);
    let section_y = p.y.div_euclid(16);
    dirty.insert((column, section_y));
    if x == 0 {
        dirty.insert((
            ChunkPos {
                x: column.x - 1,
                ..column
            },
            section_y,
        ));
    }
    if x == 15 {
        dirty.insert((
            ChunkPos {
                x: column.x + 1,
                ..column
            },
            section_y,
        ));
    }
    if z == 0 {
        dirty.insert((
            ChunkPos {
                z: column.z - 1,
                ..column
            },
            section_y,
        ));
    }
    if z == 15 {
        dirty.insert((
            ChunkPos {
                z: column.z + 1,
                ..column
            },
            section_y,
        ));
    }
    if y == 0 {
        dirty.insert((column, section_y - 1));
    }
    if y == 15 {
        dirty.insert((column, section_y + 1));
    }
}

#[derive(Debug, Default)]
pub struct Lighting {
    direct: HashMap<SectionPos, Vec<u8>>,
    columns: HashMap<ChunkPos, (i32, i32)>,
    pub initial_ms: f64,
    pub last_update_ms: f64,
    pub last_visited: usize,
    integration: Option<ColumnIntegration>,
    queued_integrations: VecDeque<QueuedColumnIntegration>,
    queued_removals: VecDeque<ChunkPos>,
    completed_sections: HashMap<ChunkPos, Vec<i32>>,
    work_counters: LightingWorkCounters,
}

#[derive(Debug)]
struct ColumnIntegration {
    column: ChunkPos,
    low: i32,
    section_ys: Vec<i32>,
    seed_cursor: usize,
    seed_total: usize,
    seed_cost: usize,
    seed_candidates: Vec<BlockPos>,
    candidate_cursor: usize,
    boundary_faces: Vec<usize>,
    boundary_cursor: usize,
    boundary_total: usize,
    queue: VecDeque<BlockPos>,
    queued: HashSet<BlockPos>,
    dirty_sections: HashSet<SectionPos>,
}

#[derive(Debug)]
struct QueuedColumnIntegration {
    column: ChunkPos,
    section_ys: Vec<i32>,
    queued_at: Instant,
    direct_ready: bool,
}

/// Fully constructed derived-light data for an unpublished voxel column.
#[derive(Debug)]
pub struct InitialLightingResult {
    pub position: ChunkPos,
    pub token: u64,
    pub sections: Vec<(i32, Chunk)>,
    pub light_sections: Vec<(i32, Vec<VoxelLight>)>,
    pub(crate) direct_sections: Vec<(i32, Vec<u8>)>,
    pub queue_wait_ms: f64,
    pub worker_elapsed_ms: f64,
    pub persist_new: bool,
    pub work_counters: LightingWorkCounters,
    pub error: Option<&'static str>,
}

struct InitialLightingJob {
    position: ChunkPos,
    token: u64,
    priority: (i64, i64, i32, i32),
    queued_at: Instant,
    default_block: BlockId,
    sections: Vec<(i32, Chunk)>,
    registry: Arc<BlockRegistry>,
    persist_new: bool,
}

pub struct InitialLightingRequest {
    pub position: ChunkPos,
    pub token: u64,
    pub priority: (i64, i64, i32, i32),
    pub default_block: BlockId,
    pub sections: Vec<(i32, Chunk)>,
    pub registry: Arc<BlockRegistry>,
    pub persist_new: bool,
}

#[derive(Default)]
struct InitialLightingQueue {
    jobs: Vec<InitialLightingJob>,
    stopped: bool,
}

/// Bounded non-preemptive workers for bulk initial lighting. The active task is allowed to
/// finish; pending tasks are re-ranked at dequeue using current interest priority plus aging.
pub struct InitialLightingScheduler {
    queue: Arc<(Mutex<InitialLightingQueue>, Condvar)>,
    results: mpsc::Receiver<InitialLightingResult>,
    workers: Vec<JoinHandle<()>>,
    capacity: usize,
    outstanding: Arc<AtomicUsize>,
    in_flight: Arc<AtomicUsize>,
    latest: HashMap<ChunkPos, u64>,
    pub submitted: u64,
    pub coalesced: u64,
    pub stale: u64,
    pub completed: u64,
    first_submit_at: Option<Instant>,
}

impl InitialLightingScheduler {
    #[must_use]
    pub fn new(worker_count: usize, capacity: usize) -> Self {
        let worker_count = worker_count.max(1);
        let capacity = capacity.max(worker_count);
        let queue = Arc::new((Mutex::new(InitialLightingQueue::default()), Condvar::new()));
        let (result_tx, results) = mpsc::sync_channel(capacity);
        let outstanding = Arc::new(AtomicUsize::new(0));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let mut workers = Vec::with_capacity(worker_count);
        for index in 0..worker_count {
            let queue = queue.clone();
            let results = result_tx.clone();
            let in_flight = in_flight.clone();
            workers.push(
                thread::Builder::new()
                    .name(format!("initial-light-{index}"))
                    .spawn(move || {
                        loop {
                            let job = {
                                let (lock, wake) = &*queue;
                                let mut state =
                                    lock.lock().expect("initial lighting queue poisoned");
                                while state.jobs.is_empty() && !state.stopped {
                                    state =
                                        wake.wait(state).expect("initial lighting queue poisoned");
                                }
                                if state.stopped && state.jobs.is_empty() {
                                    break;
                                }
                                let now = Instant::now();
                                let index = state
                                    .jobs
                                    .iter()
                                    .enumerate()
                                    .min_by_key(|(_, job)| {
                                        const AGING_PER_SECOND: i64 = 20_000_000;
                                        let age_ms = now
                                            .saturating_duration_since(job.queued_at)
                                            .as_millis()
                                            .min(i64::MAX as u128)
                                            as i64;
                                        let mut key = job.priority;
                                        key.0 = key.0.saturating_sub(
                                            age_ms.saturating_mul(AGING_PER_SECOND) / 1000,
                                        );
                                        key
                                    })
                                    .map(|(index, _)| index)
                                    .expect("non-empty queue");
                                state.jobs.swap_remove(index)
                            };
                            in_flight.fetch_add(1, Ordering::Relaxed);
                            let queue_wait_ms = job.queued_at.elapsed().as_secs_f64() * 1000.0;
                            let started = Instant::now();
                            let failed_identity = (job.position, job.token, job.persist_new);
                            let result = build_initial_lighting(job).unwrap_or_else(|error| {
                                InitialLightingResult {
                                    position: failed_identity.0,
                                    token: failed_identity.1,
                                    sections: Vec::new(),
                                    light_sections: Vec::new(),
                                    direct_sections: Vec::new(),
                                    queue_wait_ms: 0.0,
                                    worker_elapsed_ms: 0.0,
                                    persist_new: failed_identity.2,
                                    work_counters: LightingWorkCounters::default(),
                                    error: Some(error),
                                }
                            });
                            let worker_elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
                            let mut result = result;
                            result.queue_wait_ms = queue_wait_ms;
                            result.worker_elapsed_ms = worker_elapsed_ms;
                            if results.send(result).is_err() {
                                in_flight.fetch_sub(1, Ordering::Relaxed);
                                break;
                            }
                            in_flight.fetch_sub(1, Ordering::Relaxed);
                        }
                    })
                    .expect("initial lighting worker thread creation failed"),
            );
        }
        drop(result_tx);
        Self {
            queue,
            results,
            workers,
            capacity,
            outstanding,
            in_flight,
            latest: HashMap::new(),
            submitted: 0,
            coalesced: 0,
            stale: 0,
            completed: 0,
            first_submit_at: None,
        }
    }

    pub fn submit(&mut self, request: InitialLightingRequest) -> Result<(), Vec<(i32, Chunk)>> {
        let InitialLightingRequest {
            position,
            token,
            priority,
            default_block,
            sections,
            registry,
            persist_new,
        } = request;
        let (lock, wake) = &*self.queue;
        let Ok(mut state) = lock.lock() else {
            return Err(sections);
        };
        if state
            .jobs
            .iter()
            .any(|job| job.position == position && job.token == token)
        {
            self.coalesced += 1;
            return Ok(());
        }
        if self.outstanding.load(Ordering::Relaxed) >= self.capacity {
            return Err(sections);
        }
        self.outstanding.fetch_add(1, Ordering::Relaxed);
        self.latest.insert(position, token);
        self.first_submit_at.get_or_insert_with(Instant::now);
        state.jobs.push(InitialLightingJob {
            position,
            token,
            priority,
            queued_at: Instant::now(),
            default_block,
            sections,
            registry,
            persist_new,
        });
        self.submitted += 1;
        wake.notify_one();
        Ok(())
    }

    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::Relaxed)
    }

    pub fn reprioritize(&mut self, mut priority: impl FnMut(ChunkPos) -> (i64, i64, i32, i32)) {
        if let Ok(mut state) = self.queue.0.lock() {
            for job in &mut state.jobs {
                job.priority = priority(job.position);
            }
        }
    }

    pub fn take_ready(&mut self, limit: usize) -> Vec<InitialLightingResult> {
        let mut ready = Vec::with_capacity(limit);
        while ready.len() < limit {
            let Ok(result) = self.results.try_recv() else {
                break;
            };
            self.completed += 1;
            self.outstanding.fetch_sub(1, Ordering::Relaxed);
            if self.latest.get(&result.position).copied() == Some(result.token) {
                self.latest.remove(&result.position);
                ready.push(result);
            } else {
                self.stale += 1;
            }
        }
        ready
    }

    #[must_use]
    pub fn pending(&self) -> usize {
        self.queue.0.lock().map_or(0, |state| state.jobs.len())
    }

    #[must_use]
    pub fn capacity_remaining(&self) -> usize {
        self.capacity
            .saturating_sub(self.outstanding.load(Ordering::Relaxed))
    }

    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.outstanding.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn columns_per_second(&self) -> f64 {
        self.first_submit_at.map_or(0.0, |started| {
            self.completed as f64 / started.elapsed().as_secs_f64().max(0.001)
        })
    }
}

impl Drop for InitialLightingScheduler {
    fn drop(&mut self) {
        let (lock, wake) = &*self.queue;
        if let Ok(mut state) = lock.lock() {
            state.stopped = true;
            wake.notify_all();
        }
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn build_initial_lighting(
    mut job: InitialLightingJob,
) -> Result<InitialLightingResult, &'static str> {
    let position = job.position;
    let token = job.token;
    let ys = job.sections.iter().map(|(y, _)| *y).collect::<Vec<_>>();
    let mut world = World::new(job.default_block);
    world.publish_column(position, std::mem::take(&mut job.sections))?;
    let lighting = Lighting::initialize(&mut world, &job.registry);
    let (Some(low), Some(high)) = (ys.iter().min().copied(), ys.iter().max().copied()) else {
        return Err("initial lighting requires at least one section");
    };
    let low = low - 1;
    let high = high + 1;
    let mut light_sections = Vec::with_capacity(ys.len());
    let mut direct_sections = Vec::with_capacity((high - low + 1) as usize);
    for &section_y in &ys {
        let light = world
            .section_lights(position, section_y)
            .map_or_else(|| vec![VoxelLight::default(); 4096], <[VoxelLight]>::to_vec);
        light_sections.push((section_y, light));
    }
    for section_y in low..=high {
        let direct = lighting
            .direct
            .get(&(position, section_y))
            .ok_or("initial lighting omitted a direct array")?
            .clone();
        direct_sections.push((section_y, direct));
    }
    let sections = world.remove_column(position);
    Ok(InitialLightingResult {
        position,
        token,
        sections,
        light_sections,
        direct_sections,
        queue_wait_ms: 0.0,
        worker_elapsed_ms: 0.0,
        persist_new: job.persist_new,
        work_counters: lighting.work_counters(),
        error: None,
    })
}
impl Lighting {
    /// Adopt worker-built direct-light inputs and queue only the resident-neighbor boundary
    /// reconciliation. The column itself is already fully lit and is never exposed half-ready.
    pub(crate) fn adopt_initial_column(
        &mut self,
        column: ChunkPos,
        section_ys: Vec<i32>,
        direct_sections: Vec<(i32, Vec<u8>)>,
    ) {
        let (Some(low), Some(high)) = (
            section_ys.iter().min().copied(),
            section_ys.iter().max().copied(),
        ) else {
            return;
        };
        let low = low - 1;
        let high = high + 1;
        self.columns.insert(column, (low, high));
        for (section_y, direct) in direct_sections {
            self.direct.insert((column, section_y), direct);
        }
        if self.integration.is_none() && self.queued_integrations.is_empty() {
            let _ = self.start_adopted_boundary(column, section_ys);
        } else if self.integration_columns().len() < MAX_COLUMN_LIGHTING_JOBS {
            self.queued_integrations.push_back(QueuedColumnIntegration {
                column,
                section_ys,
                queued_at: Instant::now(),
                direct_ready: true,
            });
        }
        self.work_counters.columns_started += 1;
    }

    pub fn initialize(world: &mut World, r: &BlockRegistry) -> Self {
        let started = Instant::now();
        let mut s = Self::default();
        for (c, y) in world.section_positions() {
            let b = s.columns.entry(c).or_insert((y, y));
            b.0 = b.0.min(y);
            b.1 = b.1.max(y);
        }
        // One sky section and one lower section per loaded column, including light-only air.
        for (&c, b) in &mut s.columns {
            b.0 -= 1;
            b.1 += 1;
            for y in b.0..=b.1 {
                s.direct.insert((c, y), vec![0; 4096]);
            }
        }
        let mut queue = VecDeque::new();
        let columns: Vec<_> = s.columns.keys().copied().collect();
        for c in columns {
            for z in 0..16 {
                for x in 0..16 {
                    s.seed_column(world, r, c.x * 16 + x, c.z * 16 + z, &mut queue);
                }
            }
        }
        let mut dirty = HashSet::new();
        s.propagate(world, r, queue, &mut dirty);
        s.initial_ms = started.elapsed().as_secs_f64() * 1000.;
        s
    }

    /// Add derived light state for one newly published column and propagate across any already
    /// resident boundaries. This is an incremental residency hook, not a world lighting rebuild.
    pub fn integrate_column(
        &mut self,
        world: &mut World,
        r: &BlockRegistry,
        column: ChunkPos,
        section_ys: Vec<i32>,
        dirty: &mut HashSet<SectionPos>,
    ) {
        self.begin_column_integration(column, section_ys);
        while !self.advance_column_integration(world, r, dirty, usize::MAX) {}
        self.completed_sections.remove(&column);
    }

    /// Begin incremental lighting for a newly published column. Call
    /// `advance_column_integration` from bounded simulation work until it returns true.
    pub fn begin_column_integration(&mut self, column: ChunkPos, section_ys: Vec<i32>) -> bool {
        if self.integration.is_some() || !self.queued_integrations.is_empty() {
            return false;
        }
        self.start_column_integration(column, section_ys)
    }

    /// Queue lighting for a published column. Active plus queued work is bounded so fast IO and
    /// generation cannot create unlimited lighting work or pin unlimited resident columns.
    pub fn queue_column_integration(&mut self, column: ChunkPos, section_ys: Vec<i32>) -> bool {
        if self.integration_columns().contains(&column) {
            return true;
        }
        if self.integration.is_none() {
            let _ = self.start_column_integration(column, section_ys);
            return true;
        }
        if self.integration_columns().len() >= MAX_COLUMN_LIGHTING_JOBS {
            return false;
        }
        self.queued_integrations.push_back(QueuedColumnIntegration {
            column,
            section_ys,
            queued_at: Instant::now(),
            direct_ready: false,
        });
        true
    }

    fn start_column_integration(&mut self, column: ChunkPos, ys: Vec<i32>) -> bool {
        let (Some(low), Some(high)) = (ys.iter().min().copied(), ys.iter().max().copied()) else {
            return true;
        };
        let low = low - 1;
        let high = high + 1;
        let boundary_faces = self.resident_boundary_faces(column);
        self.columns.insert(column, (low, high));
        for y in low..=high {
            self.direct
                .entry((column, y))
                .or_insert_with(|| vec![0; 4096]);
        }
        let vertical = usize::try_from((high + 1 - low) * 16).unwrap_or(0);
        // Bulk-import direct skylight in one cache-local pass. It is one vertical scan per x/z
        // ray, not a queue job per voxel; only shaded transparent cells and emitters need
        // relaxation. This avoids spending many fixed ticks rediscovering direct sunlight.
        let seed_total = 256;
        let seed_cost = vertical.max(1);
        let boundary_total = vertical.saturating_mul(16 * boundary_faces.len());
        self.integration = Some(ColumnIntegration {
            column,
            low,
            section_ys: ys,
            seed_cursor: 0,
            seed_total,
            seed_cost,
            seed_candidates: Vec::new(),
            candidate_cursor: 0,
            boundary_faces,
            boundary_cursor: 0,
            boundary_total,
            queue: VecDeque::new(),
            queued: HashSet::new(),
            dirty_sections: HashSet::new(),
        });
        self.work_counters.columns_started += 1;
        false
    }

    /// Start clearing light propagated from an evicted column without doing the boundary flood
    /// synchronously. Returns false while bounded cleanup remains in progress.
    pub fn begin_column_removal(&mut self, column: ChunkPos) -> bool {
        if self.has_integration_work() {
            return false;
        }
        self.start_column_removal(column)
    }

    /// Queue lighting cleanup for an evicted column behind current bounded integration work.
    /// The physical world column can be removed immediately; only adjacent-light reconciliation
    /// remains queued. Active work for the same column is pinned until it completes.
    pub fn queue_column_removal(&mut self, column: ChunkPos) -> bool {
        if self.integration_columns().contains(&column) {
            return false;
        }
        if !self.has_integration_work() {
            return self.start_column_removal(column);
        }
        if self.queued_integrations.len() + self.queued_removals.len() + 1
            >= MAX_COLUMN_LIGHTING_JOBS
        {
            return false;
        }
        self.queued_removals.push_back(column);
        true
    }

    fn start_column_removal(&mut self, column: ChunkPos) -> bool {
        self.remove_column(column);
        let neighbors = [
            ChunkPos {
                x: column.x - 1,
                z: column.z,
            },
            ChunkPos {
                x: column.x + 1,
                z: column.z,
            },
            ChunkPos {
                x: column.x,
                z: column.z - 1,
            },
            ChunkPos {
                x: column.x,
                z: column.z + 1,
            },
        ];
        let boundary_faces = self.resident_boundary_faces(column);
        let mut spans = neighbors
            .iter()
            .filter_map(|neighbor| self.columns.get(neighbor).copied());
        let Some(first) = spans.next() else {
            return true;
        };
        let (low, high) = spans.fold(first, |(low, high), span| {
            (low.min(span.0), high.max(span.1))
        });
        let vertical = usize::try_from((high + 1 - low) * 16).unwrap_or(0);
        self.integration = Some(ColumnIntegration {
            column,
            low,
            section_ys: Vec::new(),
            seed_cursor: 256,
            seed_total: 256,
            seed_cost: 1,
            seed_candidates: Vec::new(),
            candidate_cursor: 0,
            boundary_faces: boundary_faces.clone(),
            boundary_cursor: 0,
            boundary_total: vertical.saturating_mul(16 * boundary_faces.len()),
            queue: VecDeque::new(),
            queued: HashSet::new(),
            dirty_sections: HashSet::new(),
        });
        false
    }

    fn resident_boundary_faces(&self, column: ChunkPos) -> Vec<usize> {
        [
            ChunkPos {
                x: column.x - 1,
                z: column.z,
            },
            ChunkPos {
                x: column.x + 1,
                z: column.z,
            },
            ChunkPos {
                x: column.x,
                z: column.z - 1,
            },
            ChunkPos {
                x: column.x,
                z: column.z + 1,
            },
        ]
        .into_iter()
        .enumerate()
        .filter_map(|(face, neighbor)| self.columns.contains_key(&neighbor).then_some(face))
        .collect()
    }

    /// Process at most `budget` voxel seed/propagation visits. Returns true when the column's
    /// direct sky and boundary propagation have converged.
    pub fn advance_column_integration(
        &mut self,
        world: &mut World,
        r: &BlockRegistry,
        dirty: &mut HashSet<SectionPos>,
        budget: usize,
    ) -> bool {
        let Some(mut work) = self.integration.take() else {
            return true;
        };
        // A seed unit scans one complete vertical x/z column, while boundary and propagation
        // units touch one voxel. Charge the vertical scan conservatively so the work budget bounds
        // both phases rather than letting all 256 columns run in one call.
        let mut remaining = budget.max(work.seed_cost);
        let y_start = work.low * 16;
        while remaining >= work.seed_cost && work.seed_cursor < work.seed_total {
            let index = work.seed_cursor;
            let x = (index % 16) as i32;
            let z = (index / 16) as i32;
            self.seed_column_incremental(world, r, x, z, &mut work);
            work.seed_cursor += 1;
            remaining -= work.seed_cost;
        }
        while remaining > 0
            && work.seed_cursor == work.seed_total
            && work.candidate_cursor < work.seed_candidates.len()
        {
            let p = work.seed_candidates[work.candidate_cursor];
            work.candidate_cursor += 1;
            if self.needs_propagation_at(world, r, p) && work.queued.insert(p) {
                work.queue.push_back(p);
                self.work_counters.propagation_queue_pushes += 1;
            }
            remaining -= 1;
        }
        while remaining > 0
            && work.seed_cursor == work.seed_total
            && work.boundary_cursor < work.boundary_total
        {
            let index = work.boundary_cursor;
            let face_stride = work.boundary_faces.len() * 16;
            let y = y_start + (index / face_stride) as i32;
            let within = index % face_stride;
            let face = work.boundary_faces[within / 16];
            let offset = (within % 16) as i32;
            let c = work.column;
            let p = match face {
                0 => BlockPos {
                    x: c.x * 16 - 1,
                    y,
                    z: c.z * 16 + offset,
                },
                1 => BlockPos {
                    x: c.x * 16 + 16,
                    y,
                    z: c.z * 16 + offset,
                },
                2 => BlockPos {
                    x: c.x * 16 + offset,
                    y,
                    z: c.z * 16 - 1,
                },
                _ => BlockPos {
                    x: c.x * 16 + offset,
                    y,
                    z: c.z * 16 + 16,
                },
            };
            self.work_counters.boundary_voxels_inspected += 1;
            let source = self.source(p);
            if source.is_none() {
                work.boundary_cursor += 1;
                remaining -= 1;
                continue;
            }
            let light = world.light(p);
            let block = r.get(world.get(p));
            let emission = block.map_or(0, |block| block.emission);
            let can_receive_sky =
                block.is_none_or(|block| block.sky_opacity < 15 && block.light_opacity < 15);
            let boundary_needs_relaxation = (can_receive_sky
                && (source.is_some_and(|sky| sky < 15) || light.sky() < 15))
                || light.block() > 0
                || emission > 0;
            // A fresh neighboring column may introduce an emitter or changed direct-sky source
            // on the opposite side of an otherwise fully lit face. Inspect adjacent candidates
            // as well so arrival order cannot hide that newly introduced source.
            for candidate in std::iter::once(p).chain(neighbors(p)) {
                if (boundary_needs_relaxation || candidate != p)
                    && self.needs_propagation_at(world, r, candidate)
                    && work.queued.insert(candidate)
                {
                    work.queue.push_back(candidate);
                    self.work_counters.propagation_queue_pushes += 1;
                }
            }
            work.boundary_cursor += 1;
            remaining -= 1;
        }
        let done_seeding = work.seed_cursor == work.seed_total
            && work.candidate_cursor == work.seed_candidates.len()
            && work.boundary_cursor == work.boundary_total;
        let propagation_done = if remaining > 0 && done_seeding {
            self.propagate_work(
                world,
                r,
                &mut work.queue,
                &mut work.queued,
                &mut work.dirty_sections,
                remaining,
            )
        } else {
            work.queue.is_empty() && done_seeding
        };
        if propagation_done {
            dirty.extend(work.dirty_sections.drain());
            self.work_counters.columns_completed += 1;
            if !work.section_ys.is_empty() {
                self.completed_sections.insert(work.column, work.section_ys);
            }
            if let Some(removal) = self.queued_removals.pop_front() {
                let started = self.start_column_removal(removal);
                debug_assert!(!started || self.integration.is_none());
            } else if let Some(next) = self.queued_integrations.pop_front() {
                let started = if next.direct_ready {
                    self.start_adopted_boundary(next.column, next.section_ys)
                } else {
                    self.start_column_integration(next.column, next.section_ys)
                };
                debug_assert!(!started || self.integration.is_none());
            }
            true
        } else {
            self.integration = Some(work);
            false
        }
    }

    fn start_adopted_boundary(&mut self, column: ChunkPos, section_ys: Vec<i32>) -> bool {
        let (Some(low), Some(high)) = (
            section_ys.iter().min().copied(),
            section_ys.iter().max().copied(),
        ) else {
            return true;
        };
        let low = low - 1;
        let high = high + 1;
        let boundary_faces = self.resident_boundary_faces(column);
        let vertical = usize::try_from((high + 1 - low) * 16).unwrap_or(0);
        self.integration = Some(ColumnIntegration {
            column,
            low,
            section_ys,
            seed_cursor: 256,
            seed_total: 256,
            seed_cost: 1,
            seed_candidates: Vec::new(),
            candidate_cursor: 0,
            boundary_total: vertical.saturating_mul(16 * boundary_faces.len()),
            boundary_faces,
            boundary_cursor: 0,
            queue: VecDeque::new(),
            queued: HashSet::new(),
            dirty_sections: HashSet::new(),
        });
        false
    }

    pub fn integrating_column(&self) -> Option<ChunkPos> {
        self.integration.as_ref().map(|work| work.column)
    }

    pub fn take_completed_section_ys(&mut self, column: ChunkPos) -> Vec<i32> {
        self.completed_sections.remove(&column).unwrap_or_default()
    }

    pub fn integration_columns(&self) -> Vec<ChunkPos> {
        self.integration
            .iter()
            .map(|work| work.column)
            .chain(self.queued_integrations.iter().map(|job| job.column))
            .collect()
    }

    /// Reorder work not yet started according to current generic residency priority. Aging keeps
    /// a moving interest point from starving older retained columns indefinitely. The active
    /// column finishes to avoid partial-lighting cancellation churn.
    pub fn reprioritize_pending_columns(
        &mut self,
        mut priority: impl FnMut(ChunkPos) -> (i64, i64, i32, i32),
    ) {
        const AGING_SCORE_PER_SECOND: i64 = 20_000_000;
        let now = Instant::now();
        let mut pending = self.queued_integrations.drain(..).collect::<Vec<_>>();
        pending.sort_by_key(|job| {
            let mut key = priority(job.column);
            let waited_ms = now
                .saturating_duration_since(job.queued_at)
                .as_millis()
                .min(i64::MAX as u128) as i64;
            key.0 = key
                .0
                .saturating_sub(waited_ms.saturating_mul(AGING_SCORE_PER_SECOND) / 1000);
            key
        });
        self.queued_integrations.extend(pending);
    }

    pub fn integration_capacity_remaining(&self) -> usize {
        MAX_COLUMN_LIGHTING_JOBS.saturating_sub(self.integration_columns().len())
    }

    #[must_use]
    pub fn integrating_boundary_only(&self) -> bool {
        self.integration
            .as_ref()
            .is_some_and(|work| work.seed_cursor == work.seed_total)
    }

    pub fn can_queue_column_integration(&self, column: ChunkPos) -> bool {
        self.integration_columns().contains(&column) || self.integration_capacity_remaining() > 0
    }

    /// Drop obsolete not-yet-started lighting for a column that is leaving retention. Active work
    /// is not interrupted. Cancelling either a boundary-only or legacy queued integration is safe
    /// because the caller immediately removes the authoritative column and its derived light.
    pub fn cancel_queued_for_eviction(&mut self, column: ChunkPos) -> bool {
        let before = self.queued_integrations.len();
        self.queued_integrations.retain(|job| job.column != column);
        before != self.queued_integrations.len()
    }

    pub fn has_integration_work(&self) -> bool {
        self.integration.is_some()
            || !self.queued_integrations.is_empty()
            || !self.queued_removals.is_empty()
    }

    #[must_use]
    pub fn work_counters(&self) -> LightingWorkCounters {
        self.work_counters
    }

    pub fn remove_column(&mut self, column: ChunkPos) {
        self.cancel_queued_for_eviction(column);
        self.columns.remove(&column);
        self.direct.retain(|(position, _), _| *position != column);
    }

    pub fn remove_column_from_world(
        &mut self,
        world: &mut World,
        r: &BlockRegistry,
        column: ChunkPos,
        dirty: &mut HashSet<SectionPos>,
    ) {
        self.remove_column(column);
        let mut queue = VecDeque::new();
        for neighbor in [
            ChunkPos {
                x: column.x - 1,
                z: column.z,
            },
            ChunkPos {
                x: column.x + 1,
                z: column.z,
            },
            ChunkPos {
                x: column.x,
                z: column.z - 1,
            },
            ChunkPos {
                x: column.x,
                z: column.z + 1,
            },
        ] {
            let Some(&(low, high)) = self.columns.get(&neighbor) else {
                continue;
            };
            for y in low * 16..=(high + 1) * 16 - 1 {
                for offset in 0..16 {
                    let p = if neighbor.x < column.x {
                        BlockPos {
                            x: neighbor.x * 16 + 15,
                            y,
                            z: neighbor.z * 16 + offset,
                        }
                    } else if neighbor.x > column.x {
                        BlockPos {
                            x: neighbor.x * 16,
                            y,
                            z: neighbor.z * 16 + offset,
                        }
                    } else if neighbor.z < column.z {
                        BlockPos {
                            x: neighbor.x * 16 + offset,
                            y,
                            z: neighbor.z * 16 + 15,
                        }
                    } else {
                        BlockPos {
                            x: neighbor.x * 16 + offset,
                            y,
                            z: neighbor.z * 16,
                        }
                    };
                    queue.push_back(p);
                }
            }
        }
        self.propagate(world, r, queue, dirty);
    }
    fn seed_column(
        &mut self,
        world: &mut World,
        r: &BlockRegistry,
        x: i32,
        z: i32,
        queue: &mut VecDeque<BlockPos>,
    ) {
        let c = ChunkPos {
            x: x.div_euclid(16),
            z: z.div_euclid(16),
        };
        let Some(&(low, high)) = self.columns.get(&c) else {
            return;
        };
        let mut sky = 15_u8;
        for y in (low * 16..=(high + 1) * 16 - 1).rev() {
            let p = BlockPos { x, y, z };
            self.work_counters.direct_voxels_scanned += 1;
            let definition = r.get(world.get(p));
            if definition.is_some_and(|block| block.emission > 0) {
                self.work_counters.emitters_found += 1;
            }
            let opacity = definition.map_or(0, |b| b.sky_opacity);
            sky = sky.saturating_sub(opacity);
            let (_, local) = split_block(p);
            let seed = &mut self.direct.get_mut(&section(p)).unwrap()[block_index(local)];
            let previous_seed = *seed;
            *seed = sky;
            let previous_light = world.light(p);
            if previous_light.sky() != sky {
                world.set_light(p, VoxelLight::new(sky, previous_light.block()));
                self.work_counters.light_writes += 1;
            }
            let needs_propagation = definition.is_some_and(|block| {
                block.emission > 0
                    || previous_light.block() > 0
                    || (sky < 15 && block.sky_opacity < 15 && block.light_opacity < 15)
                    || (previous_seed != sky && sky < 15 && block.sky_opacity < 15)
            });
            if needs_propagation {
                queue.push_back(p);
                self.work_counters.propagation_queue_pushes += 1;
            }
        }
    }

    fn seed_column_incremental(
        &mut self,
        world: &mut World,
        r: &BlockRegistry,
        local_x: i32,
        local_z: i32,
        work: &mut ColumnIntegration,
    ) {
        let x = work.column.x * 16 + local_x;
        let z = work.column.z * 16 + local_z;
        let c = ChunkPos {
            x: x.div_euclid(16),
            z: z.div_euclid(16),
        };
        let Some(&(low, high)) = self.columns.get(&c) else {
            return;
        };
        let mut sky = 15_u8;
        for y in (low * 16..=(high + 1) * 16 - 1).rev() {
            let p = BlockPos { x, y, z };
            self.work_counters.direct_voxels_scanned += 1;
            let definition = r.get(world.get(p));
            if definition.is_some_and(|block| block.emission > 0) {
                self.work_counters.emitters_found += 1;
            }
            let opacity = definition.map_or(0, |b| b.sky_opacity);
            sky = sky.saturating_sub(opacity);
            let (_, local) = split_block(p);
            let index = block_index(local);
            let seed = &mut self
                .direct
                .get_mut(&section(p))
                .expect("column light section initialized")[index];
            let prior = world.light(p);
            let previous_seed = *seed;
            *seed = sky;
            if prior.sky() != sky {
                world.set_light(p, VoxelLight::new(sky, prior.block()));
                self.work_counters.light_writes += 1;
                let before = work.dirty_sections.len();
                mark_light_dirty(&mut work.dirty_sections, p);
                self.work_counters.dirty_section_insertions +=
                    work.dirty_sections.len().saturating_sub(before) as u64;
            }
            // Fully sunlit cells and opaque dark solids do not need a relaxation visit. Only
            // shaded transparent cells, existing block-light state and real emitters seed it.
            let needs_propagation = definition.is_some_and(|block| {
                block.emission > 0
                    || prior.block() > 0
                    || (sky < 15 && block.sky_opacity < 15 && block.light_opacity < 15)
                    || (previous_seed != sky && sky < 15 && block.sky_opacity < 15)
            });
            if needs_propagation {
                work.seed_candidates.push(p);
            }
        }
    }
    fn source(&self, p: BlockPos) -> Option<u8> {
        let (_, local) = split_block(p);
        self.direct.get(&section(p)).map(|v| v[block_index(local)])
    }

    fn needs_propagation_at(&self, world: &World, r: &BlockRegistry, p: BlockPos) -> bool {
        let Some(source) = self.source(p) else {
            return false;
        };
        let definition = r.get(world.get(p));
        let sky_cost = definition.map_or(1, |block| block.sky_opacity.max(1));
        let block_cost = definition.map_or(1, |block| block.light_opacity.max(1));
        let mut sky = source;
        let mut block = definition.map_or(0, |definition| definition.emission);
        for neighbor in neighbors(p) {
            let light = world.light(neighbor);
            sky = sky.max(light.sky().saturating_sub(sky_cost));
            block = block.max(light.block().saturating_sub(block_cost));
        }
        VoxelLight::new(sky, block) != world.light(p)
    }
    fn propagate(
        &mut self,
        world: &mut World,
        r: &BlockRegistry,
        mut queue: VecDeque<BlockPos>,
        dirty: &mut HashSet<SectionPos>,
    ) {
        let mut queued: HashSet<_> = queue.iter().copied().collect();
        let _ = self.propagate_work(world, r, &mut queue, &mut queued, dirty, usize::MAX);
    }

    fn propagate_work(
        &mut self,
        world: &mut World,
        r: &BlockRegistry,
        queue: &mut VecDeque<BlockPos>,
        queued: &mut HashSet<BlockPos>,
        dirty: &mut HashSet<SectionPos>,
        budget: usize,
    ) -> bool {
        self.last_visited = 0;
        let mut processed = 0;
        while processed < budget {
            let Some(p) = queue.pop_front() else {
                return true;
            };
            processed += 1;
            self.work_counters.propagation_queue_pops += 1;
            queued.remove(&p);
            let Some(source) = self.source(p) else {
                continue;
            };
            self.last_visited += 1;
            let b = r.get(world.get(p));
            let sky_cost = b.map_or(1, |b| b.sky_opacity.max(1));
            let block_cost = b.map_or(1, |b| b.light_opacity.max(1));
            let mut sky = source;
            let mut block = b.map_or(0, |b| b.emission);
            for n in neighbors(p) {
                let light = world.light(n);
                sky = sky.max(light.sky().saturating_sub(sky_cost));
                block = block.max(light.block().saturating_sub(block_cost));
            }
            let new = VoxelLight::new(sky, block);
            if new != world.light(p) {
                world.set_light(p, new);
                self.work_counters.light_writes += 1;
                let before = dirty.len();
                mark_light_dirty(dirty, p);
                self.work_counters.dirty_section_insertions +=
                    dirty.len().saturating_sub(before) as u64;
                for n in neighbors(p) {
                    if self.source(n).is_some() && queued.insert(n) {
                        queue.push_back(n);
                        self.work_counters.propagation_queue_pushes += 1;
                    }
                }
            }
        }
        queue.is_empty()
    }
    pub fn update(
        &mut self,
        world: &mut World,
        r: &BlockRegistry,
        p: BlockPos,
        dirty: &mut HashSet<SectionPos>,
    ) {
        let started = Instant::now();
        let (c, y) = section(p);
        let b = self.columns.entry(c).or_insert((y - 1, y + 1));
        b.0 = b.0.min(y - 1);
        b.1 = b.1.max(y + 1);
        let mut added = false;
        for sy in b.0..=b.1 {
            if let std::collections::hash_map::Entry::Vacant(e) = self.direct.entry((c, sy)) {
                e.insert(vec![0; 4096]);
                added = true;
            }
        }
        let mut queue = VecDeque::new();
        if added {
            for z in 0..16 {
                for x in 0..16 {
                    self.seed_column(world, r, c.x * 16 + x, c.z * 16 + z, &mut queue);
                }
            }
        } else {
            self.seed_column(world, r, p.x, p.z, &mut queue);
        }
        queue.push_back(p);
        queue.extend(neighbors(p));
        self.propagate(world, r, queue, dirty);
        self.last_update_ms = started.elapsed().as_secs_f64() * 1000.;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_column_lighting_is_reprioritized_without_cancelling_work() {
        let mut lighting = Lighting {
            queued_integrations: VecDeque::from([
                QueuedColumnIntegration {
                    column: ChunkPos { x: -2, z: 0 },
                    section_ys: vec![0],
                    queued_at: Instant::now(),
                    direct_ready: false,
                },
                QueuedColumnIntegration {
                    column: ChunkPos { x: 0, z: 2 },
                    section_ys: vec![1],
                    queued_at: Instant::now(),
                    direct_ready: false,
                },
                QueuedColumnIntegration {
                    column: ChunkPos { x: 1, z: 0 },
                    section_ys: vec![2],
                    queued_at: Instant::now(),
                    direct_ready: false,
                },
            ]),
            ..Default::default()
        };
        lighting.reprioritize_pending_columns(|position| {
            let distance = i64::from(position.x).pow(2) + i64::from(position.z).pow(2);
            (distance, distance, position.x, position.z)
        });
        assert_eq!(
            lighting
                .queued_integrations
                .iter()
                .map(|job| job.column)
                .collect::<Vec<_>>(),
            [
                ChunkPos { x: 1, z: 0 },
                ChunkPos { x: -2, z: 0 },
                ChunkPos { x: 0, z: 2 },
            ]
        );
        assert_eq!(lighting.queued_integrations[0].section_ys, [2]);
    }

    #[test]
    fn lighting_priority_aging_prevents_starvation_as_interest_moves() {
        let now = Instant::now();
        let mut lighting = Lighting {
            queued_integrations: VecDeque::from([
                QueuedColumnIntegration {
                    column: ChunkPos { x: 2, z: 0 },
                    section_ys: vec![0],
                    queued_at: now - std::time::Duration::from_secs(4),
                    direct_ready: false,
                },
                QueuedColumnIntegration {
                    column: ChunkPos { x: 0, z: 0 },
                    section_ys: vec![0],
                    queued_at: now,
                    direct_ready: false,
                },
            ]),
            ..Default::default()
        };
        lighting.reprioritize_pending_columns(|position| {
            let distance = i64::from(position.x).pow(2) + i64::from(position.z).pow(2);
            (distance * 1_048_576, distance, position.x, position.z)
        });
        assert_eq!(
            lighting.queued_integrations[0].column,
            ChunkPos { x: 2, z: 0 }
        );
    }

    #[test]
    fn initial_boundary_scan_only_visits_resident_neighbor_faces() {
        let column = ChunkPos { x: 0, z: 0 };
        let mut isolated = Lighting::default();
        assert!(!isolated.start_column_integration(column, vec![0, 1]));
        let isolated_work = isolated.integration.as_ref().unwrap();
        assert!(isolated_work.boundary_faces.is_empty());
        assert_eq!(isolated_work.boundary_total, 0);

        let mut adjacent = Lighting::default();
        adjacent.columns.insert(ChunkPos { x: -1, z: 0 }, (-1, 2));
        assert!(!adjacent.start_column_integration(column, vec![0, 1]));
        let adjacent_work = adjacent.integration.as_ref().unwrap();
        assert_eq!(adjacent_work.boundary_faces, [0]);
        assert_eq!(
            adjacent_work.boundary_total * 4,
            adjacent_work.seed_cost * 64
        );
    }

    use rustcraft_engine_core::BlockId;
    use rustcraft_mod_api::BlockDefinition;
    fn registry() -> BlockRegistry {
        let mut r = BlockRegistry::default();
        r.register(BlockDefinition::cube(1, "test:stone", "test:stone"))
            .unwrap();
        r.register(BlockDefinition {
            emission: 15,
            ..BlockDefinition::cube(2, "test:lamp", "test:lamp")
        })
        .unwrap();
        r
    }
    #[test]
    fn sky_emission_removal_and_signed_boundaries() {
        let r = registry();
        let mut w = World::new(BlockId(0));
        w.fill_box(
            BlockPos {
                x: -2,
                y: -2,
                z: -2,
            },
            BlockPos { x: 17, y: -2, z: 2 },
            BlockId(1),
        );
        let mut l = Lighting::initialize(&mut w, &r);
        assert_eq!(w.light(BlockPos { x: 0, y: 0, z: 0 }).sky(), 15);
        let p = BlockPos { x: 15, y: -1, z: 0 };
        let mut dirty = HashSet::new();
        w.set(p, BlockId(2));
        l.update(&mut w, &r, p, &mut dirty);
        assert_eq!(w.light(BlockPos { x: 16, ..p }).block(), 14);
        assert_eq!(w.light(BlockPos { y: 0, ..p }).block(), 14);
        assert!(dirty.contains(&(ChunkPos { x: 1, z: 0 }, -1)));
        w.set(p, BlockId(0));
        l.update(&mut w, &r, p, &mut dirty);
        assert_eq!(w.light(BlockPos { x: 16, ..p }).block(), 0);
        w.fill_box(
            BlockPos { x: -2, y: 2, z: -2 },
            BlockPos { x: 2, y: 2, z: 2 },
            BlockId(1),
        );
        for z in -2..=2 {
            for x in -2..=2 {
                l.update(&mut w, &r, BlockPos { x, y: 2, z }, &mut dirty);
            }
        }
        assert!(w.light(BlockPos { x: 0, y: 1, z: 0 }).sky() < 15);
        let p = BlockPos { x: 0, y: 2, z: 0 };
        w.set(p, BlockId(0));
        l.update(&mut w, &r, p, &mut dirty);
        assert_eq!(w.light(BlockPos { x: 0, y: 1, z: 0 }).sky(), 15);
    }

    #[test]
    fn integrating_new_column_matches_fresh_lighting_across_boundary() {
        let r = registry();
        let left = ChunkPos { x: -1, z: 0 };
        let right = ChunkPos { x: 0, z: 0 };
        let mut streamed = World::new(BlockId(0));
        streamed
            .publish_column(
                left,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        streamed.set(BlockPos { x: -1, y: 8, z: 8 }, BlockId(2));
        let mut lighting = Lighting::initialize(&mut streamed, &r);
        streamed
            .publish_column(
                right,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        lighting.integrate_column(&mut streamed, &r, right, vec![0], &mut HashSet::new());

        let mut fresh = World::new(BlockId(0));
        fresh
            .publish_column(
                left,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        fresh
            .publish_column(
                right,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        fresh.set(BlockPos { x: -1, y: 8, z: 8 }, BlockId(2));
        let _fresh_lighting = Lighting::initialize(&mut fresh, &r);
        for x in [0, 1, 2, 4, 8] {
            let p = BlockPos { x, y: 8, z: 8 };
            assert_eq!(streamed.light(p), fresh.light(p), "boundary light at {p:?}");
        }
    }

    #[test]
    fn incremental_column_lighting_converges_to_synchronous_result_in_bounded_steps() {
        let r = registry();
        let left = ChunkPos { x: -1, z: 0 };
        let right = ChunkPos { x: 0, z: 0 };
        let mut incremental = World::new(BlockId(0));
        incremental
            .publish_column(
                left,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        incremental.set(BlockPos { x: -1, y: 8, z: 8 }, BlockId(2));
        let mut incremental_light = Lighting::initialize(&mut incremental, &r);
        incremental
            .publish_column(
                right,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        assert!(!incremental_light.begin_column_integration(right, vec![0]));
        let mut dirty = HashSet::new();
        assert!(!incremental_light.advance_column_integration(
            &mut incremental,
            &r,
            &mut dirty,
            512,
        ));
        assert!(
            dirty.is_empty(),
            "partial lighting must coalesce render dirtiness"
        );
        let mut steps = 1;
        while !incremental_light.advance_column_integration(&mut incremental, &r, &mut dirty, 512) {
            steps += 1;
            assert!(steps < 200, "bounded integration failed to converge");
        }
        assert!(
            steps > 1,
            "integration unexpectedly completed in one unbounded step"
        );
        assert!(
            !dirty.is_empty(),
            "converged lighting must invalidate render sections"
        );

        let mut synchronous = World::new(BlockId(0));
        synchronous
            .publish_column(
                left,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        synchronous
            .publish_column(
                right,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        synchronous.set(BlockPos { x: -1, y: 8, z: 8 }, BlockId(2));
        let _ = Lighting::initialize(&mut synchronous, &r);
        for y in -16..32 {
            for z in -1..17 {
                for x in -17..17 {
                    let p = BlockPos { x, y, z };
                    assert_eq!(incremental.light(p), synchronous.light(p), "light at {p:?}");
                }
            }
        }
    }

    #[test]
    fn queued_column_lighting_is_bounded_and_converges_in_order() {
        let r = registry();
        let columns = [
            ChunkPos { x: -1, z: 0 },
            ChunkPos { x: 0, z: 0 },
            ChunkPos { x: 1, z: 0 },
        ];
        let mut streamed = World::new(BlockId(0));
        streamed
            .publish_column(
                columns[0],
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        streamed.set(BlockPos { x: -1, y: 8, z: 8 }, BlockId(2));
        let mut lighting = Lighting::initialize(&mut streamed, &r);
        for &column in &columns[1..] {
            streamed
                .publish_column(
                    column,
                    vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
                )
                .unwrap();
            assert!(lighting.queue_column_integration(column, vec![0]));
        }
        assert_eq!(lighting.integration_columns(), columns[1..]);

        let mut dirty = HashSet::new();
        let mut completed = 0;
        let mut steps = 0;
        while lighting.has_integration_work() {
            if lighting.advance_column_integration(&mut streamed, &r, &mut dirty, 8_192) {
                completed += 1;
            }
            steps += 1;
            assert!(steps < 100, "queued lighting failed to converge");
        }
        assert_eq!(completed, 2);
        assert!(!dirty.is_empty());

        let mut fresh = World::new(BlockId(0));
        for &column in &columns {
            fresh
                .publish_column(
                    column,
                    vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
                )
                .unwrap();
        }
        fresh.set(BlockPos { x: -1, y: 8, z: 8 }, BlockId(2));
        let _ = Lighting::initialize(&mut fresh, &r);
        for x in -1..=32 {
            for y in 0..16 {
                let p = BlockPos { x, y, z: 8 };
                assert_eq!(streamed.light(p), fresh.light(p), "light at {p:?}");
            }
        }

        let mut full = World::new(BlockId(0));
        for x in 0..MAX_COLUMN_LIGHTING_JOBS as i32 + 1 {
            full.publish_column(
                ChunkPos { x, z: 0 },
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        }
        let mut full_lighting = Lighting::initialize(&mut full, &r);
        for x in 0..MAX_COLUMN_LIGHTING_JOBS as i32 {
            assert!(full_lighting.queue_column_integration(ChunkPos { x, z: 0 }, vec![0]));
        }
        assert_eq!(full_lighting.integration_capacity_remaining(), 0);
        assert!(!full_lighting.can_queue_column_integration(ChunkPos {
            x: MAX_COLUMN_LIGHTING_JOBS as i32,
            z: 0,
        }));
    }

    #[test]
    fn obsolete_queued_lighting_can_be_cancelled_without_interrupting_active_work() {
        let active = ChunkPos { x: 0, z: 0 };
        let obsolete = ChunkPos { x: 8, z: 0 };
        let direct = || vec![(-1, vec![0; 4096]), (0, vec![0; 4096]), (1, vec![0; 4096])];
        let mut lighting = Lighting::default();
        lighting.adopt_initial_column(active, vec![0], direct());
        lighting.adopt_initial_column(obsolete, vec![0], direct());

        assert_eq!(lighting.integrating_column(), Some(active));
        assert!(lighting.integration_columns().contains(&obsolete));
        assert!(lighting.cancel_queued_for_eviction(obsolete));
        assert_eq!(lighting.integration_columns(), vec![active]);
        assert!(!lighting.cancel_queued_for_eviction(active));
    }

    #[test]
    fn empty_column_uses_deferred_direct_scan_without_light_queue_amplification() {
        let registry = BlockRegistry::default();
        let left = ChunkPos { x: 0, z: 0 };
        let right = ChunkPos { x: 1, z: 0 };
        let mut world = World::new(BlockId(0));
        world
            .publish_column(
                left,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        let mut lighting = Lighting::initialize(&mut world, &registry);
        let before = lighting.work_counters();
        world
            .publish_column(
                right,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        assert!(lighting.queue_column_integration(right, vec![0]));
        assert_eq!(
            lighting.work_counters().direct_voxels_scanned,
            before.direct_voxels_scanned,
            "publication must not scan terrain synchronously"
        );
        let mut dirty = HashSet::new();
        assert!(lighting.advance_column_integration(&mut world, &registry, &mut dirty, usize::MAX));
        let after = lighting.work_counters();
        assert_eq!(after.columns_started - before.columns_started, 1);
        assert_eq!(after.columns_completed - before.columns_completed, 1);
        assert_eq!(
            after.propagation_queue_pushes - before.propagation_queue_pushes,
            0,
            "fully sunlit empty terrain needs no propagation queue"
        );
    }

    #[test]
    fn evicting_emissive_column_recomputes_neighbor_boundary_light() {
        let r = registry();
        let left = ChunkPos { x: 0, z: 0 };
        let right = ChunkPos { x: 1, z: 0 };
        let mut world = World::new(BlockId(0));
        world
            .publish_column(
                left,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        world
            .publish_column(
                right,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        world.set(BlockPos { x: 15, y: 8, z: 8 }, BlockId(2));
        let mut lighting = Lighting::initialize(&mut world, &r);
        assert!(world.light(BlockPos { x: 16, y: 8, z: 8 }).block() > 0);
        world.remove_column(left);
        lighting.remove_column_from_world(&mut world, &r, left, &mut HashSet::new());

        let mut fresh = World::new(BlockId(0));
        fresh
            .publish_column(
                right,
                vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
            )
            .unwrap();
        let _fresh_lighting = Lighting::initialize(&mut fresh, &r);
        assert_eq!(
            world.light(BlockPos { x: 16, y: 8, z: 8 }).block(),
            fresh.light(BlockPos { x: 16, y: 8, z: 8 }).block()
        );
    }

    #[test]
    fn incremental_column_removal_clears_neighbor_light_without_one_large_step() {
        let r = registry();
        let left = ChunkPos { x: 0, z: 0 };
        let right = ChunkPos { x: 1, z: 0 };
        let mut world = World::new(BlockId(0));
        for column in [left, right] {
            world
                .publish_column(
                    column,
                    vec![(0, rustcraft_engine_core::Chunk::new(BlockId(0)))],
                )
                .unwrap();
        }
        world.set(BlockPos { x: 15, y: 8, z: 8 }, BlockId(2));
        let mut lighting = Lighting::initialize(&mut world, &r);
        assert!(world.light(BlockPos { x: 16, y: 8, z: 8 }).block() > 0);
        world.remove_column(left);
        assert!(!lighting.begin_column_removal(left));
        let mut dirty = HashSet::new();
        let mut steps = 0;
        while !lighting.advance_column_integration(&mut world, &r, &mut dirty, 512) {
            steps += 1;
            assert!(steps < 200, "bounded removal failed to converge");
        }
        assert!(steps > 1);
        assert_eq!(world.light(BlockPos { x: 16, y: 8, z: 8 }).block(), 0);
    }
}

#[cfg(test)]
mod enclosure_tests {
    use super::*;
    use rustcraft_engine_core::BlockId;
    use rustcraft_mod_api::BlockDefinition;
    #[test]
    fn enclosed_sky_two_sources_and_removal_match_fresh_initialization() {
        let mut r = BlockRegistry::default();
        r.register(BlockDefinition::cube(1, "test:wall", "test:wall"))
            .unwrap();
        r.register(BlockDefinition {
            emission: 15,
            ..BlockDefinition::cube(2, "test:lamp", "test:lamp")
        })
        .unwrap();
        let mut w = World::new(BlockId(0));
        // Room crosses x=16 and y=16, so both channels must cross section seams.
        w.fill_box(
            BlockPos { x: 12, y: 12, z: 0 },
            BlockPos { x: 20, y: 20, z: 8 },
            BlockId(1),
        );
        w.fill_box(
            BlockPos { x: 13, y: 13, z: 1 },
            BlockPos { x: 19, y: 19, z: 7 },
            BlockId(0),
        );
        let mut light = Lighting::initialize(&mut w, &r);
        let center = BlockPos { x: 16, y: 16, z: 4 };
        assert_eq!(w.light(center), VoxelLight::new(0, 0));
        let a = BlockPos { x: 15, y: 15, z: 4 };
        let b = BlockPos { x: 18, y: 17, z: 4 };
        let mut dirty = HashSet::new();
        for p in [a, b] {
            w.set(p, BlockId(2));
            light.update(&mut w, &r, p, &mut dirty);
        }
        assert_eq!(w.light(center).block(), 13);
        w.set(a, BlockId(0));
        light.update(&mut w, &r, a, &mut dirty);
        assert_eq!(w.light(center).block(), 12);
        w.set(b, BlockId(0));
        light.update(&mut w, &r, b, &mut dirty);
        assert_eq!(w.light(center).block(), 0);
        let roof = BlockPos { y: 20, ..center };
        w.set(roof, BlockId(0));
        light.update(&mut w, &r, roof, &mut dirty);
        assert_eq!(w.light(center).sky(), 15);
        w.set(roof, BlockId(1));
        light.update(&mut w, &r, roof, &mut dirty);
        assert_eq!(w.light(center).sky(), 0);
        let mut fresh = World::new(BlockId(0));
        for y in 12..=20 {
            for z in 0..=8 {
                for x in 12..=20 {
                    let p = BlockPos { x, y, z };
                    fresh.set(p, w.get(p));
                }
            }
        }
        Lighting::initialize(&mut fresh, &r);
        for y in 12..=20 {
            for z in 0..=8 {
                for x in 12..=20 {
                    let p = BlockPos { x, y, z };
                    assert_eq!(w.light(p), fresh.light(p), "{p:?}");
                }
            }
        }
    }
}

#[cfg(test)]
mod dirty_section_tests {
    use super::*;

    #[test]
    fn boundary_voxel_invalidates_each_neighbor_section_input() {
        for (position, expected) in [
            (BlockPos { x: 0, y: 8, z: 8 }, (ChunkPos { x: -1, z: 0 }, 0)),
            (BlockPos { x: 15, y: 8, z: 8 }, (ChunkPos { x: 1, z: 0 }, 0)),
            (BlockPos { x: 8, y: 0, z: 8 }, (ChunkPos { x: 0, z: 0 }, -1)),
            (BlockPos { x: 8, y: 15, z: 8 }, (ChunkPos { x: 0, z: 0 }, 1)),
            (BlockPos { x: 8, y: 8, z: 0 }, (ChunkPos { x: 0, z: -1 }, 0)),
            (BlockPos { x: 8, y: 8, z: 15 }, (ChunkPos { x: 0, z: 1 }, 0)),
        ] {
            let mut dirty = HashSet::new();
            dirty_neighbors(&mut dirty, position);
            assert!(dirty.contains(&section(position)));
            assert!(dirty.contains(&expected), "{position:?}");
        }
    }
}

#[cfg(test)]
mod initial_worker_tests {
    use super::*;
    use rustcraft_engine_core::{BlockState, ChunkBuilder};

    fn registry() -> BlockRegistry {
        let mut registry = BlockRegistry::default();
        let mut lamp = rustcraft_mod_api::BlockDefinition::cube(2, "test_lamp", "test:lamp");
        lamp.emission = 14;
        lamp.sky_opacity = 15;
        registry.register(lamp).unwrap();
        registry
    }

    fn sections(position: ChunkPos) -> Vec<(i32, Chunk)> {
        let mut builder = ChunkBuilder::new(BlockState::new(BlockId(0)));
        if position.x == 0 {
            // Put an emitter against the shared east/west border to exercise boundary transfer.
            builder.set((15, 8, 8), BlockState::new(BlockId(2)));
        }
        vec![(0, builder.finish())]
    }

    fn worker_result(position: ChunkPos, registry: &BlockRegistry) -> InitialLightingResult {
        build_initial_lighting(InitialLightingJob {
            position,
            token: 1,
            priority: (0, 0, 0, 0),
            queued_at: Instant::now(),
            default_block: BlockId(0),
            sections: sections(position),
            registry: Arc::new(registry.clone()),
            persist_new: false,
        })
        .unwrap()
    }

    fn build_pair(order: [ChunkPos; 2], registry: &BlockRegistry) -> World {
        let mut world = World::new(BlockId(0));
        let mut lighting = Lighting::default();
        for position in order {
            let result = worker_result(position, registry);
            let section_ys = result.sections.iter().map(|(y, _)| *y).collect::<Vec<_>>();
            world.publish_column(position, result.sections).unwrap();
            for (section_y, lights) in result.light_sections {
                world
                    .replace_section_lights(position, section_y, lights)
                    .unwrap();
            }
            lighting.adopt_initial_column(position, section_ys, result.direct_sections);
        }
        let mut dirty = HashSet::new();
        while lighting.has_integration_work() {
            assert!(
                lighting.advance_column_integration(&mut world, registry, &mut dirty, 100_000),
                "one initial-light boundary column should finish per step"
            );
        }
        world
    }

    #[test]
    fn initial_worker_matches_synchronous_single_column_lighting() {
        let registry = registry();
        let position = ChunkPos { x: -1, z: 0 };
        let result = worker_result(position, &registry);
        let mut reference = World::new(BlockId(0));
        reference
            .publish_column(position, sections(position))
            .unwrap();
        Lighting::initialize(&mut reference, &registry);
        for (section_y, lights) in result.light_sections {
            assert_eq!(
                reference.section_lights(position, section_y).unwrap(),
                lights.as_slice(),
                "section {section_y}"
            );
        }
    }

    #[test]
    fn bulk_lit_column_is_presentation_dirty_before_boundary_work_finishes() {
        let registry = registry();
        let position = ChunkPos { x: 0, z: 0 };
        let mut simulation = crate::Simulation::new(
            World::new(BlockId(0)),
            registry.clone(),
            rustcraft_engine_core::Vec3::new(0.5, 2.0, 0.5),
        );
        let result = worker_result(position, &registry);
        simulation
            .publish_initial_lit_column(result, false)
            .expect("worker-lit column should publish");

        assert!(simulation.lighting.has_integration_work());
        let immediately_dirty = simulation.take_dirty_sections();
        assert!(
            immediately_dirty.contains(&(position, 0)),
            "complete local light should be meshable while boundary reconciliation is pending"
        );
    }

    #[test]
    fn initial_lighting_neighbor_order_converges_identically() {
        let registry = registry();
        let left = ChunkPos { x: 0, z: 0 };
        let right = ChunkPos { x: 1, z: 0 };
        let left_then_right = build_pair([left, right], &registry);
        let right_then_left = build_pair([right, left], &registry);
        let mut simultaneous_reference = World::new(BlockId(0));
        simultaneous_reference
            .publish_column(left, sections(left))
            .unwrap();
        simultaneous_reference
            .publish_column(right, sections(right))
            .unwrap();
        Lighting::initialize(&mut simultaneous_reference, &registry);
        for position in [left, right] {
            for section_y in 0..=0 {
                let left_right = left_then_right.section_lights(position, section_y);
                let reverse = right_then_left.section_lights(position, section_y);
                let first_difference = left_right
                    .zip(reverse)
                    .and_then(|(a, b)| a.iter().zip(b).position(|(a, b)| a != b));
                assert_eq!(
                    first_difference,
                    None,
                    "order mismatch at {position:?}, section {section_y}; left={:?}, right={:?}",
                    left_right.map(|values| values.get(first_difference.unwrap_or(0))),
                    reverse.map(|values| values.get(first_difference.unwrap_or(0)))
                );
                let expected = simultaneous_reference.section_lights(position, section_y);
                let first_difference = left_right
                    .zip(expected)
                    .and_then(|(a, b)| a.iter().zip(b).position(|(a, b)| a != b));
                assert_eq!(
                    first_difference,
                    None,
                    "reference mismatch at {position:?}, section {section_y}; actual={:?}, expected={:?}",
                    left_right.map(|values| values.get(first_difference.unwrap_or(0))),
                    expected.map(|values| values.get(first_difference.unwrap_or(0)))
                );
            }
        }
    }

    #[test]
    fn initial_lighting_scheduler_is_bounded_and_returns_completed_jobs() {
        let registry = Arc::new(registry());
        let mut scheduler = InitialLightingScheduler::new(1, 1);
        let position = ChunkPos { x: -3, z: 4 };
        scheduler
            .submit(InitialLightingRequest {
                position,
                token: 8,
                priority: (0, 0, position.x, position.z),
                default_block: BlockId(0),
                sections: sections(position),
                registry: registry.clone(),
                persist_new: false,
            })
            .unwrap();
        let other = ChunkPos { x: 6, z: -2 };
        assert!(
            scheduler
                .submit(InitialLightingRequest {
                    position: other,
                    token: 9,
                    priority: (0, 0, other.x, other.z),
                    default_block: BlockId(0),
                    sections: sections(other),
                    registry,
                    persist_new: false,
                })
                .is_err()
        );
        let deadline = Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let ready = scheduler.take_ready(1);
            if !ready.is_empty() {
                assert_eq!(ready[0].position, position);
                assert_eq!(ready[0].token, 8);
                break;
            }
            assert!(Instant::now() < deadline, "initial-light worker timed out");
            thread::yield_now();
        }
        assert_eq!(scheduler.outstanding(), 0);
    }

    #[test]
    fn superseded_initial_light_result_never_reaches_publication_queue() {
        let registry = Arc::new(registry());
        let mut scheduler = InitialLightingScheduler::new(1, 2);
        let position = ChunkPos { x: 7, z: -4 };
        for token in [10, 11] {
            scheduler
                .submit(InitialLightingRequest {
                    position,
                    token,
                    priority: (0, 0, position.x, position.z),
                    default_block: BlockId(0),
                    sections: sections(position),
                    registry: registry.clone(),
                    persist_new: false,
                })
                .unwrap();
        }
        let deadline = Instant::now() + std::time::Duration::from_secs(3);
        let mut accepted = Vec::new();
        while accepted.is_empty() && Instant::now() < deadline {
            accepted = scheduler.take_ready(2);
            thread::yield_now();
        }
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].token, 11);
        assert_eq!(scheduler.stale, 1);
        assert_eq!(scheduler.outstanding(), 0);
    }
}
