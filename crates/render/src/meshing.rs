//! Bounded CPU section meshing with generation-based stale-result rejection.

use crate::{BlockTextureResolver, PageMesh, RenderChunk, build_section_mesh_pages};
use rustcraft_engine_core::{SectionPos, Vec3};
use std::{
    collections::{HashMap, VecDeque},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Instant,
};

pub const MAX_MESH_WORKERS: usize = 32;
// Current section emitter visits 16^3 blocks, at most six quads per block; four vertices/six
// indices per quad. Vec growth is bounded by twice the occupied payload for nonempty pages.
pub const MAX_RESULT_LOGICAL_BYTES: usize =
    16 * 16 * 16 * 6 * (4 * std::mem::size_of::<crate::Vertex>() + 6 * 4);
pub const MAX_RESULT_CAPACITY_BYTES: usize =
    2 * MAX_RESULT_LOGICAL_BYTES + 2 * 16 * 16 * 16 * 6 * std::mem::size_of::<PageMesh>();

#[must_use]
pub const fn bounded_mesh_worker_count(requested: usize) -> usize {
    if requested == 0 {
        1
    } else if requested > MAX_MESH_WORKERS {
        MAX_MESH_WORKERS
    } else {
        requested
    }
}

#[derive(Debug, Default)]
struct LifetimeAccounting {
    snapshots: AtomicUsize,
    snapshot_bytes: AtomicUsize,
    completed: AtomicUsize,
    completed_bytes: AtomicUsize,
    completed_capacity: AtomicUsize,
}
#[derive(Debug)]
struct MeshJob {
    accounting: Arc<LifetimeAccounting>,
    section: SectionPos,
    generation: u64,
    snapshot: RenderChunk,
    enqueued_at: Instant,
}

impl Drop for MeshJob {
    fn drop(&mut self) {
        self.accounting.snapshots.fetch_sub(1, Ordering::Relaxed);
        self.accounting
            .snapshot_bytes
            .fetch_sub(self.snapshot.snapshot_bytes(), Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub struct CompletedMesh {
    pub section: SectionPos,
    pub generation: u64,
    pub pages: Vec<PageMesh>,
    pub queue_wait_ms: f64,
    pub mesh_ms: f64,
    pub completed_at: Instant,
}

impl CompletedMesh {
    pub fn capacity_bytes(&self) -> usize {
        self.pages.capacity() * std::mem::size_of::<PageMesh>()
            + self
                .pages
                .iter()
                .map(|p| {
                    p.mesh.vertices.capacity() * std::mem::size_of::<crate::Vertex>()
                        + p.mesh.indices.capacity() * std::mem::size_of::<u32>()
                })
                .sum::<usize>()
    }
    #[must_use]
    pub fn logical_bytes(&self) -> usize {
        self.pages
            .iter()
            .map(|page| {
                page.mesh.vertices.len() * std::mem::size_of::<crate::Vertex>()
                    + page.mesh.indices.len() * std::mem::size_of::<u32>()
            })
            .sum()
    }
}

enum WorkerResult {
    Completed(CompletedMesh),
    Failed {
        section: SectionPos,
        generation: u64,
    },
}

struct WorkerMessage {
    result: Option<WorkerResult>,
    accounting: Arc<LifetimeAccounting>,
    bytes: usize,
    capacity: usize,
}
impl Drop for WorkerMessage {
    fn drop(&mut self) {
        self.accounting.completed.fetch_sub(1, Ordering::Relaxed);
        self.accounting
            .completed_bytes
            .fetch_sub(self.bytes, Ordering::Relaxed);
        self.accounting
            .completed_capacity
            .fetch_sub(self.capacity, Ordering::Relaxed);
    }
}
struct WorkerPool {
    sender: Option<mpsc::SyncSender<MeshJob>>,
    receiver: mpsc::Receiver<WorkerMessage>,
    threads: Vec<thread::JoinHandle<()>>,
    accounting: Arc<LifetimeAccounting>,
}

impl WorkerPool {
    fn new<R>(worker_count: usize, queue_capacity: usize, resolver: R) -> Self
    where
        R: BlockTextureResolver + Send + Sync + 'static,
    {
        let (job_sender, job_receiver) = mpsc::sync_channel::<MeshJob>(queue_capacity.max(1));
        let (result_sender, result_receiver) = mpsc::channel();
        let jobs = Arc::new(Mutex::new(job_receiver));
        let resolver = Arc::new(resolver);
        let accounting = Arc::new(LifetimeAccounting::default());
        let mut threads = Vec::with_capacity(worker_count);
        for index in 0..bounded_mesh_worker_count(worker_count) {
            let jobs = Arc::clone(&jobs);
            let results = result_sender.clone();
            let resolver = Arc::clone(&resolver);
            let accounting = Arc::clone(&accounting);
            threads.push(
                thread::Builder::new()
                    .name(format!("rustcraft-mesh-{index}"))
                    .spawn(move || {
                        loop {
                            let job = {
                                let receiver = jobs.lock().expect("mesh job receiver poisoned");
                                receiver.recv()
                            };
                            let Ok(job) = job else { break };
                            let started = Instant::now();
                            let queue_wait_ms =
                                started.duration_since(job.enqueued_at).as_secs_f64() * 1000.0;
                            let section = job.section;
                            let generation = job.generation;
                            let outcome = catch_unwind(AssertUnwindSafe(|| {
                                build_section_mesh_pages(&job.snapshot, resolver.as_ref())
                            }));
                            let result = match outcome {
                                Ok(pages) => WorkerResult::Completed(CompletedMesh {
                                    section,
                                    generation,
                                    pages,
                                    queue_wait_ms,
                                    mesh_ms: started.elapsed().as_secs_f64() * 1000.0,
                                    completed_at: Instant::now(),
                                }),
                                Err(_) => WorkerResult::Failed {
                                    section,
                                    generation,
                                },
                            };
                            let bytes = match &result {
                                WorkerResult::Completed(r) => r.logical_bytes(),
                                _ => 0,
                            };
                            let capacity = match &result {
                                WorkerResult::Completed(r) => r.capacity_bytes(),
                                _ => 0,
                            };
                            accounting
                                .completed_capacity
                                .fetch_add(capacity, Ordering::Relaxed);
                            accounting.completed.fetch_add(1, Ordering::Relaxed);
                            accounting
                                .completed_bytes
                                .fetch_add(bytes, Ordering::Relaxed);
                            let message = WorkerMessage {
                                result: Some(result),
                                accounting: Arc::clone(&accounting),
                                bytes,
                                capacity,
                            };
                            if results.send(message).is_err() {
                                break;
                            }
                        }
                    })
                    .expect("spawn bounded mesh worker"),
            );
        }
        Self {
            sender: Some(job_sender),
            receiver: result_receiver,
            threads,
            accounting,
        }
    }

    fn try_submit(&self, job: MeshJob) -> Result<(), MeshJob> {
        match self
            .sender
            .as_ref()
            .expect("mesh pool active")
            .try_send(job)
        {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(job)) => Err(job),
            Err(mpsc::TrySendError::Disconnected(_)) => {
                panic!("mesh worker queue disconnected")
            }
        }
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.sender.take();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MeshingStats {
    pub mesh_jobs_submitted: u64,
    pub mesh_jobs_completed: u64,
    pub mesh_jobs_discarded_stale: u64,
    pub mesh_jobs_coalesced: u64,
    pub mesh_jobs_failed: u64,
    pub pending: usize,
    pub in_flight: usize,
    pub ready: usize,
    pub ready_cpu_bytes: usize,
    pub pending_snapshot_bytes: usize,
    pub in_flight_snapshot_bytes: usize,
    pub worker_count: usize,
    pub generation_entries: usize,
    pub generation_capacity: usize,
    pub pending_capacity: usize,
    pub retired_inflight: usize,
    pub cancelled_pending: u64,
    pub ready_capacity: usize,
    pub completed_unconsumed: usize,
    pub completed_cpu_bytes: usize,
    pub completed_capacity_bytes: usize,
    pub ready_capacity_bytes: usize,
    pub pending_order_entries: usize,
    pub pipeline_limit: usize,
    pub result_logical_byte_limit: usize,
    pub result_capacity_byte_limit: usize,
    pub live_snapshots: usize,
    pub live_snapshot_bytes: usize,
}

pub struct MeshScheduler {
    pool: WorkerPool,
    generations: HashMap<SectionPos, u64>,
    next_token: u64,
    pipeline_limit: usize,
    pending: HashMap<SectionPos, MeshJob>,
    pending_order: VecDeque<SectionPos>,
    in_flight: HashMap<SectionPos, (usize, u64)>,
    ready: Vec<CompletedMesh>,
    counters: MeshingStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnMeshStage {
    Pending,
    InFlight,
    UploadPending,
}

impl MeshScheduler {
    #[must_use]
    pub fn new<R>(worker_count: usize, queue_capacity: usize, resolver: R) -> Self
    where
        R: BlockTextureResolver + Send + Sync + 'static,
    {
        Self {
            pool: WorkerPool::new(worker_count, queue_capacity, resolver),
            generations: HashMap::new(),
            next_token: 0,
            pipeline_limit: bounded_mesh_worker_count(worker_count)
                .saturating_add(queue_capacity.max(1)),
            pending: HashMap::new(),
            pending_order: VecDeque::new(),
            in_flight: HashMap::new(),
            ready: Vec::new(),
            counters: MeshingStats::default(),
        }
    }

    pub fn mark_dirty(&mut self, snapshot: RenderChunk) -> u64 {
        let section = (snapshot.position, snapshot.section_y);
        let generation = self.allocate_token();
        self.generations.insert(section, generation);
        let accounting = Arc::clone(&self.pool.accounting);
        accounting.snapshots.fetch_add(1, Ordering::Relaxed);
        accounting
            .snapshot_bytes
            .fetch_add(snapshot.snapshot_bytes(), Ordering::Relaxed);
        let job = MeshJob {
            accounting,
            section,
            generation,
            snapshot,
            enqueued_at: Instant::now(),
        };
        if self.in_flight.contains_key(&section) {
            self.counters.mesh_jobs_coalesced += 1;
            self.pending.insert(section, job);
        } else if self.pending.insert(section, job).is_some() {
            self.counters.mesh_jobs_coalesced += 1;
        } else {
            self.pending_order.push_back(section);
        }
        self.pump();
        generation
    }

    fn allocate_token(&mut self) -> u64 {
        self.next_token = self
            .next_token
            .checked_add(1)
            .expect("mesh work token space exhausted");
        self.next_token
    }
    pub fn remove_section(&mut self, section: SectionPos) -> u64 {
        let generation = self.allocate_token();
        self.generations.remove(&section);
        self.pending_order.retain(|p| *p != section);
        if self.pending.remove(&section).is_some() {
            self.counters.cancelled_pending += 1;
        }
        self.ready.retain(|result| {
            let keep = result.section != section;
            if !keep {
                self.counters.mesh_jobs_discarded_stale += 1;
            }
            keep
        });
        generation
    }

    #[must_use]
    pub fn current_generation(&self, section: SectionPos) -> Option<u64> {
        self.generations.get(&section).copied()
    }

    pub fn poll(&mut self) {
        while let Ok(mut message) = self.pool.receiver.try_recv() {
            let result = message.result.take().expect("mesh message result");
            drop(message);
            let (section, generation) = match &result {
                WorkerResult::Completed(result) => (result.section, result.generation),
                WorkerResult::Failed {
                    section,
                    generation,
                } => (*section, *generation),
            };
            self.in_flight.remove(&section);
            match result {
                WorkerResult::Completed(result) => {
                    self.counters.mesh_jobs_completed += 1;
                    if self.generations.get(&section).copied() == Some(generation) {
                        self.ready.push(result);
                    } else {
                        self.counters.mesh_jobs_discarded_stale += 1;
                    }
                }
                WorkerResult::Failed { .. } => {
                    self.counters.mesh_jobs_failed += 1;
                    eprintln!(
                        "mesh worker failed section=({},{},{}) generation={generation}",
                        section.0.x, section.1, section.0.z
                    );
                }
            }
            if self.pending.contains_key(&section) {
                self.pending_order.push_back(section);
            }
        }
        self.pump();
    }

    fn pump(&mut self) {
        // in_flight includes queued, running and completed-unconsumed jobs. Moving a result
        // into ready preserves this count. Nonblocking result sends therefore cannot exceed the
        // window, and shutdown never waits on a producer blocked behind the consumer.
        while self.in_flight.len() + self.ready.len() < self.pipeline_limit {
            let Some(section) = self.pending_order.pop_front() else {
                break;
            };
            if self.in_flight.contains_key(&section) {
                continue;
            }
            let Some(job) = self.pending.remove(&section) else {
                continue;
            };
            let snapshot_bytes = job.snapshot.snapshot_bytes();
            let token = job.generation;
            match self.pool.try_submit(job) {
                Ok(()) => {
                    self.in_flight.insert(section, (snapshot_bytes, token));
                    self.counters.mesh_jobs_submitted += 1;
                }
                Err(job) => {
                    self.pending.insert(section, job);
                    self.pending_order.push_front(section);
                    break;
                }
            }
        }
    }

    pub fn take_ready(
        &mut self,
        camera: Vec3,
        view_forward: Vec3,
        max_sections: usize,
        max_bytes: usize,
    ) -> Vec<CompletedMesh> {
        self.ready.retain(|result| {
            let current = self.generations.get(&result.section).copied();
            if current == Some(result.generation) {
                true
            } else {
                self.counters.mesh_jobs_discarded_stale += 1;
                false
            }
        });
        let forward_length = (view_forward.x * view_forward.x
            + view_forward.y * view_forward.y
            + view_forward.z * view_forward.z)
            .sqrt();
        let forward = if forward_length > 1.0e-5 {
            Vec3::new(
                view_forward.x / forward_length,
                view_forward.y / forward_length,
                view_forward.z / forward_length,
            )
        } else {
            Vec3::ZERO
        };
        self.ready.sort_by(|a, b| {
            let key = |result: &CompletedMesh| {
                let x = result.section.0.x as f32 * 16.0 + 8.0 - camera.x;
                let y = result.section.1 as f32 * 16.0 + 8.0 - camera.y;
                let z = result.section.0.z as f32 * 16.0 + 8.0 - camera.z;
                let distance_squared = x * x + y * y + z * z;
                // Keep the immediate safety core ahead of ordinary visible and
                // prefetch work, then prefer ready sections in the view direction.
                let urgency = if distance_squared <= 2_000.0 {
                    0
                } else if distance_squared <= 16_000.0 {
                    1
                } else {
                    2
                };
                let distance = distance_squared.sqrt();
                let alignment = if distance > 1.0e-5 {
                    (x * forward.x + y * forward.y + z * forward.z) / distance
                } else {
                    0.0
                };
                // A bounded directional discount breaks same-ring ties without
                // allowing a distant speculative result to outrank nearby work.
                let directional_distance = distance - alignment * distance.min(64.0) * 0.35;
                (urgency, directional_distance, distance_squared)
            };
            let a_key = key(a);
            let b_key = key(b);
            a_key
                .0
                .cmp(&b_key.0)
                .then_with(|| a_key.1.total_cmp(&b_key.1))
                .then_with(|| a_key.2.total_cmp(&b_key.2))
                .then_with(|| a.section.0.x.cmp(&b.section.0.x))
                .then_with(|| a.section.0.z.cmp(&b.section.0.z))
                .then_with(|| a.section.1.cmp(&b.section.1))
        });
        let mut bytes = 0_usize;
        let mut split = 0;
        for (count, result) in self.ready.iter().enumerate() {
            let next = result.logical_bytes();
            if count >= max_sections || (count > 0 && bytes.saturating_add(next) > max_bytes) {
                break;
            }
            bytes += next;
            split = count + 1;
        }
        self.ready.drain(..split).collect()
    }

    #[must_use]
    pub fn stats(&self) -> MeshingStats {
        MeshingStats {
            retired_inflight: self
                .in_flight
                .iter()
                .filter(|(p, (_, token))| self.generations.get(p) != Some(token))
                .count(),
            completed_capacity_bytes: self
                .pool
                .accounting
                .completed_capacity
                .load(Ordering::Relaxed),
            ready_capacity_bytes: self.ready.iter().map(CompletedMesh::capacity_bytes).sum(),
            pending_order_entries: self.pending_order.len(),
            pipeline_limit: self.pipeline_limit,
            result_logical_byte_limit: self.pipeline_limit.saturating_mul(MAX_RESULT_LOGICAL_BYTES),
            result_capacity_byte_limit: self
                .pipeline_limit
                .saturating_mul(MAX_RESULT_CAPACITY_BYTES),
            generation_entries: self.generations.len(),
            generation_capacity: self.generations.capacity(),
            pending_capacity: self.pending.capacity(),
            ready_capacity: self.ready.capacity(),
            completed_unconsumed: self.pool.accounting.completed.load(Ordering::Relaxed),
            completed_cpu_bytes: self.pool.accounting.completed_bytes.load(Ordering::Relaxed),
            live_snapshots: self.pool.accounting.snapshots.load(Ordering::Relaxed),
            live_snapshot_bytes: self.pool.accounting.snapshot_bytes.load(Ordering::Relaxed),
            pending: self.pending.len(),
            in_flight: self.in_flight.len(),
            ready: self.ready.len(),
            ready_cpu_bytes: self.ready.iter().map(CompletedMesh::logical_bytes).sum(),
            pending_snapshot_bytes: self
                .pending
                .values()
                .map(|job| job.snapshot.snapshot_bytes())
                .sum(),
            in_flight_snapshot_bytes: self.in_flight.values().map(|(b, _)| b).sum(),
            worker_count: self.pool.threads.len(),
            ..self.counters
        }
    }

    #[must_use]
    pub fn column_stage(&self, column: rustcraft_engine_core::ChunkPos) -> Option<ColumnMeshStage> {
        if self.ready.iter().any(|result| result.section.0 == column) {
            Some(ColumnMeshStage::UploadPending)
        } else if self.in_flight.keys().any(|section| section.0 == column) {
            Some(ColumnMeshStage::InFlight)
        } else if self.pending.keys().any(|section| section.0 == column) {
            Some(ColumnMeshStage::Pending)
        } else {
            None
        }
    }

    /// Read-only current-section ownership; retired tokens do not require historical map keys.
    pub fn generation_entry_count(&self) -> usize {
        self.generations.len()
    }

    /// Selected-section query; does not alter scheduling or generation retention.
    pub fn section_stage(&self, section: SectionPos) -> Option<ColumnMeshStage> {
        if self.ready.iter().any(|r| r.section == section) {
            Some(ColumnMeshStage::UploadPending)
        } else if self.in_flight.contains_key(&section) {
            Some(ColumnMeshStage::InFlight)
        } else if self.pending.contains_key(&section) {
            Some(ColumnMeshStage::Pending)
        } else {
            None
        }
    }

    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.pending.is_empty() && self.in_flight.is_empty() && self.ready.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AtlasRegion, Face, RenderWorld, TextureHandle};
    use rustcraft_engine_core::{BlockId, BlockPos, World};

    #[test]
    fn rsm1_unique_metadata_and_upload_pressure_are_bounded() {
        let mut history = MeshScheduler::new(2, 4, StaticMaterials);
        let template = snapshot(1, 0);
        for x in 0..10000 {
            let mut next = template.clone();
            next.position.x = x;
            history.mark_dirty(next);
            history.remove_section((rustcraft_engine_core::ChunkPos { x, z: 0 }, 0));
        }
        assert_eq!(history.stats().generation_entries, 0);
        assert_eq!(history.stats().pending_order_entries, 0);
        drop(history);
        let mut scheduler = MeshScheduler::new(2, 4, StaticMaterials);
        for x in 0..256 {
            scheduler.mark_dirty(snapshot(1, x));
        }
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        while scheduler.stats().ready < 6 {
            scheduler.poll();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        for _ in 0..1000 {
            scheduler.poll();
        }
        let m = scheduler.stats();
        assert_eq!(m.ready, 6);
        assert_eq!(m.pending, 250);
        assert_eq!(m.in_flight, 0);
        assert!(m.ready_capacity_bytes <= m.result_capacity_byte_limit);
        println!(
            "RSM1_PRESSURE ready={} pending={} ready_bytes={} ready_capacity_bytes={} window={} logical_limit={} capacity_limit={}",
            m.ready,
            m.pending,
            m.ready_cpu_bytes,
            m.ready_capacity_bytes,
            m.pipeline_limit,
            m.result_logical_byte_limit,
            m.result_capacity_byte_limit
        );
        let mut accepted = 0;
        while accepted < 256 {
            scheduler.poll();
            accepted += scheduler
                .take_ready(Vec3::ZERO, Vec3::ZERO, 6, usize::MAX)
                .len();
            assert!(scheduler.stats().in_flight + scheduler.stats().ready <= 6);
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(scheduler.is_idle());
        let accounting = Arc::clone(&scheduler.pool.accounting);
        drop(scheduler);
        assert_eq!(accounting.snapshots.load(Ordering::Relaxed), 0);
        assert_eq!(accounting.completed.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn rsm1_stalled_poll_and_shutdown_release_all_owned_messages() {
        let mut scheduler = MeshScheduler::new(2, 4, StaticMaterials);
        for x in 0..256 {
            scheduler.mark_dirty(snapshot(1, x));
        }
        let accounting = Arc::clone(&scheduler.pool.accounting);
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        while accounting.completed.load(Ordering::Relaxed) < scheduler.stats().in_flight {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(accounting.completed.load(Ordering::Relaxed) <= 6);
        drop(scheduler); // Nonblocking producers drain/join without polling or uploading.
        assert_eq!(accounting.snapshots.load(Ordering::Relaxed), 0);
        assert_eq!(accounting.completed.load(Ordering::Relaxed), 0);
        assert_eq!(accounting.completed_bytes.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn rsm1_large_payload_pressure_obeys_byte_envelope_and_resumes() {
        let mut world = World::new(BlockId(0));
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    if (x + y + z) % 2 == 0 {
                        world.set(BlockPos { x, y, z }, BlockId(1));
                    }
                }
            }
        }
        let template = RenderWorld::from_world(&world)
            .chunks()
            .next()
            .unwrap()
            .clone();
        let mut scheduler = MeshScheduler::new(2, 2, StaticMaterials);
        for x in 0..16 {
            let mut next = template.clone();
            next.position.x = x;
            scheduler.mark_dirty(next);
        }
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        while scheduler.stats().ready < 4 {
            scheduler.poll();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let m = scheduler.stats();
        assert!(m.ready_cpu_bytes > 4_000_000);
        assert!(m.ready_cpu_bytes <= m.result_logical_byte_limit);
        assert!(m.ready_capacity_bytes <= m.result_capacity_byte_limit);
        println!(
            "RSM1_LARGE_PRESSURE ready={} logical={} capacity={} limit={}",
            m.ready, m.ready_cpu_bytes, m.ready_capacity_bytes, m.result_capacity_byte_limit
        );
        let mut accepted = 0;
        while accepted < 16 {
            scheduler.poll();
            accepted += scheduler
                .take_ready(Vec3::ZERO, Vec3::ZERO, 4, usize::MAX)
                .len();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(scheduler.is_idle());
    }
    #[test]
    fn rsm1_evicted_revisited_section_rejects_late_old_work() {
        let rendezvous = Arc::new(std::sync::Barrier::new(2));
        let mut scheduler = MeshScheduler::new(
            1,
            1,
            BlockingMaterials {
                rendezvous: Arc::clone(&rendezvous),
                first_call: std::sync::atomic::AtomicBool::new(true),
            },
        );
        let section = (rustcraft_engine_core::ChunkPos { x: 0, z: 0 }, 0);
        let old = scheduler.mark_dirty(snapshot(1, 0));
        rendezvous.wait();
        scheduler.remove_section(section);
        let new = scheduler.mark_dirty(snapshot(2, 0));
        assert_ne!(old, new);
        assert_eq!(scheduler.stats().retired_inflight, 1);
        rendezvous.wait();
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        let ready = loop {
            scheduler.poll();
            let ready = scheduler.take_ready(Vec3::ZERO, Vec3::ZERO, 2, usize::MAX);
            if !ready.is_empty() {
                break ready;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].generation, new);
        assert_eq!(ready[0].pages[0].texture, TextureHandle(2));
        assert_eq!(scheduler.stats().mesh_jobs_discarded_stale, 1);
        // Even a late old ready payload is rechecked at the upload handoff.
        scheduler.ready.push(CompletedMesh {
            section,
            generation: old,
            pages: vec![],
            queue_wait_ms: 0.,
            mesh_ms: 0.,
            completed_at: Instant::now(),
        });
        assert!(
            scheduler
                .take_ready(Vec3::ZERO, Vec3::ZERO, 2, usize::MAX)
                .is_empty()
        );
        scheduler.remove_section(section);
        assert_eq!(scheduler.stats().generation_entries, 0);
        assert_eq!(scheduler.stats().pending_order_entries, 0);
    }

    #[test]
    fn worker_count_is_bounded_and_never_zero() {
        assert_eq!(bounded_mesh_worker_count(0), 1);
        assert_eq!(bounded_mesh_worker_count(1), 1);
        assert_eq!(
            bounded_mesh_worker_count(MAX_MESH_WORKERS),
            MAX_MESH_WORKERS
        );
        assert_eq!(bounded_mesh_worker_count(usize::MAX), MAX_MESH_WORKERS);
    }

    fn snapshot(block_id: u32, section_x: i32) -> RenderChunk {
        snapshot_at(block_id, section_x, 0)
    }

    fn snapshot_at(block_id: u32, section_x: i32, section_z: i32) -> RenderChunk {
        let mut world = World::new(BlockId(0));
        world.set(
            BlockPos {
                x: section_x * 16,
                y: 0,
                z: section_z * 16,
            },
            BlockId(block_id),
        );
        RenderWorld::from_world(&world)
            .chunks()
            .next()
            .unwrap()
            .clone()
    }

    struct StaticMaterials;
    impl BlockTextureResolver for StaticMaterials {
        fn texture(&self, block: BlockId, _: Face) -> Option<AtlasRegion> {
            (block.0 != 0).then(|| AtlasRegion::full(TextureHandle(block.0)))
        }
        fn opaque(&self, block: BlockId) -> bool {
            block.0 != 0
        }
    }

    #[test]
    fn repeated_dirty_sections_coalesce_and_only_latest_generation_is_accepted() {
        let rendezvous = Arc::new(std::sync::Barrier::new(2));
        let mut scheduler = MeshScheduler::new(
            1,
            1,
            BlockingMaterials {
                rendezvous: Arc::clone(&rendezvous),
                first_call: std::sync::atomic::AtomicBool::new(true),
            },
        );
        assert_eq!(scheduler.stats().worker_count, 1);
        assert_eq!(scheduler.mark_dirty(snapshot(1, 0)), 1);
        rendezvous.wait();
        let mut newest = 1;
        for generation in 2..=100 {
            newest = scheduler.mark_dirty(snapshot(generation, 0));
        }
        assert_eq!(scheduler.stats().in_flight, 1);
        assert_eq!(scheduler.stats().pending, 1);
        rendezvous.wait();
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        let completed = loop {
            scheduler.poll();
            let ready = scheduler.take_ready(Vec3::ZERO, Vec3::ZERO, 4, usize::MAX);
            if !ready.is_empty() {
                break ready;
            }
            assert!(Instant::now() < deadline, "mesh workers did not converge");
            std::thread::yield_now();
        };
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].generation, newest);
        assert_eq!(completed[0].pages[0].texture, TextureHandle(100));
        let stats = scheduler.stats();
        assert_eq!(stats.mesh_jobs_submitted, 2);
        assert_eq!(stats.mesh_jobs_completed, 2);
        assert_eq!(stats.mesh_jobs_discarded_stale, 1);
        assert_eq!(stats.mesh_jobs_coalesced, 99);
        assert_eq!(stats.mesh_jobs_failed, 0);
        assert!(scheduler.is_idle());
        drop(scheduler); // Drop closes the bounded queue and joins the worker.
    }

    struct BlockingMaterials {
        rendezvous: Arc<std::sync::Barrier>,
        first_call: std::sync::atomic::AtomicBool,
    }

    impl BlockTextureResolver for BlockingMaterials {
        fn texture(&self, block: BlockId, _: Face) -> Option<AtlasRegion> {
            if self
                .first_call
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                self.rendezvous.wait();
                self.rendezvous.wait();
            }
            (block.0 != 0).then(|| AtlasRegion::full(TextureHandle(block.0)))
        }
        fn opaque(&self, block: BlockId) -> bool {
            block.0 != 0
        }
    }

    #[test]
    fn removed_section_invalidates_an_in_flight_mesh_result() {
        let rendezvous = Arc::new(std::sync::Barrier::new(2));
        let mut scheduler = MeshScheduler::new(
            1,
            1,
            BlockingMaterials {
                rendezvous: Arc::clone(&rendezvous),
                first_call: std::sync::atomic::AtomicBool::new(true),
            },
        );
        let section = (rustcraft_engine_core::ChunkPos { x: 0, z: 0 }, 0);
        let old = scheduler.mark_dirty(snapshot(1, 0));
        rendezvous.wait();
        assert!(scheduler.remove_section(section) > old);
        rendezvous.wait();
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        while !scheduler.is_idle() {
            scheduler.poll();
            assert!(
                Instant::now() < deadline,
                "removed section result did not drain"
            );
            std::thread::yield_now();
        }
        assert_eq!(scheduler.stats().ready, 0);
        assert_eq!(scheduler.stats().mesh_jobs_discarded_stale, 1);
        drop(scheduler);
    }

    #[test]
    fn upload_budget_retains_remaining_completed_sections() {
        let mut scheduler = MeshScheduler::new(2, 2, StaticMaterials);
        for x in 0..3 {
            scheduler.mark_dirty(snapshot(x as u32 + 1, x));
        }
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        while scheduler.stats().mesh_jobs_completed < 3 {
            scheduler.poll();
            assert!(Instant::now() < deadline, "meshing did not finish");
            std::thread::yield_now();
        }
        assert_eq!(scheduler.stats().ready, 3);
        let per_section_bytes = scheduler.stats().ready_cpu_bytes / 3;
        let first = scheduler.take_ready(Vec3::ZERO, Vec3::ZERO, 3, per_section_bytes);
        assert_eq!(first.len(), 1);
        assert_eq!(scheduler.stats().ready, 2);
        let rest = scheduler.take_ready(Vec3::ZERO, Vec3::ZERO, 2, usize::MAX);
        assert_eq!(rest.len(), 2);
        assert_eq!(scheduler.stats().ready, 0);
    }

    #[test]
    fn ready_mesh_uploads_prefer_camera_forward_within_same_urgency_ring() {
        let mut scheduler = MeshScheduler::new(3, 3, StaticMaterials);
        let forward = (rustcraft_engine_core::ChunkPos { x: 1, z: 0 }, 0);
        let rear = (rustcraft_engine_core::ChunkPos { x: -1, z: 0 }, 0);
        let lateral = (rustcraft_engine_core::ChunkPos { x: 0, z: 1 }, 0);
        scheduler.mark_dirty(snapshot_at(1, forward.0.x, forward.0.z));
        scheduler.mark_dirty(snapshot_at(2, rear.0.x, rear.0.z));
        scheduler.mark_dirty(snapshot_at(3, lateral.0.x, lateral.0.z));

        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        while scheduler.stats().mesh_jobs_completed < 3 {
            scheduler.poll();
            assert!(Instant::now() < deadline, "meshing did not finish");
            std::thread::yield_now();
        }
        let ready = scheduler.take_ready(
            Vec3::new(8.0, 8.0, 8.0),
            Vec3::new(1.0, 0.0, 0.0),
            1,
            usize::MAX,
        );
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].section, forward);
    }

    #[test]
    fn ready_generation_is_discarded_if_newer_generation_arrives_before_upload() {
        let mut scheduler = MeshScheduler::new(1, 1, StaticMaterials);
        let section = (rustcraft_engine_core::ChunkPos { x: 0, z: 0 }, 0);
        assert_eq!(scheduler.mark_dirty(snapshot(1, 0)), 1);
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        while scheduler.stats().mesh_jobs_completed < 1 {
            scheduler.poll();
            assert!(Instant::now() < deadline, "first generation did not finish");
            std::thread::yield_now();
        }
        assert_eq!(scheduler.stats().ready, 1);

        assert_eq!(scheduler.mark_dirty(snapshot(2, 0)), 2);
        assert!(
            scheduler
                .take_ready(Vec3::ZERO, Vec3::ZERO, 1, usize::MAX)
                .is_empty()
        );
        assert_eq!(scheduler.stats().mesh_jobs_discarded_stale, 1);

        while scheduler.stats().mesh_jobs_completed < 2 || scheduler.stats().ready == 0 {
            scheduler.poll();
            assert!(Instant::now() < deadline, "new generation did not finish");
            std::thread::yield_now();
        }
        let latest = scheduler.take_ready(Vec3::ZERO, Vec3::ZERO, 1, usize::MAX);
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].section, section);
        assert_eq!(latest[0].generation, 2);
        assert_eq!(latest[0].pages[0].texture, TextureHandle(2));
        assert!(scheduler.is_idle());
    }
}
