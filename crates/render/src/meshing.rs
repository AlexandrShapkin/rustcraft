//! Bounded CPU section meshing with generation-based stale-result rejection.

use crate::{BlockTextureResolver, PageMesh, RenderChunk, build_section_mesh_pages};
use rustcraft_engine_core::{SectionPos, Vec3};
use std::{
    collections::{HashMap, VecDeque},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Instant,
};

pub const MAX_MESH_WORKERS: usize = 32;

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

#[derive(Debug)]
struct MeshJob {
    section: SectionPos,
    generation: u64,
    snapshot: RenderChunk,
}

#[derive(Debug)]
pub struct CompletedMesh {
    pub section: SectionPos,
    pub generation: u64,
    pub pages: Vec<PageMesh>,
    pub mesh_ms: f64,
}

impl CompletedMesh {
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

struct WorkerPool {
    sender: Option<mpsc::SyncSender<MeshJob>>,
    receiver: mpsc::Receiver<WorkerResult>,
    threads: Vec<thread::JoinHandle<()>>,
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
        let mut threads = Vec::with_capacity(worker_count);
        for index in 0..bounded_mesh_worker_count(worker_count) {
            let jobs = Arc::clone(&jobs);
            let results = result_sender.clone();
            let resolver = Arc::clone(&resolver);
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
                                    mesh_ms: started.elapsed().as_secs_f64() * 1000.0,
                                }),
                                Err(_) => WorkerResult::Failed {
                                    section,
                                    generation,
                                },
                            };
                            if results.send(result).is_err() {
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
}

pub struct MeshScheduler {
    pool: WorkerPool,
    generations: HashMap<SectionPos, u64>,
    pending: HashMap<SectionPos, MeshJob>,
    pending_order: VecDeque<SectionPos>,
    in_flight: HashMap<SectionPos, usize>,
    ready: Vec<CompletedMesh>,
    counters: MeshingStats,
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
            pending: HashMap::new(),
            pending_order: VecDeque::new(),
            in_flight: HashMap::new(),
            ready: Vec::new(),
            counters: MeshingStats::default(),
        }
    }

    pub fn mark_dirty(&mut self, snapshot: RenderChunk) -> u64 {
        let section = (snapshot.position, snapshot.section_y);
        let generation = self.generations.entry(section).or_default();
        *generation = generation.saturating_add(1);
        let generation = *generation;
        let job = MeshJob {
            section,
            generation,
            snapshot,
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

    pub fn remove_section(&mut self, section: SectionPos) -> u64 {
        let generation = self.generations.entry(section).or_default();
        *generation = generation.saturating_add(1);
        self.pending.remove(&section);
        self.ready.retain(|result| {
            let keep = result.section != section;
            if !keep {
                self.counters.mesh_jobs_discarded_stale += 1;
            }
            keep
        });
        *generation
    }

    #[must_use]
    pub fn current_generation(&self, section: SectionPos) -> Option<u64> {
        self.generations.get(&section).copied()
    }

    pub fn poll(&mut self) {
        while let Ok(result) = self.pool.receiver.try_recv() {
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
        while let Some(section) = self.pending_order.pop_front() {
            if self.in_flight.contains_key(&section) {
                continue;
            }
            let Some(job) = self.pending.remove(&section) else {
                continue;
            };
            let snapshot_bytes = job.snapshot.snapshot_bytes();
            match self.pool.try_submit(job) {
                Ok(()) => {
                    self.in_flight.insert(section, snapshot_bytes);
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
        self.ready.sort_by(|a, b| {
            let distance = |result: &CompletedMesh| {
                let x = result.section.0.x as f32 * 16.0 + 8.0 - camera.x;
                let y = result.section.1 as f32 * 16.0 + 8.0 - camera.y;
                let z = result.section.0.z as f32 * 16.0 + 8.0 - camera.z;
                x * x + y * y + z * z
            };
            distance(a)
                .total_cmp(&distance(b))
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
            pending: self.pending.len(),
            in_flight: self.in_flight.len(),
            ready: self.ready.len(),
            ready_cpu_bytes: self.ready.iter().map(CompletedMesh::logical_bytes).sum(),
            pending_snapshot_bytes: self
                .pending
                .values()
                .map(|job| job.snapshot.snapshot_bytes())
                .sum(),
            in_flight_snapshot_bytes: self.in_flight.values().sum(),
            worker_count: self.pool.threads.len(),
            ..self.counters
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
        let mut world = World::new(BlockId(0));
        world.set(
            BlockPos {
                x: section_x * 16,
                y: 0,
                z: 0,
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
            let ready = scheduler.take_ready(Vec3::ZERO, 4, usize::MAX);
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
        let first = scheduler.take_ready(Vec3::ZERO, 3, per_section_bytes);
        assert_eq!(first.len(), 1);
        assert_eq!(scheduler.stats().ready, 2);
        let rest = scheduler.take_ready(Vec3::ZERO, 2, usize::MAX);
        assert_eq!(rest.len(), 2);
        assert_eq!(scheduler.stats().ready, 0);
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
        assert!(scheduler.take_ready(Vec3::ZERO, 1, usize::MAX).is_empty());
        assert_eq!(scheduler.stats().mesh_jobs_discarded_stale, 1);

        while scheduler.stats().mesh_jobs_completed < 2 || scheduler.stats().ready == 0 {
            scheduler.poll();
            assert!(Instant::now() < deadline, "new generation did not finish");
            std::thread::yield_now();
        }
        let latest = scheduler.take_ready(Vec3::ZERO, 1, usize::MAX);
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].section, section);
        assert_eq!(latest[0].generation, 2);
        assert_eq!(latest[0].pages[0].texture, TextureHandle(2));
        assert!(scheduler.is_idle());
    }
}
