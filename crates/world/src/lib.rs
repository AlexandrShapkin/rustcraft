//! Generic deterministic generation contracts and versioned world storage.
//! This crate stores semantic identities and never interprets game-specific terrain policy.

use rustcraft_engine_core::{BlockId, BlockState, CHUNK_VOLUME, Chunk, ChunkPos, EntityId};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
};

const WORLD_MAGIC: &[u8; 8] = b"RCWORLD\0";
const CHUNK_MAGIC: &[u8; 8] = b"RCCHNK\0\0";
const PLAYER_MAGIC: &[u8; 8] = b"RCPLAY\0\0";
const WORLD_STATE_MAGIC: &[u8; 8] = b"RCSTATE\0";
pub const WORLD_FORMAT_VERSION: u32 = 1;
pub const WORLD_METADATA_VERSION: u32 = 2;
pub const PERSISTED_STATE_SCHEMA_VERSION: u32 = 1;
pub const CHUNK_FORMAT_VERSION: u32 = 3;
const LEGACY_CHUNK_FORMAT_VERSION: u32 = 2;
pub const MAX_METADATA_BYTES: usize = 64 * 1024;
pub const MAX_CHUNK_BYTES: usize = 8 * 1024 * 1024;
pub const PLAYER_RECORD_VERSION: u32 = 2;
const LEGACY_PLAYER_RECORD_VERSION: u32 = 1;
pub const MAX_PLAYER_RECORD_BYTES: usize = 64 * 1024;
pub const MAX_PLAYER_COMPONENTS: usize = 64;
pub const MAX_PLAYER_COMPONENT_ID_BYTES: usize = 256;
pub const MAX_PLAYER_COMPONENT_BYTES: usize = 32 * 1024;
pub const WORLD_STATE_RECORD_VERSION: u32 = 1;
pub const MAX_WORLD_STATE_BYTES: usize = 64 * 1024;
pub const MAX_WORLD_COMPONENTS: usize = 64;
pub const MAX_WORLD_COMPONENT_ID_BYTES: usize = 256;
pub const MAX_WORLD_COMPONENT_BYTES: usize = 32 * 1024;
pub const MAX_SPATIAL_RECORDS: usize = 1_024;
pub const MAX_SPATIAL_TOMBSTONES: usize = 1_024;
pub const MAX_SPATIAL_TYPE_ID_BYTES: usize = 256;
pub const MAX_SPATIAL_PAYLOAD_BYTES: usize = 4 * 1024;
pub const MAX_SECTIONS: usize = 64;
pub const MAX_PALETTE: usize = CHUNK_VOLUME;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldMetadata {
    pub seed: i64,
    pub game_id: String,
    /// Informational identity of the active game definitions; never a chunk-load gate.
    pub profile_fingerprint: String,
    /// Version of the persisted semantic BlockKey + u16 variant representation.
    pub persistence_schema_version: u32,
    pub generator_id: String,
    pub generator_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldCompatibilityError {
    GameProfile {
        saved: String,
        active: String,
    },
    PersistenceSchema {
        saved: u32,
        active: u32,
    },
    Generator {
        saved_id: String,
        saved_version: u32,
        active_id: String,
        active_version: u32,
    },
}

impl std::fmt::Display for WorldCompatibilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GameProfile { saved, active } => write!(
                f,
                "game/profile family mismatch (saved {saved}, active {active})"
            ),
            Self::PersistenceSchema { saved, active } => write!(
                f,
                "persisted state schema mismatch (saved v{saved}, active v{active}); explicit migration required"
            ),
            Self::Generator {
                saved_id,
                saved_version,
                active_id,
                active_version,
            } => write!(
                f,
                "missing chunks require saved generator {saved_id}:v{saved_version}, but active generator is {active_id}:v{active_version}"
            ),
        }
    }
}

/// Validate only hard persistence boundaries. Profile fingerprints are informational: existing
/// semantic palettes are validated entry-by-entry by `load_runtime_chunk`.
pub fn validate_world_compatibility(
    saved: &WorldMetadata,
    active: &WorldMetadata,
    missing_chunks_require_generation: bool,
) -> Result<(), WorldCompatibilityError> {
    if saved.game_id != active.game_id {
        return Err(WorldCompatibilityError::GameProfile {
            saved: saved.game_id.clone(),
            active: active.game_id.clone(),
        });
    }
    if saved.persistence_schema_version != active.persistence_schema_version {
        return Err(WorldCompatibilityError::PersistenceSchema {
            saved: saved.persistence_schema_version,
            active: active.persistence_schema_version,
        });
    }
    if missing_chunks_require_generation
        && (saved.generator_id != active.generator_id
            || saved.generator_version != active.generator_version)
    {
        return Err(WorldCompatibilityError::Generator {
            saved_id: saved.generator_id.clone(),
            saved_version: saved.generator_version,
            active_id: active.generator_id.clone(),
            active_version: active.generator_version,
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSection {
    pub y: i32,
    pub states: Vec<(String, u16)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredChunk {
    pub position: ChunkPos,
    pub sections: Vec<StoredSection>,
    /// Generic spatial envelopes. The active game owns each semantic type and payload codec.
    pub spatial_records: Vec<SpatialRecord>,
    /// Bounded recovery markers used while an entity removal is being made durable in another
    /// column/domain. They are pruned after the source snapshot is safely rewritten.
    pub spatial_tombstones: Vec<SpatialTombstone>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpatialRecord {
    pub entity_id: EntityId,
    pub entity_revision: u64,
    pub entity_type: String,
    pub schema_version: u32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpatialTombstone {
    pub entity_id: EntityId,
    pub entity_revision: u64,
    pub source: ChunkPos,
}

#[derive(Debug)]
pub struct LoadedColumn {
    pub sections: Vec<(i32, Chunk)>,
    pub spatial_records: Vec<SpatialRecord>,
    pub spatial_tombstones: Vec<SpatialTombstone>,
}

/// Opaque game-owned payload for one independently versioned global world component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldStateComponent {
    pub id: String,
    pub schema_version: u32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldStateRecord {
    pub revision: u64,
    pub components: Vec<WorldStateComponent>,
    pub recovered_from_checkpoint: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WorldStateCheckpointIoMetrics {
    pub encoded_bytes: usize,
    pub write_ms: f64,
    pub sync_ms: f64,
}

/// Opaque game-owned payload for one independently versioned durable player component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerComponent {
    pub id: String,
    pub schema_version: u32,
    pub payload: Vec<u8>,
}

/// Generic versioned player record. Component meaning remains game-owned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRecord {
    pub player_id: String,
    pub revision: u64,
    pub components: Vec<PlayerComponent>,
    /// True only when load recovered from the older of two checkpoint slots.
    pub recovered_from_checkpoint: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PlayerCheckpointIoMetrics {
    pub encoded_bytes: usize,
    pub write_ms: f64,
    pub sync_ms: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ChunkEncodingMetrics {
    pub raw_payload_bytes: usize,
    pub stored_file_bytes: usize,
    pub compression_ms: f64,
    pub compression_method: u8,
}

#[derive(Debug)]
pub enum WorldError {
    Io(io::Error),
    InvalidWorldName,
    InvalidData(&'static str),
    UnsupportedVersion(u32),
    UnknownBlock(String),
    Compatibility(String),
}

impl std::fmt::Display for WorldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "world I/O: {error}"),
            Self::InvalidWorldName => f.write_str("invalid world name"),
            Self::InvalidData(message) => write!(f, "invalid world data: {message}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported world format version {version}")
            }
            Self::UnknownBlock(key) => write!(f, "world requires unavailable semantic block {key}"),
            Self::Compatibility(message) => write!(f, "world compatibility error: {message}"),
        }
    }
}
impl std::error::Error for WorldError {}
impl From<io::Error> for WorldError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Game-owned codec bridge. Persistent data stores `key`, never profile-local numeric IDs.
pub trait SemanticBlockResolver: Send + Sync {
    fn key_for(&self, state: BlockState) -> Option<&str>;
    fn state_for(&self, key: &str, variant: u16) -> Option<BlockState>;
}

/// Storage boundary consumed by world ownership/scheduling. The filesystem backend is the only
/// implementation in M4; other containers can be added without changing simulation policy.
pub trait WorldStore: Send + Sync {
    fn load_metadata(&self) -> Result<WorldMetadata, WorldError>;
    fn store_metadata(&self, metadata: &WorldMetadata) -> Result<(), WorldError>;
    fn load_chunk(&self, position: ChunkPos) -> Result<StoredChunk, WorldError>;
    fn store_chunk(&self, chunk: &StoredChunk) -> Result<(), WorldError>;
    fn load_player(&self, player_id: &str) -> Result<Option<PlayerRecord>, WorldError>;
    fn store_player(&self, record: &PlayerRecord) -> Result<(), WorldError>;
    fn load_world_state(&self) -> Result<Option<WorldStateRecord>, WorldError>;
    fn store_world_state(&self, record: &WorldStateRecord) -> Result<(), WorldError>;
    fn chunk_exists(&self, position: ChunkPos) -> bool;
    fn flush(&self) -> Result<(), WorldError>;
    fn close(&self) -> Result<(), WorldError> {
        self.flush()
    }
}

/// Policy-independent generator interface. Implementations must derive randomness from the
/// supplied world seed and coordinates, not call order or shared mutable RNG state.
pub trait ChunkGenerator: Send + Sync + 'static {
    fn id(&self) -> &str;
    fn version(&self) -> u32;
    fn generate(&self, seed: i64, position: ChunkPos) -> Result<Vec<(i32, Chunk)>, WorldError>;
}

#[derive(Debug)]
pub struct GeneratedColumn {
    pub position: ChunkPos,
    pub generation: u64,
    pub queue_wait_ms: f64,
    pub generation_ms: f64,
    pub sections: Result<Vec<(i32, Chunk)>, WorldError>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GenerationMetrics {
    pub submitted: u64,
    pub coalesced: u64,
    pub completed: u64,
    pub stale_discarded: u64,
    pub pending: usize,
    pub in_flight: usize,
    pub generation_total_ms: f64,
}

pub struct LoadCompletion {
    pub request: ResidencyRequest,
    /// `None` means no persisted file exists. All other failures are explicit and must not
    /// transition into generation.
    pub result: Result<Option<LoadedColumn>, WorldError>,
    pub queue_wait_ms: f64,
    pub load_ms: f64,
}

struct LoadJob {
    storage: WorldStorage,
    request: ResidencyRequest,
    resolver: Arc<dyn SemanticBlockResolver>,
    queued_at: std::time::Instant,
}

struct LoadWorker {
    sender: Option<SyncSender<LoadJob>>,
    thread: Option<JoinHandle<()>>,
}

/// Bounded chunk file/decode workers. Both file access and semantic palette resolution happen on
/// workers; missing is represented separately from corrupt/incompatible data.
pub struct ChunkLoadScheduler {
    workers: Vec<LoadWorker>,
    results: Option<Receiver<LoadCompletion>>,
    next_worker: usize,
    queued: Arc<AtomicUsize>,
    in_flight: Arc<AtomicUsize>,
    completed: Arc<AtomicU64>,
    missing: Arc<AtomicU64>,
    failed: Arc<AtomicU64>,
    load_micros: Arc<AtomicU64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ChunkLoadMetrics {
    pub queued: usize,
    pub in_flight: usize,
    pub completed: u64,
    pub missing: u64,
    pub failed: u64,
    pub total_load_ms: f64,
}

impl ChunkLoadScheduler {
    pub fn new(worker_count: usize, queue_capacity_per_worker: usize) -> Self {
        let count = worker_count.max(1);
        let capacity = queue_capacity_per_worker.max(1);
        let (result_tx, results) = mpsc::sync_channel(count * capacity);
        let queued = Arc::new(AtomicUsize::new(0));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicU64::new(0));
        let missing = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicU64::new(0));
        let load_micros = Arc::new(AtomicU64::new(0));
        let mut workers = Vec::with_capacity(count);
        for _ in 0..count {
            let (sender, receiver) = mpsc::sync_channel::<LoadJob>(capacity);
            let tx = result_tx.clone();
            let q = queued.clone();
            let f = in_flight.clone();
            let c = completed.clone();
            let m = missing.clone();
            let e = failed.clone();
            let t = load_micros.clone();
            let thread = thread::Builder::new()
                .name("world-load-worker".into())
                .spawn(move || {
                    while let Ok(job) = receiver.recv() {
                        q.fetch_sub(1, Ordering::Relaxed);
                        f.fetch_add(1, Ordering::Relaxed);
                        let queue_wait_ms = job.queued_at.elapsed().as_secs_f64() * 1000.0;
                        let started = std::time::Instant::now();
                        let result = job.storage.load_runtime_column_if_present(
                            job.request.position,
                            job.resolver.as_ref(),
                        );
                        let elapsed = started.elapsed();
                        t.fetch_add(
                            elapsed.as_micros().min(u128::from(u64::MAX)) as u64,
                            Ordering::Relaxed,
                        );
                        f.fetch_sub(1, Ordering::Relaxed);
                        match &result {
                            Ok(Some(_)) => {
                                c.fetch_add(1, Ordering::Relaxed);
                            }
                            Ok(None) => {
                                m.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(_) => {
                                e.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        if tx
                            .send(LoadCompletion {
                                request: job.request,
                                result,
                                queue_wait_ms,
                                load_ms: elapsed.as_secs_f64() * 1000.0,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .expect("world load worker thread creation failed");
            workers.push(LoadWorker {
                sender: Some(sender),
                thread: Some(thread),
            });
        }
        drop(result_tx);
        Self {
            workers,
            results: Some(results),
            next_worker: 0,
            queued,
            in_flight,
            completed,
            missing,
            failed,
            load_micros,
        }
    }

    pub fn submit(
        &mut self,
        storage: WorldStorage,
        request: ResidencyRequest,
        resolver: Arc<dyn SemanticBlockResolver>,
    ) -> Result<(), WorldError> {
        let count = self.workers.len();
        for offset in 0..count {
            let index = (self.next_worker + offset) % count;
            let job = LoadJob {
                storage: storage.clone(),
                request,
                resolver: resolver.clone(),
                queued_at: std::time::Instant::now(),
            };
            self.queued.fetch_add(1, Ordering::Relaxed);
            match self.workers[index]
                .sender
                .as_ref()
                .expect("live load worker")
                .try_send(job)
            {
                Ok(()) => {
                    self.next_worker = (index + 1) % count;
                    return Ok(());
                }
                Err(TrySendError::Full(_)) => {
                    self.queued.fetch_sub(1, Ordering::Relaxed);
                }
                Err(TrySendError::Disconnected(_)) => {
                    self.queued.fetch_sub(1, Ordering::Relaxed);
                }
            }
        }
        Err(WorldError::InvalidData("load queue full"))
    }

    pub fn take_ready(&mut self, limit: usize) -> Vec<LoadCompletion> {
        let mut ready = Vec::with_capacity(limit);
        let Some(results) = self.results.as_ref() else {
            return ready;
        };
        while ready.len() < limit {
            match results.try_recv() {
                Ok(result) => ready.push(result),
                Err(_) => break,
            }
        }
        ready
    }

    pub fn metrics(&self) -> ChunkLoadMetrics {
        ChunkLoadMetrics {
            queued: self.queued.load(Ordering::Relaxed),
            in_flight: self.in_flight.load(Ordering::Relaxed),
            completed: self.completed.load(Ordering::Relaxed),
            missing: self.missing.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            total_load_ms: self.load_micros.load(Ordering::Relaxed) as f64 / 1000.0,
        }
    }
}

impl Drop for ChunkLoadScheduler {
    fn drop(&mut self) {
        for worker in &mut self.workers {
            worker.sender.take();
        }
        self.results.take();
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

struct GenerationJob {
    generator: Arc<dyn ChunkGenerator>,
    seed: i64,
    position: ChunkPos,
    generation: u64,
    queued_at: std::time::Instant,
}
struct Worker {
    sender: Option<SyncSender<GenerationJob>>,
    thread: Option<JoinHandle<()>>,
}

/// Bounded, owned-input generation workers. Generation order is intentionally unspecified;
/// implementations must be coordinate/stage deterministic. A newer generation invalidates older
/// outstanding results before callers can publish them.
pub struct GenerationScheduler {
    workers: Vec<Worker>,
    results: Option<Receiver<GeneratedColumn>>,
    latest: HashMap<ChunkPos, u64>,
    outstanding: HashSet<(ChunkPos, u64)>,
    next_worker: usize,
    submitted: u64,
    coalesced: u64,
    stale_discarded: u64,
    pending: Arc<AtomicUsize>,
    in_flight: Arc<AtomicUsize>,
    completed: Arc<AtomicU64>,
    generation_micros: Arc<AtomicU64>,
}

struct SaveJob {
    storage: WorldStorage,
    token: SaveToken,
    chunk: StoredChunk,
}
pub struct SaveCompletion {
    pub token: SaveToken,
    pub result: Result<ChunkEncodingMetrics, String>,
}
struct SaveWorker {
    sender: Option<SyncSender<SaveJob>>,
    thread: Option<JoinHandle<()>>,
}

/// Bounded disk-write workers; serialization/compression and atomic replacement happen away from
/// the caller. The owner must feed completions back to `PersistenceDirtyTracker`.
pub struct SaveScheduler {
    workers: Vec<SaveWorker>,
    results: Option<Receiver<SaveCompletion>>,
    next_worker: usize,
    queued: Arc<AtomicUsize>,
    in_flight: Arc<AtomicUsize>,
    completed: Arc<AtomicU64>,
    failed: Arc<AtomicU64>,
}

impl SaveScheduler {
    pub fn new(worker_count: usize, queue_capacity_per_worker: usize) -> Self {
        let count = worker_count.max(1);
        let capacity = queue_capacity_per_worker.max(1);
        let (result_tx, results) = mpsc::sync_channel(count * capacity);
        let queued = Arc::new(AtomicUsize::new(0));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicU64::new(0));
        let mut workers = Vec::new();
        for _ in 0..count {
            let (sender, receiver) = mpsc::sync_channel::<SaveJob>(capacity);
            let result_tx = result_tx.clone();
            let queued_count = queued.clone();
            let flight_count = in_flight.clone();
            let complete_count = completed.clone();
            let failed_count = failed.clone();
            let thread = thread::Builder::new()
                .name("world-save-worker".into())
                .spawn(move || {
                    while let Ok(job) = receiver.recv() {
                        flight_count.fetch_add(1, Ordering::Relaxed);
                        queued_count.fetch_sub(1, Ordering::Relaxed);
                        let result = job
                            .storage
                            .store_chunk_measured(&job.chunk)
                            .map_err(|error| error.to_string());
                        flight_count.fetch_sub(1, Ordering::Relaxed);
                        if result.is_ok() {
                            complete_count.fetch_add(1, Ordering::Relaxed);
                        } else {
                            failed_count.fetch_add(1, Ordering::Relaxed);
                        }
                        if result_tx
                            .send(SaveCompletion {
                                token: job.token,
                                result,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .expect("world save worker thread creation failed");
            workers.push(SaveWorker {
                sender: Some(sender),
                thread: Some(thread),
            });
        }
        drop(result_tx);
        Self {
            workers,
            results: Some(results),
            next_worker: 0,
            queued,
            in_flight,
            completed,
            failed,
        }
    }
    pub fn submit(
        &mut self,
        storage: WorldStorage,
        token: SaveToken,
        chunk: StoredChunk,
    ) -> Result<(), WorldError> {
        let index = self.next_worker;
        let job = SaveJob {
            storage,
            token,
            chunk,
        };
        self.queued.fetch_add(1, Ordering::Relaxed);
        match self.workers[index]
            .sender
            .as_ref()
            .expect("live save worker")
            .try_send(job)
        {
            Ok(()) => {
                self.next_worker = (index + 1) % self.workers.len();
                Ok(())
            }
            Err(TrySendError::Full(_)) => {
                self.queued.fetch_sub(1, Ordering::Relaxed);
                Err(WorldError::InvalidData("save queue full"))
            }
            Err(TrySendError::Disconnected(_)) => {
                self.queued.fetch_sub(1, Ordering::Relaxed);
                Err(WorldError::InvalidData("save worker stopped"))
            }
        }
    }
    pub fn take_completed(&mut self) -> Vec<SaveCompletion> {
        let mut result = Vec::new();
        while let Some(completion) = self.results.as_ref().and_then(|rx| rx.try_recv().ok()) {
            result.push(completion);
        }
        result
    }
    pub fn metrics(&self) -> SaveSchedulerMetrics {
        SaveSchedulerMetrics {
            queued: self.queued.load(Ordering::Relaxed),
            in_flight: self.in_flight.load(Ordering::Relaxed),
            completed: self.completed.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
        }
    }
}
impl Drop for SaveScheduler {
    fn drop(&mut self) {
        for worker in &mut self.workers {
            worker.sender.take();
        }
        self.results.take();
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SaveSchedulerMetrics {
    pub queued: usize,
    pub in_flight: usize,
    pub completed: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkPhase {
    Requested,
    Loading,
    Generating,
    Ready,
    Dirty,
    Saving,
    Evictable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidencyPhase {
    Requested,
    Loading,
    Generating,
    InitialLighting,
    Ready,
    Saving,
    Failed,
}

/// End-to-end streaming urgency. Membership geometry is Chebyshev/square; lookahead may only
/// reorder columns inside one class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResidencyUrgency {
    Current,
    SafeCore,
    Visible,
    Prefetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResidencyRequest {
    pub position: ChunkPos,
    pub token: u64,
}

#[derive(Debug, Default)]
pub struct ResidencyPlan {
    pub requests: Vec<ResidencyRequest>,
    pub evict: Vec<ChunkPos>,
    pub cancelled: Vec<ResidencyRequest>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ResidencyMotion {
    pub displacement: [f32; 2],
    pub elapsed_seconds: f32,
    pub view_direction: [f32; 2],
}

#[derive(Debug, Clone, Copy)]
struct ResidencyEntry {
    token: u64,
    phase: ResidencyPhase,
}

/// Generic interest-driven column residency policy. It computes requests and eviction candidates;
/// storage, generation, publication and save-before-evict remain owned by the caller.
#[derive(Debug)]
pub struct WorldResidency {
    load_radius: i32,
    retain_radius: i32,
    center: Option<ChunkPos>,
    motion_direction_x: f64,
    motion_direction_z: f64,
    motion_speed_blocks_per_second: f64,
    sustained_motion_seconds: f64,
    priority_direction_x: i64,
    priority_direction_z: i64,
    priority_lookahead_milli: i64,
    next_token: u64,
    entries: HashMap<ChunkPos, ResidencyEntry>,
}

impl WorldResidency {
    fn chebyshev_distance_from(&self, position: ChunkPos) -> i64 {
        let center = self.center.unwrap_or(ChunkPos { x: 0, z: 0 });
        let dx = i64::from(position.x) - i64::from(center.x);
        let dz = i64::from(position.z) - i64::from(center.z);
        dx.abs().max(dz.abs())
    }

    #[must_use]
    pub fn is_desired(&self, position: ChunkPos) -> bool {
        self.center.is_some()
            && self.chebyshev_distance_from(position) <= i64::from(self.load_radius)
    }

    #[must_use]
    pub fn is_retained_by_radius(&self, position: ChunkPos) -> bool {
        self.center.is_some()
            && self.chebyshev_distance_from(position) <= i64::from(self.retain_radius)
    }

    #[must_use]
    pub fn urgency(&self, position: ChunkPos) -> ResidencyUrgency {
        match self.chebyshev_distance_from(position) {
            0 => ResidencyUrgency::Current,
            1 => ResidencyUrgency::SafeCore,
            2 => ResidencyUrgency::Visible,
            _ => ResidencyUrgency::Prefetch,
        }
    }

    pub fn new(load_radius: i32, hysteresis: i32) -> Self {
        let load_radius = load_radius.max(1);
        Self {
            load_radius,
            retain_radius: load_radius + hysteresis.max(0),
            center: None,
            motion_direction_x: 0.0,
            motion_direction_z: 0.0,
            motion_speed_blocks_per_second: 0.0,
            sustained_motion_seconds: 0.0,
            priority_direction_x: 0,
            priority_direction_z: 0,
            priority_lookahead_milli: 0,
            next_token: 0,
            entries: HashMap::new(),
        }
    }

    /// Apply validated operational radii without losing generations, residency entries or pins.
    pub fn set_radii(&mut self, load: i32, retain: i32) -> Result<(), &'static str> {
        if !(1..=12).contains(&load) || retain < load || retain > 16 {
            return Err("invalid load/retain radii");
        }
        self.load_radius = load;
        self.retain_radius = retain;
        Ok(())
    }

    pub fn phase(&self, position: ChunkPos) -> Option<ResidencyPhase> {
        self.entries.get(&position).map(|entry| entry.phase)
    }

    pub fn set_phase(&mut self, request: ResidencyRequest, phase: ResidencyPhase) -> bool {
        let Some(entry) = self.entries.get_mut(&request.position) else {
            return false;
        };
        if entry.token != request.token {
            return false;
        }
        entry.phase = phase;
        true
    }

    #[must_use]
    pub fn is_current(&self, request: ResidencyRequest) -> bool {
        self.entries
            .get(&request.position)
            .is_some_and(|entry| entry.token == request.token)
    }

    pub fn request_retry(&mut self, position: ChunkPos) -> ResidencyRequest {
        self.next_token = self.next_token.wrapping_add(1).max(1);
        let request = ResidencyRequest {
            position,
            token: self.next_token,
        };
        self.entries.insert(
            position,
            ResidencyEntry {
                token: request.token,
                phase: ResidencyPhase::Requested,
            },
        );
        request
    }

    pub fn published(&mut self, position: ChunkPos, token: u64) -> bool {
        let request = ResidencyRequest { position, token };
        self.set_phase(request, ResidencyPhase::Ready)
    }

    pub fn defer(&mut self, request: ResidencyRequest) {
        if self.is_current(request) {
            self.entries.remove(&request.position);
        }
    }

    pub fn evicted(&mut self, position: ChunkPos) {
        self.entries.remove(&position);
    }

    pub fn update(
        &mut self,
        center: ChunkPos,
        resident: &HashSet<ChunkPos>,
        pinned: &HashSet<ChunkPos>,
    ) -> ResidencyPlan {
        self.update_with_motion(center, resident, pinned, 0.0, 0.0, 0.0)
    }

    /// Like [`Self::update`], but biases requests along sustained displacement. Lookahead is
    /// speed × time on a stable heading, capped at three columns; brief movement therefore cannot
    /// trigger a large prefetch, and a heading change restarts the duration.
    pub fn update_with_motion(
        &mut self,
        center: ChunkPos,
        resident: &HashSet<ChunkPos>,
        pinned: &HashSet<ChunkPos>,
        displacement_x: f32,
        displacement_z: f32,
        elapsed_seconds: f32,
    ) -> ResidencyPlan {
        self.update_with_motion_in_view(
            center,
            resident,
            pinned,
            ResidencyMotion {
                displacement: [displacement_x, displacement_z],
                elapsed_seconds,
                view_direction: [displacement_x, displacement_z],
            },
        )
    }

    /// Motion controls the bounded lookahead magnitude; the view direction determines which
    /// requested columns in that area are most useful to render next.
    pub fn update_with_motion_in_view(
        &mut self,
        center: ChunkPos,
        resident: &HashSet<ChunkPos>,
        pinned: &HashSet<ChunkPos>,
        motion: ResidencyMotion,
    ) -> ResidencyPlan {
        self.center = Some(center);
        let dt = if motion.elapsed_seconds.is_finite() {
            motion.elapsed_seconds.clamp(0.0, 1.0) as f64
        } else {
            0.0
        };
        let displacement_x = if motion.displacement[0].is_finite() {
            f64::from(motion.displacement[0].clamp(-8.0, 8.0))
        } else {
            0.0
        };
        let displacement_z = if motion.displacement[1].is_finite() {
            f64::from(motion.displacement[1].clamp(-8.0, 8.0))
        } else {
            0.0
        };
        let distance = displacement_x.hypot(displacement_z);
        if dt > 0.0 && distance > 0.01 {
            let direction_x = displacement_x / distance;
            let direction_z = displacement_z / distance;
            let observed_speed = (distance / dt).clamp(0.0, 32.0);
            let alignment =
                self.motion_direction_x * direction_x + self.motion_direction_z * direction_z;
            if self.sustained_motion_seconds == 0.0 || alignment < 0.707 {
                self.sustained_motion_seconds = dt;
                self.motion_speed_blocks_per_second = observed_speed;
            } else {
                self.sustained_motion_seconds = (self.sustained_motion_seconds + dt).min(12.0);
                self.motion_speed_blocks_per_second =
                    self.motion_speed_blocks_per_second * 0.75 + observed_speed * 0.25;
            }
            self.motion_direction_x = direction_x;
            self.motion_direction_z = direction_z;
        } else {
            self.sustained_motion_seconds = (self.sustained_motion_seconds - dt * 2.0).max(0.0);
        }
        let lookahead_columns =
            (self.motion_speed_blocks_per_second * self.sustained_motion_seconds / 16.0)
                .clamp(0.0, 3.0);
        let lookahead_milli = (lookahead_columns * 1024.0).round() as i64;
        let view_length =
            f64::from(motion.view_direction[0]).hypot(f64::from(motion.view_direction[1]));
        let (view_x, view_z) = if view_length.is_finite() && view_length > 1.0e-6 {
            (
                f64::from(motion.view_direction[0]) / view_length,
                f64::from(motion.view_direction[1]) / view_length,
            )
        } else {
            (self.motion_direction_x, self.motion_direction_z)
        };
        // Actual travel is the stronger predictor. The view direction still biases visible
        // terrain, but looking sideways cannot erase priority along the player's velocity.
        let blended_x = self.motion_direction_x * 0.7 + view_x * 0.3;
        let blended_z = self.motion_direction_z * 0.7 + view_z * 0.3;
        let blended_length = blended_x.hypot(blended_z);
        let (priority_direction_x, priority_direction_z) = if blended_length > 1.0e-6 {
            (blended_x / blended_length, blended_z / blended_length)
        } else {
            (self.motion_direction_x, self.motion_direction_z)
        };
        let direction_x = (priority_direction_x * 1024.0).round() as i64;
        let direction_z = (priority_direction_z * 1024.0).round() as i64;
        self.priority_direction_x = direction_x;
        self.priority_direction_z = direction_z;
        self.priority_lookahead_milli = lookahead_milli;
        let mut plan = ResidencyPlan::default();
        let in_radius = |position: ChunkPos, radius: i32| {
            let dx = i64::from(position.x) - i64::from(center.x);
            let dz = i64::from(position.z) - i64::from(center.z);
            dx.abs().max(dz.abs()) <= i64::from(radius)
        };

        let abandoned = self
            .entries
            .iter()
            .filter_map(|(position, entry)| {
                (!resident.contains(position)
                    && !pinned.contains(position)
                    && !in_radius(*position, self.retain_radius))
                .then_some(ResidencyRequest {
                    position: *position,
                    token: entry.token,
                })
            })
            .collect::<Vec<_>>();
        for request in abandoned {
            self.entries.remove(&request.position);
            plan.cancelled.push(request);
        }

        let radius = self.load_radius;
        for z in center.z.saturating_sub(radius)..=center.z.saturating_add(radius) {
            for x in center.x.saturating_sub(radius)..=center.x.saturating_add(radius) {
                let position = ChunkPos { x, z };
                if !in_radius(position, radius)
                    || resident.contains(&position)
                    || self.entries.contains_key(&position)
                {
                    continue;
                }
                plan.requests.push(self.request_retry(position));
            }
        }
        plan.requests
            .sort_by_key(|request| self.priority_key(request.position));
        plan.evict = resident
            .iter()
            .copied()
            .filter(|position| {
                !pinned.contains(position) && !in_radius(*position, self.retain_radius)
            })
            .collect();
        plan.evict.sort_by_key(|position| {
            let dx = i64::from(position.x) - i64::from(center.x);
            let dz = i64::from(position.z) - i64::from(center.z);
            std::cmp::Reverse(dx.abs().max(dz.abs()))
        });
        plan
    }

    pub fn retain_radius(&self) -> i32 {
        self.retain_radius
    }

    pub fn load_radius(&self) -> i32 {
        self.load_radius
    }

    pub fn motion_lookahead_columns(&self) -> f32 {
        (self.motion_speed_blocks_per_second * self.sustained_motion_seconds / 16.0).clamp(0.0, 3.0)
            as f32
    }

    /// Stable distance/forward score shared by residency submission and downstream world work.
    /// Lower keys are more urgent; the game remains responsible for choosing the interest point
    /// and movement/view signals that update this generic ordering.
    pub fn priority_key(&self, position: ChunkPos) -> (i64, i64, i32, i32) {
        let center = self.center.unwrap_or(ChunkPos { x: 0, z: 0 });
        let dx = i64::from(position.x) - i64::from(center.x);
        let dz = i64::from(position.z) - i64::from(center.z);
        let distance = dx.abs().max(dz.abs());
        let radial_distance = dx * dx + dz * dz;
        let forward_alignment = dx * self.priority_direction_x + dz * self.priority_direction_z;
        // Keep urgency classes dominant over lookahead. The local score only orders work within a
        // class, so sustained motion cannot make distant speculation outrank the contiguous core.
        let urgency = self.urgency(position) as i64;
        const URGENCY_STRIDE: i64 = 1_000_000_000_000;
        (
            urgency * URGENCY_STRIDE + distance * 1_048_576
                - forward_alignment * self.priority_lookahead_milli,
            radial_distance,
            position.x,
            position.z,
        )
    }

    pub fn desired_column_count(&self) -> usize {
        if self.center.is_none() {
            return 0;
        }
        let r = self.load_radius;
        let width = r.saturating_mul(2).saturating_add(1) as usize;
        width.saturating_mul(width)
    }

    pub fn pending_column_count(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| {
                matches!(
                    entry.phase,
                    ResidencyPhase::Requested
                        | ResidencyPhase::Loading
                        | ResidencyPhase::Generating
                )
            })
            .count()
    }
}

#[cfg(test)]
mod residency_tests {
    use super::*;

    #[test]
    fn requests_nearby_first_and_uses_negative_coordinates_with_hysteresis() {
        let mut residency = WorldResidency::new(1, 1);
        let center = ChunkPos { x: -4, z: 7 };
        let resident = HashSet::from([center]);
        let plan = residency.update(center, &resident, &HashSet::new());
        assert_eq!(plan.requests.len(), 8);
        assert!(plan.requests.iter().all(|request| {
            (request.position.x - center.x).abs() <= 1 && (request.position.z - center.z).abs() <= 1
        }));
        assert_eq!(residency.desired_column_count(), 9);
        assert_eq!(residency.retain_radius(), 2);
        let just_outside_load =
            residency.update(ChunkPos { x: -2, z: 7 }, &resident, &HashSet::new());
        assert!(!just_outside_load.evict.contains(&center));
    }

    #[test]
    fn desired_and_retained_membership_are_complete_chebyshev_squares() {
        for (radius, expected) in [(3, 49), (4, 81), (5, 121), (6, 169)] {
            let center = ChunkPos { x: -7, z: 11 };
            let mut residency = WorldResidency::new(radius, 1);
            let plan = residency.update(center, &HashSet::new(), &HashSet::new());
            assert_eq!(residency.desired_column_count(), expected);
            assert_eq!(plan.requests.len(), expected);
            assert!(residency.is_desired(ChunkPos {
                x: center.x + radius,
                z: center.z + radius,
            }));
            assert!(!residency.is_desired(ChunkPos {
                x: center.x + radius + 1,
                z: center.z,
            }));
            assert!(residency.is_retained_by_radius(ChunkPos {
                x: center.x + radius + 1,
                z: center.z + radius + 1,
            }));
        }
    }

    #[test]
    fn short_displacement_only_breaks_near_distance_ties() {
        let mut residency = WorldResidency::new(2, 1);
        let center = ChunkPos { x: -4, z: 7 };
        let resident = HashSet::from([center]);
        let plan =
            residency.update_with_motion(center, &resident, &HashSet::new(), 0.0, -0.25, 0.05);
        assert!(residency.motion_lookahead_columns() < 0.1);

        let first_ring = plan
            .requests
            .iter()
            .filter(|request| {
                let dx = request.position.x - center.x;
                let dz = request.position.z - center.z;
                dx * dx + dz * dz == 1
            })
            .map(|request| request.position)
            .collect::<Vec<_>>();
        assert_eq!(first_ring[0], ChunkPos { x: -4, z: 6 });
        assert_eq!(first_ring[1], ChunkPos { x: -5, z: 7 });
        assert_eq!(first_ring[2], ChunkPos { x: -3, z: 7 });
        assert_eq!(first_ring[3], ChunkPos { x: -4, z: 8 });
    }

    #[test]
    fn sustained_motion_prioritizes_a_bounded_lead_ahead_of_the_player() {
        let mut residency = WorldResidency::new(4, 1);
        let center = ChunkPos { x: 3, z: -2 };
        let resident = HashSet::from([center]);
        let settled_area = (-4..=4)
            .flat_map(|dz| (-4..=4).map(move |dx| (dx, dz)))
            .filter(|(dx, dz)| dx * dx + dz * dz <= 16)
            .map(|(dx, dz)| ChunkPos {
                x: center.x + dx,
                z: center.z + dz,
            })
            .collect::<HashSet<_>>();
        for _ in 0..80 {
            let _ = residency.update_with_motion(
                center,
                &settled_area,
                &HashSet::new(),
                0.7,
                0.0,
                0.05,
            );
        }
        assert!(residency.motion_lookahead_columns() > 1.8);
        assert!(residency.motion_lookahead_columns() <= 3.0);
        let plan = residency.update_with_motion(center, &resident, &HashSet::new(), 0.0, 0.0, 0.05);
        let first_prefetch = plan
            .requests
            .iter()
            .find(|request| {
                let dx = request.position.x - center.x;
                let dz = request.position.z - center.z;
                dx * dx + dz * dz > 2
            })
            .expect("the load radius includes a visible/prefetch ring");
        assert_eq!(first_prefetch.position, ChunkPos { x: 5, z: -2 });
        for request in &plan.requests {
            residency.defer(*request);
        }

        let turned =
            residency.update_with_motion(center, &resident, &HashSet::new(), 0.0, 0.7, 0.05);
        assert!(residency.motion_lookahead_columns() < 0.1);
        assert_eq!(turned.requests[0].position, ChunkPos { x: 3, z: -1 });
        for request in &turned.requests {
            residency.defer(*request);
        }

        let reversed =
            residency.update_with_motion(center, &resident, &HashSet::new(), -0.7, 0.0, 0.05);
        assert!(residency.motion_lookahead_columns() < 0.1);
        assert_eq!(reversed.requests[0].position, ChunkPos { x: 2, z: -2 });

        for _ in 0..120 {
            let _ =
                residency.update_with_motion(center, &resident, &HashSet::new(), 0.0, 0.0, 0.05);
        }
        assert!(residency.motion_lookahead_columns() < 0.1);
    }

    #[test]
    fn urgency_classes_keep_required_and_visible_ahead_of_prefetch() {
        let mut residency = WorldResidency::new(6, 1);
        let center = ChunkPos { x: 0, z: 0 };
        let resident = HashSet::from([center]);
        for _ in 0..240 {
            let _ =
                residency.update_with_motion(center, &resident, &HashSet::new(), 1.6, 0.0, 0.05);
        }
        let required = residency.priority_key(ChunkPos { x: 1, z: 0 });
        let visible = residency.priority_key(ChunkPos { x: 2, z: 0 });
        let prefetch = residency.priority_key(ChunkPos { x: 5, z: 0 });
        assert!(required < visible);
        assert!(visible < prefetch);
    }

    #[test]
    fn sustained_motion_blends_camera_view_without_losing_travel_priority() {
        let center = ChunkPos { x: 0, z: 0 };
        let resident = HashSet::new();
        let mut residency = WorldResidency::new(4, 1);

        for _ in 0..60 {
            let plan = residency.update_with_motion_in_view(
                center,
                &resident,
                &HashSet::new(),
                ResidencyMotion {
                    displacement: [0.0, 0.7],
                    elapsed_seconds: 0.05,
                    view_direction: [1.0, 0.0],
                },
            );
            for request in plan.requests {
                residency.defer(request);
            }
        }

        let plan = residency.update_with_motion_in_view(
            center,
            &resident,
            &HashSet::new(),
            ResidencyMotion {
                displacement: [0.0, 0.0],
                elapsed_seconds: 0.05,
                view_direction: [1.0, 0.0],
            },
        );
        let east = plan
            .requests
            .iter()
            .position(|request| request.position == ChunkPos { x: 1, z: 0 })
            .unwrap();
        let north = plan
            .requests
            .iter()
            .position(|request| request.position == ChunkPos { x: 0, z: 1 })
            .unwrap();
        let west = plan
            .requests
            .iter()
            .position(|request| request.position == ChunkPos { x: -1, z: 0 })
            .unwrap();
        let south = plan
            .requests
            .iter()
            .position(|request| request.position == ChunkPos { x: 0, z: -1 })
            .unwrap();
        assert!(north < east, "travel direction remains the stronger signal");
        assert!(
            east < west && east < south,
            "camera view still biases visible columns"
        );
    }

    #[test]
    fn update_without_direction_keeps_requests_deterministically_ordered() {
        let center = ChunkPos { x: 0, z: 0 };
        let resident = HashSet::from([center]);
        let mut a = WorldResidency::new(2, 1);
        let mut b = WorldResidency::new(2, 1);
        let a = a.update(center, &resident, &HashSet::new());
        let b = b.update(center, &resident, &HashSet::new());
        assert_eq!(
            a.requests.iter().map(|r| r.position).collect::<Vec<_>>(),
            b.requests.iter().map(|r| r.position).collect::<Vec<_>>()
        );
    }

    #[test]
    fn teleport_invalidates_old_requests_and_rejects_late_results() {
        let mut residency = WorldResidency::new(1, 1);
        let old = residency.update(ChunkPos { x: 0, z: 0 }, &HashSet::new(), &HashSet::new());
        let stale = old.requests[0];
        let new = residency.update(
            ChunkPos { x: 100, z: -100 },
            &HashSet::new(),
            &HashSet::new(),
        );
        assert!(!residency.is_current(stale));
        assert!(new.cancelled.contains(&stale));
        assert!(new.requests.iter().all(|request| {
            (request.position.x - 100).abs() <= 1 && (request.position.z + 100).abs() <= 1
        }));
    }

    #[test]
    fn resident_columns_outside_retain_radius_are_eviction_candidates_unless_pinned() {
        let mut residency = WorldResidency::new(2, 1);
        let center = ChunkPos { x: 0, z: 0 };
        let old = ChunkPos { x: -8, z: 0 };
        let pinned = ChunkPos { x: 0, z: 8 };
        let resident = HashSet::from([center, old, pinned]);
        let plan = residency.update(center, &resident, &HashSet::from([pinned]));
        assert!(plan.evict.contains(&old));
        assert!(!plan.evict.contains(&center));
        assert!(!plan.evict.contains(&pinned));
    }

    #[test]
    fn long_path_keeps_desired_resident_area_bounded() {
        let mut residency = WorldResidency::new(2, 1);
        let mut resident = HashSet::new();
        for x in -80..=80 {
            let center = ChunkPos { x, z: -x / 2 };
            let plan = residency.update(center, &resident, &HashSet::new());
            resident.extend(plan.requests.iter().map(|request| request.position));
            resident.retain(|position| {
                (position.x - center.x).abs() <= residency.retain_radius()
                    && (position.z - center.z).abs() <= residency.retain_radius()
            });
            assert!(
                resident.len() <= 49,
                "resident columns grew to {}",
                resident.len()
            );
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SaveToken {
    pub position: ChunkPos,
    pub generation: u64,
}

/// Persistence dirty generations are intentionally independent from renderer dirty sections.
/// Finishing an old save never marks a newer mutation clean; a failed save stays retryable.
#[derive(Debug, Default)]
pub struct PersistenceDirtyTracker {
    dirty: BTreeMap<(i32, i32), u64>,
    saving: HashSet<(i32, i32)>,
    next_generation: u64,
    saved: u64,
    failed: u64,
}

impl PersistenceDirtyTracker {
    pub fn mark_dirty(&mut self, position: ChunkPos) -> u64 {
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        self.dirty
            .insert((position.x, position.z), self.next_generation);
        self.next_generation
    }
    pub fn begin_save(&mut self, position: ChunkPos) -> Option<SaveToken> {
        let key = (position.x, position.z);
        let generation = *self.dirty.get(&key)?;
        if !self.saving.insert(key) {
            return None;
        }
        Some(SaveToken {
            position,
            generation,
        })
    }
    pub fn complete_save(&mut self, token: SaveToken, success: bool) {
        let key = (token.position.x, token.position.z);
        self.saving.remove(&key);
        if success {
            self.saved += 1;
            if self.dirty.get(&key) == Some(&token.generation) {
                self.dirty.remove(&key);
            }
        } else {
            self.failed += 1;
        }
    }
    /// Bounded immutable tooling view. Dirty generations are not persisted schema revisions.
    pub fn diagnostic_entries(
        &self,
        limit: usize,
    ) -> impl Iterator<Item = (ChunkPos, u64, bool)> + '_ {
        self.dirty
            .iter()
            .take(limit.min(32))
            .map(|(&(x, z), &generation)| {
                (ChunkPos { x, z }, generation, self.saving.contains(&(x, z)))
            })
    }
    pub fn dirty_count(&self) -> usize {
        self.dirty.len()
    }
    #[must_use]
    pub fn is_dirty(&self, position: ChunkPos) -> bool {
        self.dirty.contains_key(&(position.x, position.z))
    }
    #[must_use]
    pub fn is_saving(&self, position: ChunkPos) -> bool {
        self.saving.contains(&(position.x, position.z))
    }
    pub fn queued(&self, limit: usize) -> Vec<ChunkPos> {
        self.dirty
            .keys()
            .take(limit)
            .map(|(x, z)| ChunkPos { x: *x, z: *z })
            .collect()
    }
    pub fn metrics(&self) -> SaveMetrics {
        SaveMetrics {
            dirty_chunks: self.dirty.len(),
            saves_in_flight: self.saving.len(),
            saved: self.saved,
            failed: self.failed,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SaveMetrics {
    pub dirty_chunks: usize,
    pub saves_in_flight: usize,
    pub saved: u64,
    pub failed: u64,
}

/// Small state tracker for lifecycle observability. Chunk bytes are still published atomically
/// through `World::publish_column`; only Ready/Dirty/Evictable states are externally usable.
#[derive(Debug, Default)]
pub struct ChunkLifecycle {
    states: HashMap<ChunkPos, ChunkPhase>,
}

impl ChunkLifecycle {
    pub fn phase(&self, position: ChunkPos) -> Option<ChunkPhase> {
        self.states.get(&position).copied()
    }
    pub fn transition(&mut self, position: ChunkPos, next: ChunkPhase) -> Result<(), WorldError> {
        let current = self.states.get(&position).copied();
        let valid = matches!(
            (current, next),
            (None, ChunkPhase::Requested)
                | (
                    Some(ChunkPhase::Requested),
                    ChunkPhase::Loading | ChunkPhase::Generating
                )
                | (
                    Some(ChunkPhase::Loading | ChunkPhase::Generating),
                    ChunkPhase::Ready
                )
                | (
                    Some(ChunkPhase::Ready),
                    ChunkPhase::Dirty | ChunkPhase::Saving | ChunkPhase::Evictable
                )
                | (
                    Some(ChunkPhase::Dirty),
                    ChunkPhase::Saving | ChunkPhase::Evictable
                )
                | (
                    Some(ChunkPhase::Saving),
                    ChunkPhase::Ready | ChunkPhase::Dirty | ChunkPhase::Evictable
                )
                | (Some(ChunkPhase::Evictable), ChunkPhase::Requested)
        );
        if !valid {
            return Err(WorldError::InvalidData(
                "invalid chunk lifecycle transition",
            ));
        }
        if next == ChunkPhase::Requested && current == Some(ChunkPhase::Evictable) {
            self.states.remove(&position);
        } else {
            self.states.insert(position, next);
        }
        Ok(())
    }
}

impl GenerationScheduler {
    pub fn new(worker_count: usize, queue_capacity_per_worker: usize) -> Self {
        let count = worker_count.max(1);
        let (result_tx, results) = mpsc::sync_channel(count * queue_capacity_per_worker.max(1));
        let pending = Arc::new(AtomicUsize::new(0));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicU64::new(0));
        let generation_micros = Arc::new(AtomicU64::new(0));
        let mut workers = Vec::with_capacity(count);
        for _ in 0..count {
            let (sender, receiver) =
                mpsc::sync_channel::<GenerationJob>(queue_capacity_per_worker.max(1));
            let result_tx = result_tx.clone();
            let pending_count = pending.clone();
            let flight_count = in_flight.clone();
            let complete_count = completed.clone();
            let time_count = generation_micros.clone();
            let thread = thread::Builder::new()
                .name("worldgen-worker".into())
                .spawn(move || {
                    while let Ok(job) = receiver.recv() {
                        flight_count.fetch_add(1, Ordering::Relaxed);
                        pending_count.fetch_sub(1, Ordering::Relaxed);
                        let queue_wait_ms = job.queued_at.elapsed().as_secs_f64() * 1000.0;
                        let started = std::time::Instant::now();
                        let sections = job.generator.generate(job.seed, job.position);
                        let elapsed = started.elapsed();
                        time_count.fetch_add(
                            elapsed.as_micros().min(u128::from(u64::MAX)) as u64,
                            Ordering::Relaxed,
                        );
                        flight_count.fetch_sub(1, Ordering::Relaxed);
                        complete_count.fetch_add(1, Ordering::Relaxed);
                        if result_tx
                            .send(GeneratedColumn {
                                position: job.position,
                                generation: job.generation,
                                queue_wait_ms,
                                generation_ms: elapsed.as_secs_f64() * 1000.0,
                                sections,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .expect("world generation worker thread creation failed");
            workers.push(Worker {
                sender: Some(sender),
                thread: Some(thread),
            });
        }
        drop(result_tx);
        Self {
            workers,
            results: Some(results),
            latest: HashMap::new(),
            outstanding: HashSet::new(),
            next_worker: 0,
            submitted: 0,
            coalesced: 0,
            stale_discarded: 0,
            pending,
            in_flight,
            completed,
            generation_micros,
        }
    }

    pub fn request(
        &mut self,
        generator: Arc<dyn ChunkGenerator>,
        seed: i64,
        position: ChunkPos,
        generation: u64,
    ) -> Result<bool, WorldError> {
        let key = (position, generation);
        if self.outstanding.contains(&key) {
            self.coalesced += 1;
            return Ok(false);
        }
        if self
            .latest
            .get(&position)
            .is_some_and(|latest| generation < *latest)
        {
            self.stale_discarded += 1;
            return Ok(false);
        }
        let worker_index = self.next_worker;
        let job = GenerationJob {
            generator,
            seed,
            position,
            generation,
            queued_at: std::time::Instant::now(),
        };
        self.pending.fetch_add(1, Ordering::Relaxed);
        match self.workers[worker_index]
            .sender
            .as_ref()
            .expect("live worker sender")
            .try_send(job)
        {
            Ok(()) => {
                self.next_worker = (worker_index + 1) % self.workers.len();
                self.outstanding.insert(key);
                self.latest
                    .entry(position)
                    .and_modify(|latest| *latest = (*latest).max(generation))
                    .or_insert(generation);
                self.submitted += 1;
                Ok(true)
            }
            Err(TrySendError::Full(_)) => {
                self.pending.fetch_sub(1, Ordering::Relaxed);
                Err(WorldError::InvalidData("generation queue full"))
            }
            Err(TrySendError::Disconnected(_)) => {
                self.pending.fetch_sub(1, Ordering::Relaxed);
                Err(WorldError::InvalidData("generation worker stopped"))
            }
        }
    }

    /// Drain currently completed work, suppressing any result older than the authoritative token.
    pub fn take_ready(&mut self) -> Vec<GeneratedColumn> {
        self.take_ready_limit(usize::MAX)
    }

    /// Drain at most `limit` accepted results, leaving the rest queued for later publication.
    pub fn take_ready_limit(&mut self, limit: usize) -> Vec<GeneratedColumn> {
        let mut ready = Vec::with_capacity(limit.min(self.outstanding.len()));
        while ready.len() < limit {
            let Some(result) = self.results.as_ref().and_then(|rx| rx.try_recv().ok()) else {
                break;
            };
            self.outstanding
                .remove(&(result.position, result.generation));
            if self.latest.get(&result.position).copied() == Some(result.generation) {
                ready.push(result);
            } else {
                self.stale_discarded += 1;
            }
        }
        ready
    }

    pub fn metrics(&self) -> GenerationMetrics {
        GenerationMetrics {
            submitted: self.submitted,
            coalesced: self.coalesced,
            completed: self.completed.load(Ordering::Relaxed),
            stale_discarded: self.stale_discarded,
            pending: self.pending.load(Ordering::Relaxed),
            in_flight: self.in_flight.load(Ordering::Relaxed),
            generation_total_ms: self.generation_micros.load(Ordering::Relaxed) as f64 / 1000.0,
        }
    }
}

impl Drop for GenerationScheduler {
    fn drop(&mut self) {
        for worker in &mut self.workers {
            worker.sender.take();
        }
        self.results.take();
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorldStorage {
    root: PathBuf,
}

impl WorldStorage {
    pub fn open(saves_root: impl AsRef<Path>, name: &str) -> Result<Self, WorldError> {
        if name.is_empty()
            || name.len() > 64
            || name == "."
            || name == ".."
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err(WorldError::InvalidWorldName);
        }
        let root = saves_root.as_ref().join(name);
        if fs::symlink_metadata(&root).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(WorldError::InvalidWorldName);
        }
        fs::create_dir_all(root.join("chunks"))?;
        if fs::symlink_metadata(root.join("chunks")).is_ok_and(|meta| meta.file_type().is_symlink())
        {
            return Err(WorldError::InvalidWorldName);
        }
        if let Some(parent) = root.parent() {
            sync_directory(parent)?;
        }
        sync_directory(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn has_metadata(&self) -> bool {
        self.root.join("world.rcw").is_file()
    }

    pub fn load_player(&self, player_id: &str) -> Result<Option<PlayerRecord>, WorldError> {
        validate_player_id(player_id)?;
        let directory = self.root.join("players");
        let directory_metadata = match fs::symlink_metadata(&directory) {
            Ok(meta) => meta,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(WorldError::Io(error)),
        };
        if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
            return Err(WorldError::InvalidWorldName);
        }
        let slots = [
            self.player_slot_path(player_id, 0),
            self.player_slot_path(player_id, 1),
        ];
        let mut records = Vec::new();
        let mut had_invalid = false;
        let mut last_error = None;
        for path in &slots {
            match read_player_checkpoint(path, player_id) {
                Ok(Some(record)) => records.push(record),
                Ok(None) => {}
                Err(error) => {
                    had_invalid = true;
                    last_error = Some(error);
                }
            }
        }
        if !records.is_empty() {
            records.sort_by_key(|record| record.revision);
            let mut newest = records.pop().unwrap();
            newest.recovered_from_checkpoint = had_invalid;
            return Ok(Some(newest));
        }
        if had_invalid {
            if let Some(mut legacy) = read_player_legacy(&self.player_path(player_id), player_id)? {
                legacy.recovered_from_checkpoint = true;
                return Ok(Some(legacy));
            }
            return Err(last_error.unwrap_or(WorldError::InvalidData("no valid player checkpoint")));
        }

        // Pre-component M4 records used one atomically replaced `.rcp` file. Preserve their
        // payload as an opaque legacy component so the game codec can migrate it on next save.
        let legacy_path = self.player_path(player_id);
        match read_player_legacy(&legacy_path, player_id) {
            Ok(record) => Ok(record),
            Err(error) => Err(error),
        }
    }

    pub fn store_player(&self, record: &PlayerRecord) -> Result<(), WorldError> {
        self.store_player_measured(record).map(|_| ())
    }

    pub fn store_player_measured(
        &self,
        record: &PlayerRecord,
    ) -> Result<PlayerCheckpointIoMetrics, WorldError> {
        validate_player_id(&record.player_id)?;
        validate_player_components(&record.components)?;
        if record.revision == 0 {
            return Err(WorldError::InvalidData("player revision must be nonzero"));
        }
        let directory = self.root.join("players");
        let directory_existed = fs::symlink_metadata(&directory).is_ok();
        fs::create_dir_all(&directory)?;
        if fs::symlink_metadata(&directory).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(WorldError::InvalidWorldName);
        }
        if !directory_existed {
            sync_directory(&self.root)?;
        }
        let paths = [
            self.player_slot_path(&record.player_id, 0),
            self.player_slot_path(&record.player_id, 1),
        ];
        let valid = paths
            .iter()
            .map(|path| {
                read_player_checkpoint(path, &record.player_id)
                    .ok()
                    .flatten()
            })
            .collect::<Vec<_>>();
        let newest = valid
            .iter()
            .flatten()
            .max_by_key(|existing| existing.revision);
        let target = match newest {
            Some(existing) if existing.revision > record.revision => {
                return Ok(PlayerCheckpointIoMetrics::default());
            }
            Some(existing) if existing.revision == record.revision => {
                if existing.components == record.components {
                    return Ok(PlayerCheckpointIoMetrics::default());
                }
                return Err(WorldError::InvalidData("player revision content conflict"));
            }
            Some(existing) => {
                if valid[0]
                    .as_ref()
                    .is_some_and(|slot| slot.revision == existing.revision)
                {
                    1
                } else {
                    0
                }
            }
            None => (record.revision & 1) as usize,
        };
        let mut out = Vec::new();
        out.extend_from_slice(PLAYER_MAGIC);
        put_u32(&mut out, PLAYER_RECORD_VERSION);
        put_string(&mut out, &record.player_id)?;
        put_u64(&mut out, record.revision);
        put_u16(&mut out, record.components.len() as u16);
        for component in &record.components {
            put_string(&mut out, &component.id)?;
            put_u32(&mut out, component.schema_version);
            put_u32(&mut out, component.payload.len() as u32);
            out.extend_from_slice(&component.payload);
        }
        finish_checksum(&mut out);
        if out.len() > MAX_PLAYER_RECORD_BYTES {
            return Err(WorldError::InvalidData("player record limit"));
        }
        let (write_ms, sync_ms) = durable_write_slot(&paths[target], &out)?;
        Ok(PlayerCheckpointIoMetrics {
            encoded_bytes: out.len(),
            write_ms,
            sync_ms,
        })
    }

    fn player_path(&self, player_id: &str) -> PathBuf {
        self.root.join("players").join(format!("{player_id}.rcp"))
    }

    fn player_slot_path(&self, player_id: &str, slot: u8) -> PathBuf {
        self.root
            .join("players")
            .join(format!("{player_id}.{slot}.rcp"))
    }

    pub fn load_world_state(&self) -> Result<Option<WorldStateRecord>, WorldError> {
        let paths = [self.world_state_path(0), self.world_state_path(1)];
        let mut records = Vec::new();
        let mut had_invalid = false;
        let mut last_error = None;
        for path in &paths {
            match read_world_state_checkpoint(path) {
                Ok(Some(record)) => records.push(record),
                Ok(None) => {}
                Err(error) => {
                    had_invalid = true;
                    last_error = Some(error);
                }
            }
        }
        if !records.is_empty() {
            records.sort_by_key(|record| record.revision);
            let mut newest = records.pop().expect("non-empty world state records");
            newest.recovered_from_checkpoint = had_invalid;
            return Ok(Some(newest));
        }
        if had_invalid {
            return Err(
                last_error.unwrap_or(WorldError::InvalidData("no valid world-state checkpoint"))
            );
        }
        Ok(None)
    }

    pub fn store_world_state(&self, record: &WorldStateRecord) -> Result<(), WorldError> {
        self.store_world_state_measured(record).map(|_| ())
    }

    pub fn store_world_state_measured(
        &self,
        record: &WorldStateRecord,
    ) -> Result<WorldStateCheckpointIoMetrics, WorldError> {
        validate_world_components(&record.components)?;
        if record.revision == 0 {
            return Err(WorldError::InvalidData(
                "world-state revision must be nonzero",
            ));
        }
        let paths = [self.world_state_path(0), self.world_state_path(1)];
        let valid = paths
            .iter()
            .map(|path| read_world_state_checkpoint(path).ok().flatten())
            .collect::<Vec<_>>();
        let newest = valid
            .iter()
            .flatten()
            .max_by_key(|existing| existing.revision);
        let target = match newest {
            Some(existing) if existing.revision > record.revision => {
                return Ok(WorldStateCheckpointIoMetrics::default());
            }
            Some(existing) if existing.revision == record.revision => {
                if existing.components == record.components {
                    return Ok(WorldStateCheckpointIoMetrics::default());
                }
                return Err(WorldError::InvalidData(
                    "world-state revision content conflict",
                ));
            }
            Some(existing) => {
                if valid[0]
                    .as_ref()
                    .is_some_and(|slot| slot.revision == existing.revision)
                {
                    1
                } else {
                    0
                }
            }
            None => (record.revision & 1) as usize,
        };
        let mut out = Vec::new();
        out.extend_from_slice(WORLD_STATE_MAGIC);
        put_u32(&mut out, WORLD_STATE_RECORD_VERSION);
        put_u64(&mut out, record.revision);
        put_u16(&mut out, record.components.len() as u16);
        for component in &record.components {
            put_string(&mut out, &component.id)?;
            put_u32(&mut out, component.schema_version);
            put_u32(&mut out, component.payload.len() as u32);
            out.extend_from_slice(&component.payload);
        }
        finish_checksum(&mut out);
        if out.len() > MAX_WORLD_STATE_BYTES {
            return Err(WorldError::InvalidData("world-state record limit"));
        }
        let (write_ms, sync_ms) = durable_write_slot(&paths[target], &out)?;
        Ok(WorldStateCheckpointIoMetrics {
            encoded_bytes: out.len(),
            write_ms,
            sync_ms,
        })
    }

    fn world_state_path(&self, slot: u8) -> PathBuf {
        self.root.join(format!("world-state.{slot}.rcs"))
    }

    pub fn store_metadata(&self, metadata: &WorldMetadata) -> Result<(), WorldError> {
        let mut out = Vec::new();
        out.extend_from_slice(WORLD_MAGIC);
        put_u32(&mut out, WORLD_FORMAT_VERSION);
        put_u32(&mut out, WORLD_METADATA_VERSION);
        put_i64(&mut out, metadata.seed);
        put_string(&mut out, &metadata.game_id)?;
        put_string(&mut out, &metadata.profile_fingerprint)?;
        put_u32(&mut out, metadata.persistence_schema_version);
        put_string(&mut out, &metadata.generator_id)?;
        put_u32(&mut out, metadata.generator_version);
        finish_checksum(&mut out);
        if out.len() > MAX_METADATA_BYTES {
            return Err(WorldError::InvalidData("metadata limit"));
        }
        for identity in [&metadata.game_id, &metadata.generator_id] {
            if !valid_semantic_key(identity) {
                return Err(WorldError::InvalidData("invalid metadata identity"));
            }
        }
        atomic_write(&self.root.join("world.rcw"), &out)
    }

    pub fn load_metadata(&self) -> Result<WorldMetadata, WorldError> {
        let data = read_limited(&self.root.join("world.rcw"), MAX_METADATA_BYTES)?;
        let body = verify_file(&data, WORLD_MAGIC)?;
        let mut r = Reader::new(body);
        let version = r.u32()?;
        if version != WORLD_FORMAT_VERSION {
            return Err(WorldError::UnsupportedVersion(version));
        }
        let metadata_version = r.u32()?;
        if metadata_version != 1 && metadata_version != WORLD_METADATA_VERSION {
            return Err(WorldError::UnsupportedVersion(metadata_version));
        }
        let seed = r.i64()?;
        let game_id = r.string()?;
        let profile_fingerprint = r.string()?;
        // Metadata v1 predates explicit persistence-schema identity. Its chunk payload version
        // fixes the semantic key + u16 variant representation, so migrate it as schema v1.
        let persistence_schema_version = if metadata_version == 1 { 1 } else { r.u32()? };
        let result = WorldMetadata {
            seed,
            game_id,
            profile_fingerprint,
            persistence_schema_version,
            generator_id: r.string()?,
            generator_version: r.u32()?,
        };
        for identity in [&result.game_id, &result.generator_id] {
            if !valid_semantic_key(identity) {
                return Err(WorldError::InvalidData("invalid metadata identity"));
            }
        }
        r.finish()?;
        Ok(result)
    }

    pub fn store_chunk(&self, chunk: &StoredChunk) -> Result<(), WorldError> {
        let (bytes, _) = encode_chunk_file(chunk)?;
        atomic_write(&self.chunk_path(chunk.position), &bytes)
    }

    pub fn store_chunk_measured(
        &self,
        chunk: &StoredChunk,
    ) -> Result<ChunkEncodingMetrics, WorldError> {
        let (bytes, metrics) = encode_chunk_file(chunk)?;
        atomic_write(&self.chunk_path(chunk.position), &bytes)?;
        Ok(metrics)
    }

    pub fn load_chunk(&self, position: ChunkPos) -> Result<StoredChunk, WorldError> {
        let data = read_limited(&self.chunk_path(position), MAX_CHUNK_BYTES)?;
        let body = verify_file(&data, CHUNK_MAGIC)?;
        let mut header = Reader::new(body);
        let version = header.u32()?;
        if version != LEGACY_CHUNK_FORMAT_VERSION && version != CHUNK_FORMAT_VERSION {
            return Err(WorldError::UnsupportedVersion(version));
        }
        let actual = ChunkPos {
            x: header.i32()?,
            z: header.i32()?,
        };
        if actual != position {
            return Err(WorldError::InvalidData("chunk coordinate mismatch"));
        }
        let count = header.u16()? as usize;
        if count > MAX_SECTIONS {
            return Err(WorldError::InvalidData("too many sections"));
        }
        let method = header.u8()?;
        let raw_len = header.u32()? as usize;
        if raw_len > MAX_CHUNK_BYTES {
            return Err(WorldError::InvalidData("decoded chunk limit"));
        }
        let stored_payload = header.remaining();
        let decoded = match method {
            0 if stored_payload.len() == raw_len => stored_payload.to_vec(),
            1 => {
                let mut decoder = flate2::read::ZlibDecoder::new(stored_payload);
                let mut output = Vec::with_capacity(raw_len.min(MAX_CHUNK_BYTES));
                decoder
                    .by_ref()
                    .take(raw_len as u64 + 1)
                    .read_to_end(&mut output)?;
                if output.len() != raw_len || decoder.total_in() as usize != stored_payload.len() {
                    return Err(WorldError::InvalidData("compressed length mismatch"));
                }
                output
            }
            _ => return Err(WorldError::InvalidData("unknown compression method")),
        };
        let mut r = Reader::new(&decoded);
        let mut sections = Vec::with_capacity(count);
        let mut section_ys = HashSet::with_capacity(count);
        for _ in 0..count {
            let y = r.i32()?;
            if !section_ys.insert(y) {
                return Err(WorldError::InvalidData("duplicate section"));
            }
            let palette_len = r.u16()? as usize;
            if palette_len == 0 || palette_len > MAX_PALETTE {
                return Err(WorldError::InvalidData("palette size"));
            }
            let mut palette = Vec::with_capacity(palette_len);
            for _ in 0..palette_len {
                let key = r.string()?;
                let variant = r.u16()?;
                if !valid_semantic_key(&key)
                    || palette.iter().any(|(old, old_variant): &(String, u16)| {
                        old == &key && *old_variant == variant
                    })
                {
                    return Err(WorldError::InvalidData("invalid or duplicate semantic key"));
                }
                palette.push((key, variant));
            }
            let mut states = Vec::with_capacity(CHUNK_VOLUME);
            for _ in 0..CHUNK_VOLUME {
                let index = r.u16()? as usize;
                states.push(
                    palette
                        .get(index)
                        .cloned()
                        .ok_or(WorldError::InvalidData("palette index out of range"))?,
                );
            }
            sections.push(StoredSection { y, states });
        }
        let (spatial_records, spatial_tombstones) = if version == LEGACY_CHUNK_FORMAT_VERSION {
            (Vec::new(), Vec::new())
        } else {
            let record_count = r.u16()? as usize;
            if record_count > MAX_SPATIAL_RECORDS {
                return Err(WorldError::InvalidData("spatial entity count limit"));
            }
            let mut records = Vec::with_capacity(record_count);
            let mut ids = HashSet::with_capacity(record_count);
            for _ in 0..record_count {
                let entity_id = EntityId(u128::from_le_bytes(
                    r.take(16)?
                        .try_into()
                        .map_err(|_| WorldError::InvalidData("invalid entity id"))?,
                ));
                if entity_id == EntityId::NIL || !ids.insert(entity_id) {
                    return Err(WorldError::InvalidData(
                        "zero or duplicate spatial entity id",
                    ));
                }
                let entity_revision = r.u64()?;
                if entity_revision == 0 {
                    return Err(WorldError::InvalidData("zero spatial entity revision"));
                }
                let entity_type = r.string()?;
                if entity_type.len() > MAX_SPATIAL_TYPE_ID_BYTES
                    || !valid_semantic_key(&entity_type)
                {
                    return Err(WorldError::InvalidData("invalid spatial entity type"));
                }
                let schema_version = r.u32()?;
                let payload_len = r.u32()? as usize;
                if payload_len > MAX_SPATIAL_PAYLOAD_BYTES {
                    return Err(WorldError::InvalidData("spatial entity payload limit"));
                }
                records.push(SpatialRecord {
                    entity_id,
                    entity_revision,
                    entity_type,
                    schema_version,
                    payload: r.take(payload_len)?.to_vec(),
                });
            }
            let tombstone_count = r.u16()? as usize;
            if tombstone_count > MAX_SPATIAL_TOMBSTONES {
                return Err(WorldError::InvalidData("spatial tombstone count limit"));
            }
            let mut tombstones = Vec::with_capacity(tombstone_count);
            let mut tombstone_ids = HashSet::with_capacity(tombstone_count);
            for _ in 0..tombstone_count {
                let entity_id =
                    EntityId(u128::from_le_bytes(r.take(16)?.try_into().map_err(
                        |_| WorldError::InvalidData("invalid tombstone entity id"),
                    )?));
                if entity_id == EntityId::NIL || !tombstone_ids.insert(entity_id) {
                    return Err(WorldError::InvalidData(
                        "zero or duplicate spatial tombstone id",
                    ));
                }
                let entity_revision = r.u64()?;
                if entity_revision == 0 {
                    return Err(WorldError::InvalidData("zero spatial tombstone revision"));
                }
                tombstones.push(SpatialTombstone {
                    entity_id,
                    entity_revision,
                    source: ChunkPos {
                        x: r.i32()?,
                        z: r.i32()?,
                    },
                });
            }
            (records, tombstones)
        };
        r.finish()?;
        Ok(StoredChunk {
            position,
            sections,
            spatial_records,
            spatial_tombstones,
        })
    }

    pub fn load_runtime_chunk(
        &self,
        position: ChunkPos,
        resolver: &(impl SemanticBlockResolver + ?Sized),
    ) -> Result<Vec<(i32, Chunk)>, WorldError> {
        self.load_runtime_column(position, resolver)
            .map(|column| column.sections)
    }

    pub fn load_runtime_column(
        &self,
        position: ChunkPos,
        resolver: &(impl SemanticBlockResolver + ?Sized),
    ) -> Result<LoadedColumn, WorldError> {
        let stored = self.load_chunk(position)?;
        let sections = stored
            .sections
            .into_iter()
            .map(|section| {
                let mut chunk = Chunk::new(BlockId(0));
                for (index, (key, variant)) in section.states.iter().enumerate() {
                    let state = resolver
                        .state_for(key, *variant)
                        .ok_or_else(|| WorldError::UnknownBlock(key.clone()))?;
                    chunk.set_state(
                        (
                            (index % 16) as u8,
                            (index / 256) as u8,
                            ((index / 16) % 16) as u8,
                        ),
                        state,
                    );
                }
                Ok((section.y, chunk))
            })
            .collect::<Result<Vec<_>, WorldError>>()?;
        Ok(LoadedColumn {
            sections,
            spatial_records: stored.spatial_records,
            spatial_tombstones: stored.spatial_tombstones,
        })
    }

    /// Load and resolve a stored column, returning `None` only for ordinary absence. Corruption,
    /// inaccessible files and unknown semantic content remain explicit errors.
    pub fn load_runtime_chunk_if_present(
        &self,
        position: ChunkPos,
        resolver: &dyn SemanticBlockResolver,
    ) -> Result<Option<Vec<(i32, Chunk)>>, WorldError> {
        let path = self.chunk_path(position);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(WorldError::Io(error)),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(WorldError::InvalidData("chunk path is not a regular file"));
        }
        self.load_runtime_chunk(position, resolver).map(Some)
    }

    pub fn load_runtime_column_if_present(
        &self,
        position: ChunkPos,
        resolver: &dyn SemanticBlockResolver,
    ) -> Result<Option<LoadedColumn>, WorldError> {
        let path = self.chunk_path(position);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(WorldError::Io(error)),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(WorldError::InvalidData("chunk path is not a regular file"));
        }
        self.load_runtime_column(position, resolver).map(Some)
    }

    pub fn encode_runtime_chunk(
        position: ChunkPos,
        sections: impl IntoIterator<Item = (i32, Chunk)>,
        resolver: &(impl SemanticBlockResolver + ?Sized),
    ) -> Result<StoredChunk, WorldError> {
        let mut encoded = Vec::new();
        for (y, chunk) in sections {
            let states = chunk
                .states()
                .iter()
                .map(|state| {
                    resolver
                        .key_for(*state)
                        .map(|key| (key.to_owned(), state.variant))
                        .ok_or_else(|| {
                            WorldError::UnknownBlock(format!("runtime block {}", state.block.0))
                        })
                })
                .collect::<Result<_, _>>()?;
            encoded.push(StoredSection { y, states });
        }
        Ok(StoredChunk {
            position,
            sections: encoded,
            spatial_records: Vec::new(),
            spatial_tombstones: Vec::new(),
        })
    }

    pub fn encode_runtime_column(
        position: ChunkPos,
        sections: impl IntoIterator<Item = (i32, Chunk)>,
        resolver: &(impl SemanticBlockResolver + ?Sized),
        spatial_records: Vec<SpatialRecord>,
        spatial_tombstones: Vec<SpatialTombstone>,
    ) -> Result<StoredChunk, WorldError> {
        let mut chunk = Self::encode_runtime_chunk(position, sections, resolver)?;
        chunk.spatial_records = spatial_records;
        chunk.spatial_tombstones = spatial_tombstones;
        validate_spatial_records(&chunk.spatial_records, &chunk.spatial_tombstones)?;
        Ok(chunk)
    }

    pub fn chunk_exists(&self, position: ChunkPos) -> bool {
        fs::symlink_metadata(self.chunk_path(position)).is_ok_and(|meta| meta.file_type().is_file())
    }
    pub fn chunk_file_bytes(&self, position: ChunkPos) -> Option<u64> {
        fs::symlink_metadata(self.chunk_path(position))
            .ok()
            .filter(|meta| meta.file_type().is_file())
            .map(|meta| meta.len())
    }
    /// All committed files have already been `sync_all`'d. Flush syncs directory entries where
    /// the platform exposes directory synchronization; it does not wait on asynchronous workers.
    pub fn flush(&self) -> Result<(), WorldError> {
        sync_directory(&self.root)?;
        for child in [self.root.join("chunks"), self.root.join("players")] {
            if child.is_dir() {
                sync_directory(&child)?;
            }
        }
        Ok(())
    }
    fn chunk_path(&self, position: ChunkPos) -> PathBuf {
        self.root
            .join("chunks")
            .join(format!("{}.{}.rcc", position.x, position.z))
    }
}

fn validate_player_id(id: &str) -> Result<(), WorldError> {
    if id.is_empty()
        || id.len() > 64
        || id == "."
        || id == ".."
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(WorldError::InvalidData("invalid persistent player id"));
    }
    Ok(())
}

#[derive(Debug)]
pub struct PlayerSaveCompletion {
    pub player_id: String,
    pub revision: u64,
    pub result: Result<(), WorldError>,
    pub elapsed_ms: f64,
    pub io: PlayerCheckpointIoMetrics,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerSaveMetrics {
    pub requests: u64,
    pub coalesced: u64,
    pub successes: u64,
    pub failures: u64,
    pub pending: usize,
    pub in_flight: usize,
}

struct PlayerSaveJob {
    storage: WorldStorage,
    record: PlayerRecord,
}
#[derive(Default)]
struct PlayerSaveQueue {
    pending: Option<PlayerSaveJob>,
    stopped: bool,
}

/// One bounded persistence worker with one replaceable pending player snapshot. Newer snapshots
/// coalesce while disk sync is in flight; no movement tick performs a blocking player write.
pub struct PlayerSaveScheduler {
    queue: Arc<(Mutex<PlayerSaveQueue>, Condvar)>,
    results: Option<Receiver<PlayerSaveCompletion>>,
    thread: Option<JoinHandle<()>>,
    requests: AtomicU64,
    coalesced: AtomicU64,
    successes: AtomicU64,
    failures: AtomicU64,
    in_flight: Arc<AtomicUsize>,
}

impl PlayerSaveScheduler {
    #[must_use]
    pub fn new() -> Self {
        let queue = Arc::new((Mutex::new(PlayerSaveQueue::default()), Condvar::new()));
        let worker_queue = Arc::clone(&queue);
        let worker_in_flight = Arc::new(AtomicUsize::new(0));
        let worker_in_flight_count = Arc::clone(&worker_in_flight);
        let (result_sender, results) = mpsc::sync_channel(2);
        let worker = thread::Builder::new()
            .name("rustcraft-player-save".into())
            .spawn(move || {
                loop {
                    let job = {
                        let (lock, wake) = &*worker_queue;
                        let mut state = lock.lock().expect("player save queue poisoned");
                        while state.pending.is_none() && !state.stopped {
                            state = wake.wait(state).expect("player save queue poisoned");
                        }
                        if state.stopped && state.pending.is_none() {
                            break;
                        }
                        let job = state.pending.take().expect("pending player job");
                        worker_in_flight_count.fetch_add(1, Ordering::Relaxed);
                        job
                    };
                    let started = std::time::Instant::now();
                    let revision = job.record.revision;
                    let player_id = job.record.player_id.clone();
                    let result = job.storage.store_player_measured(&job.record);
                    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
                    worker_in_flight_count.fetch_sub(1, Ordering::Relaxed);
                    let io = result.as_ref().copied().unwrap_or_default();
                    if result_sender
                        .send(PlayerSaveCompletion {
                            player_id,
                            revision,
                            result: result.map(|_| ()),
                            elapsed_ms,
                            io,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("player save worker creation failed");
        Self {
            queue,
            results: Some(results),
            thread: Some(worker),
            requests: AtomicU64::new(0),
            coalesced: AtomicU64::new(0),
            successes: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            in_flight: worker_in_flight,
        }
    }

    pub fn submit(&self, storage: WorldStorage, record: PlayerRecord) -> Result<(), WorldError> {
        self.requests.fetch_add(1, Ordering::Relaxed);
        let (lock, wake) = &*self.queue;
        let mut state = lock.lock().expect("player save queue poisoned");
        if state.stopped {
            return Err(WorldError::InvalidData("player save worker stopped"));
        }
        if let Some(existing) = state.pending.as_ref() {
            if existing.record.revision > record.revision {
                self.coalesced.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
            self.coalesced.fetch_add(1, Ordering::Relaxed);
        }
        state.pending = Some(PlayerSaveJob { storage, record });
        wake.notify_one();
        Ok(())
    }

    pub fn take_completed(&self) -> Vec<PlayerSaveCompletion> {
        let mut completed = Vec::new();
        while let Ok(completion) = self
            .results
            .as_ref()
            .expect("live player save result receiver")
            .try_recv()
        {
            if completion.result.is_ok() {
                self.successes.fetch_add(1, Ordering::Relaxed);
            } else {
                self.failures.fetch_add(1, Ordering::Relaxed);
            }
            completed.push(completion);
        }
        completed
    }

    pub fn metrics(&self) -> PlayerSaveMetrics {
        let pending = self
            .queue
            .0
            .lock()
            .map(|state| usize::from(state.pending.is_some()))
            .unwrap_or_default();
        PlayerSaveMetrics {
            requests: self.requests.load(Ordering::Relaxed),
            coalesced: self.coalesced.load(Ordering::Relaxed),
            successes: self.successes.load(Ordering::Relaxed),
            failures: self.failures.load(Ordering::Relaxed),
            pending,
            in_flight: self.in_flight.load(Ordering::Relaxed),
        }
    }
}

impl Default for PlayerSaveScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PlayerSaveScheduler {
    fn drop(&mut self) {
        self.results.take();
        let (lock, wake) = &*self.queue;
        if let Ok(mut state) = lock.lock() {
            state.stopped = true;
            wake.notify_all();
        }
        if let Some(worker) = self.thread.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Debug)]
pub struct WorldStateSaveCompletion {
    pub revision: u64,
    pub result: Result<(), WorldError>,
    pub elapsed_ms: f64,
    pub io: WorldStateCheckpointIoMetrics,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorldStateSaveMetrics {
    pub requests: u64,
    pub coalesced: u64,
    pub successes: u64,
    pub failures: u64,
    pub pending: usize,
    pub in_flight: usize,
}

struct WorldStateSaveJob {
    storage: WorldStorage,
    record: WorldStateRecord,
}

#[derive(Default)]
struct WorldStateSaveQueue {
    pending: Option<WorldStateSaveJob>,
    stopped: bool,
}

/// One bounded coalescing worker for the small world-global component record.
pub struct WorldStateSaveScheduler {
    queue: Arc<(Mutex<WorldStateSaveQueue>, Condvar)>,
    results: Option<Receiver<WorldStateSaveCompletion>>,
    thread: Option<JoinHandle<()>>,
    requests: AtomicU64,
    coalesced: AtomicU64,
    successes: AtomicU64,
    failures: AtomicU64,
    in_flight: Arc<AtomicUsize>,
}

impl WorldStateSaveScheduler {
    #[must_use]
    pub fn new() -> Self {
        let queue = Arc::new((Mutex::new(WorldStateSaveQueue::default()), Condvar::new()));
        let worker_queue = Arc::clone(&queue);
        let worker_in_flight = Arc::new(AtomicUsize::new(0));
        let worker_in_flight_count = Arc::clone(&worker_in_flight);
        let (result_sender, results) = mpsc::sync_channel(2);
        let worker = thread::Builder::new()
            .name("rustcraft-world-state-save".into())
            .spawn(move || {
                loop {
                    let job = {
                        let (lock, wake) = &*worker_queue;
                        let mut state = lock.lock().expect("world-state save queue poisoned");
                        while state.pending.is_none() && !state.stopped {
                            state = wake.wait(state).expect("world-state save queue poisoned");
                        }
                        if state.stopped && state.pending.is_none() {
                            break;
                        }
                        let job = state.pending.take().expect("pending world-state job");
                        worker_in_flight_count.fetch_add(1, Ordering::Relaxed);
                        job
                    };
                    let started = std::time::Instant::now();
                    let revision = job.record.revision;
                    let result = job.storage.store_world_state_measured(&job.record);
                    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
                    worker_in_flight_count.fetch_sub(1, Ordering::Relaxed);
                    let io = result.as_ref().copied().unwrap_or_default();
                    if result_sender
                        .send(WorldStateSaveCompletion {
                            revision,
                            result: result.map(|_| ()),
                            elapsed_ms,
                            io,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("world-state save worker creation failed");
        Self {
            queue,
            results: Some(results),
            thread: Some(worker),
            requests: AtomicU64::new(0),
            coalesced: AtomicU64::new(0),
            successes: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            in_flight: worker_in_flight,
        }
    }

    pub fn submit(
        &self,
        storage: WorldStorage,
        record: WorldStateRecord,
    ) -> Result<(), WorldError> {
        self.requests.fetch_add(1, Ordering::Relaxed);
        let (lock, wake) = &*self.queue;
        let mut state = lock.lock().expect("world-state save queue poisoned");
        if state.stopped {
            return Err(WorldError::InvalidData("world-state save worker stopped"));
        }
        if let Some(existing) = state.pending.as_ref() {
            if existing.record.revision > record.revision {
                self.coalesced.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
            self.coalesced.fetch_add(1, Ordering::Relaxed);
        }
        state.pending = Some(WorldStateSaveJob { storage, record });
        wake.notify_one();
        Ok(())
    }

    pub fn take_completed(&self) -> Vec<WorldStateSaveCompletion> {
        let mut completed = Vec::new();
        while let Ok(completion) = self
            .results
            .as_ref()
            .expect("live world-state save result receiver")
            .try_recv()
        {
            if completion.result.is_ok() {
                self.successes.fetch_add(1, Ordering::Relaxed);
            } else {
                self.failures.fetch_add(1, Ordering::Relaxed);
            }
            completed.push(completion);
        }
        completed
    }

    pub fn metrics(&self) -> WorldStateSaveMetrics {
        let pending = self
            .queue
            .0
            .lock()
            .map(|state| usize::from(state.pending.is_some()))
            .unwrap_or_default();
        WorldStateSaveMetrics {
            requests: self.requests.load(Ordering::Relaxed),
            coalesced: self.coalesced.load(Ordering::Relaxed),
            successes: self.successes.load(Ordering::Relaxed),
            failures: self.failures.load(Ordering::Relaxed),
            pending,
            in_flight: self.in_flight.load(Ordering::Relaxed),
        }
    }
}

impl Default for WorldStateSaveScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WorldStateSaveScheduler {
    fn drop(&mut self) {
        self.results.take();
        let (lock, wake) = &*self.queue;
        if let Ok(mut state) = lock.lock() {
            state.stopped = true;
            wake.notify_all();
        }
        if let Some(worker) = self.thread.take() {
            let _ = worker.join();
        }
    }
}

impl WorldStore for WorldStorage {
    fn load_metadata(&self) -> Result<WorldMetadata, WorldError> {
        WorldStorage::load_metadata(self)
    }
    fn store_metadata(&self, metadata: &WorldMetadata) -> Result<(), WorldError> {
        WorldStorage::store_metadata(self, metadata)
    }
    fn load_chunk(&self, position: ChunkPos) -> Result<StoredChunk, WorldError> {
        WorldStorage::load_chunk(self, position)
    }
    fn store_chunk(&self, chunk: &StoredChunk) -> Result<(), WorldError> {
        WorldStorage::store_chunk(self, chunk)
    }
    fn load_player(&self, player_id: &str) -> Result<Option<PlayerRecord>, WorldError> {
        WorldStorage::load_player(self, player_id)
    }
    fn store_player(&self, record: &PlayerRecord) -> Result<(), WorldError> {
        WorldStorage::store_player(self, record)
    }
    fn load_world_state(&self) -> Result<Option<WorldStateRecord>, WorldError> {
        WorldStorage::load_world_state(self)
    }
    fn store_world_state(&self, record: &WorldStateRecord) -> Result<(), WorldError> {
        WorldStorage::store_world_state(self, record)
    }
    fn chunk_exists(&self, position: ChunkPos) -> bool {
        WorldStorage::chunk_exists(self, position)
    }
    fn flush(&self) -> Result<(), WorldError> {
        WorldStorage::flush(self)
    }
}

fn valid_semantic_key(key: &str) -> bool {
    if key.len() > 256 {
        return false;
    }
    let Some((namespace, path)) = key.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && !path.contains(':')
        && namespace
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
        && path.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'/' | b'.')
        })
}

fn validate_spatial_records(
    records: &[SpatialRecord],
    tombstones: &[SpatialTombstone],
) -> Result<(), WorldError> {
    if records.len() > MAX_SPATIAL_RECORDS {
        return Err(WorldError::InvalidData("spatial entity count limit"));
    }
    if tombstones.len() > MAX_SPATIAL_TOMBSTONES {
        return Err(WorldError::InvalidData("spatial tombstone count limit"));
    }
    let mut ids = HashSet::with_capacity(records.len());
    for record in records {
        if record.entity_id == EntityId::NIL || !ids.insert(record.entity_id) {
            return Err(WorldError::InvalidData(
                "zero or duplicate spatial entity id",
            ));
        }
        if record.entity_revision == 0 {
            return Err(WorldError::InvalidData("zero spatial entity revision"));
        }
        if record.entity_type.len() > MAX_SPATIAL_TYPE_ID_BYTES
            || !valid_semantic_key(&record.entity_type)
        {
            return Err(WorldError::InvalidData("invalid spatial entity type"));
        }
        if record.payload.len() > MAX_SPATIAL_PAYLOAD_BYTES {
            return Err(WorldError::InvalidData("spatial entity payload limit"));
        }
    }
    let mut tombstone_ids = HashSet::with_capacity(tombstones.len());
    for tombstone in tombstones {
        if tombstone.entity_id == EntityId::NIL || !tombstone_ids.insert(tombstone.entity_id) {
            return Err(WorldError::InvalidData(
                "zero or duplicate spatial tombstone id",
            ));
        }
        if tombstone.entity_revision == 0 {
            return Err(WorldError::InvalidData("zero spatial tombstone revision"));
        }
    }
    Ok(())
}

fn encode_chunk_file(chunk: &StoredChunk) -> Result<(Vec<u8>, ChunkEncodingMetrics), WorldError> {
    if chunk.sections.len() > MAX_SECTIONS {
        return Err(WorldError::InvalidData("too many sections"));
    }
    validate_spatial_records(&chunk.spatial_records, &chunk.spatial_tombstones)?;
    let mut raw = Vec::new();
    let mut sorted = chunk.sections.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|section| section.y);
    if sorted.windows(2).any(|pair| pair[0].y == pair[1].y) {
        return Err(WorldError::InvalidData("duplicate section"));
    }
    for section in sorted {
        if section.states.len() != CHUNK_VOLUME {
            return Err(WorldError::InvalidData("invalid voxel count"));
        }
        let mut palette = Vec::<(String, u16)>::new();
        let mut lookup = BTreeMap::<(String, u16), u16>::new();
        let mut indexes = Vec::with_capacity(CHUNK_VOLUME);
        for (key, variant) in &section.states {
            if !valid_semantic_key(key) {
                return Err(WorldError::InvalidData("invalid semantic block key"));
            }
            let item = (key.clone(), *variant);
            let index = if let Some(index) = lookup.get(&item) {
                *index
            } else {
                if palette.len() >= MAX_PALETTE {
                    return Err(WorldError::InvalidData("palette limit"));
                }
                let index = palette.len() as u16;
                palette.push(item.clone());
                lookup.insert(item, index);
                index
            };
            indexes.push(index);
        }
        put_i32(&mut raw, section.y);
        put_u16(&mut raw, palette.len() as u16);
        for (key, variant) in palette {
            put_string(&mut raw, &key)?;
            put_u16(&mut raw, variant);
        }
        for index in indexes {
            put_u16(&mut raw, index);
        }
        if raw.len().saturating_add(64) > MAX_CHUNK_BYTES {
            return Err(WorldError::InvalidData("chunk payload limit"));
        }
    }
    let mut records = chunk.spatial_records.iter().collect::<Vec<_>>();
    records.sort_by_key(|record| record.entity_id);
    put_u16(&mut raw, records.len() as u16);
    for record in records {
        raw.extend_from_slice(&record.entity_id.0.to_le_bytes());
        put_u64(&mut raw, record.entity_revision);
        put_string(&mut raw, &record.entity_type)?;
        put_u32(&mut raw, record.schema_version);
        put_u32(
            &mut raw,
            u32::try_from(record.payload.len())
                .map_err(|_| WorldError::InvalidData("spatial entity payload limit"))?,
        );
        raw.extend_from_slice(&record.payload);
    }
    let mut tombstones = chunk.spatial_tombstones.clone();
    tombstones.sort_by_key(|record| record.entity_id);
    put_u16(&mut raw, tombstones.len() as u16);
    for tombstone in tombstones {
        raw.extend_from_slice(&tombstone.entity_id.0.to_le_bytes());
        put_u64(&mut raw, tombstone.entity_revision);
        put_i32(&mut raw, tombstone.source.x);
        put_i32(&mut raw, tombstone.source.z);
    }
    if raw.len().saturating_add(64) > MAX_CHUNK_BYTES {
        return Err(WorldError::InvalidData("chunk payload limit"));
    }
    let compression_started = std::time::Instant::now();
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&raw)?;
    let compressed = encoder.finish()?;
    let compression_ms = compression_started.elapsed().as_secs_f64() * 1000.0;
    let (method, payload) = if compressed.len() < raw.len() {
        (1, compressed.as_slice())
    } else {
        (0, raw.as_slice())
    };
    let mut out = Vec::with_capacity(8 + 4 + 8 + 2 + 1 + 4 + payload.len() + 32);
    out.extend_from_slice(CHUNK_MAGIC);
    put_u32(&mut out, CHUNK_FORMAT_VERSION);
    put_i32(&mut out, chunk.position.x);
    put_i32(&mut out, chunk.position.z);
    put_u16(&mut out, chunk.sections.len() as u16);
    out.push(method);
    put_u32(
        &mut out,
        u32::try_from(raw.len()).map_err(|_| WorldError::InvalidData("decoded chunk limit"))?,
    );
    out.extend_from_slice(payload);
    if out.len().saturating_add(32) > MAX_CHUNK_BYTES {
        return Err(WorldError::InvalidData("chunk file limit"));
    }
    finish_checksum(&mut out);
    let metrics = ChunkEncodingMetrics {
        raw_payload_bytes: raw.len(),
        stored_file_bytes: out.len(),
        compression_ms,
        compression_method: method,
    };
    Ok((out, metrics))
}
fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), WorldError> {
    if value.len() > 4096 {
        return Err(WorldError::InvalidData("string limit"));
    }
    put_u16(out, value.len() as u16);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put_i32(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put_i64(out: &mut Vec<u8>, v: i64) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn finish_checksum(out: &mut Vec<u8>) {
    let hash = blake3::hash(out);
    out.extend_from_slice(hash.as_bytes());
}
fn verify_file<'a>(data: &'a [u8], magic: &[u8]) -> Result<&'a [u8], WorldError> {
    if data.len() < magic.len() + 32 || &data[..magic.len()] != magic {
        return Err(WorldError::InvalidData("magic/checksum framing"));
    }
    let (body, checksum) = data.split_at(data.len() - 32);
    if blake3::hash(body).as_bytes() != checksum {
        return Err(WorldError::InvalidData("checksum mismatch"));
    }
    Ok(&body[magic.len()..])
}
fn read_limited(path: &Path, limit: usize) -> Result<Vec<u8>, WorldError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(WorldError::InvalidData("expected regular file"));
    }
    let file = File::open(path)?;
    let length =
        usize::try_from(metadata.len()).map_err(|_| WorldError::InvalidData("file too large"))?;
    if length > limit {
        return Err(WorldError::InvalidData("file limit"));
    }
    let mut data = Vec::with_capacity(length);
    file.take((limit + 1) as u64).read_to_end(&mut data)?;
    if data.len() != length {
        return Err(WorldError::InvalidData("file changed while reading"));
    }
    Ok(data)
}
fn atomic_write(path: &Path, data: &[u8]) -> Result<(), WorldError> {
    let parent = path
        .parent()
        .ok_or(WorldError::InvalidData("missing parent"))?;
    atomicwrites::AtomicFile::new(path, atomicwrites::AllowOverwrite)
        .write(|file| file.write_all(data))
        .map_err(|error| WorldError::Io(error.into()))?;
    sync_directory(parent)?;
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

// std does not provide a portable way to open/sync a directory on Windows. Files are still
// flushed before atomic replacement; Windows directory-entry durability is not claimed here.
#[cfg(windows)]
fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn durable_write_slot(path: &Path, data: &[u8]) -> Result<(f64, f64), WorldError> {
    if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(WorldError::InvalidData("player checkpoint is a symlink"));
    }
    let mut write_ms = 0.0;
    let checkpoint_started = std::time::Instant::now();
    atomicwrites::AtomicFile::new(path, atomicwrites::AllowOverwrite)
        .write(|file| {
            let write_started = std::time::Instant::now();
            file.write_all(data)?;
            write_ms = write_started.elapsed().as_secs_f64() * 1000.0;
            file.sync_all()?;
            Ok(())
        })
        .map_err(|error| WorldError::Io(error.into()))?;
    if let Some(parent) = path.parent() {
        // Persist a newly created directory entry where directory sync is supported. Some
        // platforms reject opening directories; the checkpoint file itself is still synced.
        sync_directory(parent)?;
    }
    let sync_ms = (checkpoint_started.elapsed().as_secs_f64() * 1000.0 - write_ms).max(0.0);
    Ok((write_ms, sync_ms))
}

fn read_player_checkpoint(
    path: &Path,
    requested_id: &str,
) -> Result<Option<PlayerRecord>, WorldError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(WorldError::Io(error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(WorldError::InvalidData("invalid player checkpoint path"));
    }
    let data = read_limited(path, MAX_PLAYER_RECORD_BYTES)?;
    let body = verify_file(&data, PLAYER_MAGIC)?;
    let mut r = Reader::new(body);
    let version = r.u32()?;
    if version != PLAYER_RECORD_VERSION {
        return Err(WorldError::UnsupportedVersion(version));
    }
    let saved_id = r.string()?;
    validate_player_id(&saved_id)?;
    if saved_id != requested_id {
        return Err(WorldError::InvalidData("player record identity mismatch"));
    }
    let revision = r.u64()?;
    if revision == 0 {
        return Err(WorldError::InvalidData("zero player revision"));
    }
    let count = r.u16()? as usize;
    if count > MAX_PLAYER_COMPONENTS {
        return Err(WorldError::InvalidData("player component count limit"));
    }
    let mut components = Vec::with_capacity(count);
    for _ in 0..count {
        let id = r.string()?;
        let schema_version = r.u32()?;
        let length = r.u32()? as usize;
        if length > MAX_PLAYER_COMPONENT_BYTES {
            return Err(WorldError::InvalidData("player component payload limit"));
        }
        components.push(PlayerComponent {
            id,
            schema_version,
            payload: r.take(length)?.to_vec(),
        });
    }
    r.finish()?;
    validate_player_components(&components)?;
    Ok(Some(PlayerRecord {
        player_id: saved_id,
        revision,
        components,
        recovered_from_checkpoint: false,
    }))
}

fn read_player_legacy(path: &Path, requested_id: &str) -> Result<Option<PlayerRecord>, WorldError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(WorldError::Io(error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(WorldError::InvalidData("invalid legacy player record path"));
    }
    let data = read_limited(path, MAX_PLAYER_RECORD_BYTES)?;
    let body = verify_file(&data, PLAYER_MAGIC)?;
    let mut r = Reader::new(body);
    let version = r.u32()?;
    if version != LEGACY_PLAYER_RECORD_VERSION {
        return Err(WorldError::UnsupportedVersion(version));
    }
    let saved_id = r.string()?;
    validate_player_id(&saved_id)?;
    if saved_id != requested_id {
        return Err(WorldError::InvalidData("player record identity mismatch"));
    }
    let game_schema_version = r.u32()?;
    let length = r.u32()? as usize;
    if length > MAX_PLAYER_COMPONENT_BYTES {
        return Err(WorldError::InvalidData("legacy player payload limit"));
    }
    let payload = r.take(length)?.to_vec();
    r.finish()?;
    Ok(Some(PlayerRecord {
        player_id: saved_id,
        revision: 0,
        components: vec![PlayerComponent {
            id: "rustcraft:legacy-player-payload".into(),
            schema_version: game_schema_version,
            payload,
        }],
        recovered_from_checkpoint: false,
    }))
}

fn read_world_state_checkpoint(path: &Path) -> Result<Option<WorldStateRecord>, WorldError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(WorldError::Io(error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(WorldError::InvalidData(
            "invalid world-state checkpoint path",
        ));
    }
    let data = read_limited(path, MAX_WORLD_STATE_BYTES)?;
    let body = verify_file(&data, WORLD_STATE_MAGIC)?;
    let mut r = Reader::new(body);
    let version = r.u32()?;
    if version != WORLD_STATE_RECORD_VERSION {
        return Err(WorldError::UnsupportedVersion(version));
    }
    let revision = r.u64()?;
    if revision == 0 {
        return Err(WorldError::InvalidData("zero world-state revision"));
    }
    let count = r.u16()? as usize;
    if count > MAX_WORLD_COMPONENTS {
        return Err(WorldError::InvalidData("world component count limit"));
    }
    let mut components = Vec::with_capacity(count);
    for _ in 0..count {
        let id = r.string()?;
        let schema_version = r.u32()?;
        let length = r.u32()? as usize;
        if length > MAX_WORLD_COMPONENT_BYTES {
            return Err(WorldError::InvalidData("world component payload limit"));
        }
        components.push(WorldStateComponent {
            id,
            schema_version,
            payload: r.take(length)?.to_vec(),
        });
    }
    r.finish()?;
    validate_world_components(&components)?;
    Ok(Some(WorldStateRecord {
        revision,
        components,
        recovered_from_checkpoint: false,
    }))
}

fn validate_world_components(components: &[WorldStateComponent]) -> Result<(), WorldError> {
    if components.len() > MAX_WORLD_COMPONENTS {
        return Err(WorldError::InvalidData("world component count limit"));
    }
    let mut previous: Option<&str> = None;
    let mut total = 0usize;
    for component in components {
        if component.id.len() > MAX_WORLD_COMPONENT_ID_BYTES || !valid_semantic_key(&component.id) {
            return Err(WorldError::InvalidData("invalid world component id"));
        }
        if previous.is_some_and(|prior| prior >= component.id.as_str()) {
            return Err(WorldError::InvalidData(
                "world components must be unique and sorted",
            ));
        }
        if component.payload.len() > MAX_WORLD_COMPONENT_BYTES {
            return Err(WorldError::InvalidData("world component payload limit"));
        }
        total = total
            .checked_add(component.payload.len())
            .ok_or(WorldError::InvalidData("world payload length overflow"))?;
        if total > MAX_WORLD_STATE_BYTES {
            return Err(WorldError::InvalidData("world payload limit"));
        }
        previous = Some(&component.id);
    }
    Ok(())
}

fn validate_player_components(components: &[PlayerComponent]) -> Result<(), WorldError> {
    if components.len() > MAX_PLAYER_COMPONENTS {
        return Err(WorldError::InvalidData("player component count limit"));
    }
    let mut previous: Option<&str> = None;
    let mut total = 0usize;
    for component in components {
        if component.id.len() > MAX_PLAYER_COMPONENT_ID_BYTES || !valid_semantic_key(&component.id)
        {
            return Err(WorldError::InvalidData("invalid player component id"));
        }
        if previous.is_some_and(|prior| prior >= component.id.as_str()) {
            return Err(WorldError::InvalidData(
                "player components must be unique and sorted",
            ));
        }
        if component.payload.len() > MAX_PLAYER_COMPONENT_BYTES {
            return Err(WorldError::InvalidData("player component payload limit"));
        }
        total = total
            .checked_add(component.payload.len())
            .ok_or(WorldError::InvalidData("player payload length overflow"))?;
        if total > MAX_PLAYER_RECORD_BYTES {
            return Err(WorldError::InvalidData("player payload limit"));
        }
        previous = Some(&component.id);
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], WorldError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(WorldError::InvalidData("length overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(WorldError::InvalidData("truncated"))?;
        self.offset = end;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16, WorldError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u8(&mut self) -> Result<u8, WorldError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, WorldError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, WorldError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, WorldError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, WorldError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<String, WorldError> {
        let len = self.u16()? as usize;
        if len > 4096 {
            return Err(WorldError::InvalidData("string limit"));
        }
        String::from_utf8(self.take(len)?.to_vec())
            .map_err(|_| WorldError::InvalidData("string encoding"))
    }
    fn finish(&self) -> Result<(), WorldError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(WorldError::InvalidData("trailing bytes"))
        }
    }
    fn remaining(&mut self) -> &'a [u8] {
        let value = &self.bytes[self.offset..];
        self.offset = self.bytes.len();
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Resolver(HashMap<BlockId, String>, HashMap<String, BlockId>);
    impl SemanticBlockResolver for Resolver {
        fn key_for(&self, state: BlockState) -> Option<&str> {
            self.0.get(&state.block).map(String::as_str)
        }
        fn state_for(&self, key: &str, variant: u16) -> Option<BlockState> {
            self.1
                .get(key)
                .copied()
                .map(|block| BlockState { block, variant })
        }
    }

    fn temp_root() -> PathBuf {
        static NEXT_TEMP_ROOT: AtomicU64 = AtomicU64::new(0);
        loop {
            let id = NEXT_TEMP_ROOT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("rustcraft-world-test-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return path,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create temp world root: {error}"),
            }
        }
    }
    fn resolver(a: u32, b: u32) -> Resolver {
        Resolver(
            HashMap::from([
                (BlockId(a), "sample:air".to_owned()),
                (BlockId(b), "sample:stone".to_owned()),
            ]),
            HashMap::from([
                ("sample:air".to_owned(), BlockId(a)),
                ("sample:stone".to_owned(), BlockId(b)),
            ]),
        )
    }

    struct EmptyGenerator;
    impl ChunkGenerator for EmptyGenerator {
        fn id(&self) -> &str {
            "test:empty"
        }
        fn version(&self) -> u32 {
            1
        }
        fn generate(
            &self,
            _seed: i64,
            _position: ChunkPos,
        ) -> Result<Vec<(i32, Chunk)>, WorldError> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn scheduler_coalesces_duplicate_generation_and_rejects_stale_results() {
        let mut scheduler = GenerationScheduler::new(2, 2);
        let generator: Arc<dyn ChunkGenerator> = Arc::new(EmptyGenerator);
        let pos = ChunkPos { x: -4, z: 8 };
        assert!(scheduler.request(generator.clone(), 7, pos, 1).unwrap());
        assert!(!scheduler.request(generator.clone(), 7, pos, 1).unwrap());
        assert!(scheduler.request(generator.clone(), 7, pos, 2).unwrap());
        assert_eq!(scheduler.metrics().submitted, 2);
        assert_eq!(scheduler.metrics().coalesced, 1);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut accepted = Vec::new();
        while std::time::Instant::now() < deadline && accepted.is_empty() {
            accepted.extend(scheduler.take_ready());
            std::thread::yield_now();
        }
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].generation, 2);
        while std::time::Instant::now() < deadline && scheduler.metrics().stale_discarded == 0 {
            let _ = scheduler.take_ready();
            std::thread::yield_now();
        }
        assert_eq!(scheduler.metrics().stale_discarded, 1);
        assert_eq!(scheduler.metrics().completed, 2);
    }

    #[test]
    fn generation_publication_budget_leaves_other_completed_results_queued() {
        let mut scheduler = GenerationScheduler::new(2, 2);
        let generator: Arc<dyn ChunkGenerator> = Arc::new(EmptyGenerator);
        let positions = [
            ChunkPos { x: -2, z: 0 },
            ChunkPos { x: 0, z: 0 },
            ChunkPos { x: 2, z: 0 },
        ];
        for (token, position) in positions.into_iter().enumerate() {
            scheduler
                .request(generator.clone(), 7, position, token as u64 + 1)
                .unwrap();
        }

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut published = Vec::new();
        while published.len() < positions.len() && std::time::Instant::now() < deadline {
            let next = scheduler.take_ready_limit(1);
            assert!(next.len() <= 1, "publication budget was exceeded");
            published.extend(next.into_iter().map(|result| result.position));
            std::thread::yield_now();
        }
        assert_eq!(published.len(), positions.len());
        assert!(
            positions
                .into_iter()
                .all(|position| published.contains(&position))
        );
    }

    #[test]
    fn async_load_distinguishes_loaded_missing_and_corrupt_and_resolves_off_thread() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "stream").unwrap();
        let resolver: Arc<dyn SemanticBlockResolver> = Arc::new(resolver(1, 2));
        let loaded = ChunkPos { x: -2, z: 3 };
        let missing = ChunkPos { x: 4, z: -5 };
        let corrupt = ChunkPos { x: 7, z: 8 };
        let states = vec![("sample:stone".to_owned(), 2); CHUNK_VOLUME];
        storage
            .store_chunk(&StoredChunk {
                position: loaded,
                sections: vec![StoredSection { y: 0, states }],
                spatial_records: Vec::new(),
                spatial_tombstones: Vec::new(),
            })
            .unwrap();
        fs::write(storage.chunk_path(corrupt), b"truncated").unwrap();
        let mut scheduler = ChunkLoadScheduler::new(2, 2);
        for (token, position) in [(1, loaded), (2, missing), (3, corrupt)] {
            scheduler
                .submit(
                    storage.clone(),
                    ResidencyRequest { position, token },
                    resolver.clone(),
                )
                .unwrap();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut completed = Vec::new();
        while std::time::Instant::now() < deadline && completed.len() < 3 {
            completed.extend(scheduler.take_ready(3 - completed.len()));
            std::thread::yield_now();
        }
        assert_eq!(completed.len(), 3);
        assert!(completed.iter().any(|entry| {
            entry.request.position == loaded
                && entry.result.as_ref().is_ok_and(|loaded| {
                    loaded
                        .as_ref()
                        .is_some_and(|column| column.sections[0].1.get((0, 0, 0)) == BlockId(2))
                })
        }));
        assert!(completed.iter().any(|entry| {
            entry.request.position == missing && matches!(entry.result, Ok(None))
        }));
        assert!(
            completed
                .iter()
                .any(|entry| { entry.request.position == corrupt && entry.result.is_err() })
        );
        drop(scheduler);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_or_failed_save_never_clears_new_dirty_generation() {
        let mut dirty = PersistenceDirtyTracker::default();
        let pos = ChunkPos { x: -2, z: 9 };
        dirty.mark_dirty(pos);
        let old = dirty.begin_save(pos).unwrap();
        assert!(dirty.begin_save(pos).is_none());
        dirty.mark_dirty(pos);
        dirty.complete_save(old, true);
        assert_eq!(dirty.dirty_count(), 1);
        let current = dirty.begin_save(pos).unwrap();
        dirty.complete_save(current, false);
        assert_eq!(dirty.dirty_count(), 1);
        let retry = dirty.begin_save(pos).unwrap();
        dirty.complete_save(retry, true);
        assert_eq!(dirty.dirty_count(), 0);
        assert_eq!(dirty.metrics().failed, 1);
    }

    #[test]
    fn rsm1_delayed_failed_and_stale_saves_keep_eviction_pin_until_retry_ack() {
        let old = ChunkPos { x: -8, z: 0 };
        let center = ChunkPos { x: 8, z: -8 };
        let residents = HashSet::from([old]);
        let mut residency = WorldResidency::new(2, 1);
        let mut dirty = PersistenceDirtyTracker::default();
        dirty.mark_dirty(old);
        let delayed = dirty.begin_save(old).unwrap();
        let pinned = |dirty: &PersistenceDirtyTracker| {
            residents
                .iter()
                .copied()
                .filter(|p| dirty.is_dirty(*p) || dirty.is_saving(*p))
                .collect::<HashSet<_>>()
        };
        assert!(
            !residency
                .update(center, &residents, &pinned(&dirty))
                .evict
                .contains(&old)
        );
        dirty.mark_dirty(old); // edit while the older snapshot is in flight
        dirty.complete_save(delayed, true);
        assert!(
            !residency
                .update(center, &residents, &pinned(&dirty))
                .evict
                .contains(&old)
        );
        let failed = dirty.begin_save(old).unwrap();
        dirty.complete_save(failed, false);
        assert!(dirty.is_dirty(old));
        assert!(
            !residency
                .update(center, &residents, &pinned(&dirty))
                .evict
                .contains(&old)
        );
        let retry = dirty.begin_save(old).unwrap();
        dirty.complete_save(retry, true);
        assert!(!dirty.is_dirty(old) && !dirty.is_saving(old));
        assert!(
            residency
                .update(center, &residents, &pinned(&dirty))
                .evict
                .contains(&old)
        );
    }

    #[test]
    fn lifecycle_requires_request_then_complete_publication() {
        let mut life = ChunkLifecycle::default();
        let pos = ChunkPos { x: 1, z: -1 };
        assert!(life.transition(pos, ChunkPhase::Ready).is_err());
        life.transition(pos, ChunkPhase::Requested).unwrap();
        life.transition(pos, ChunkPhase::Generating).unwrap();
        life.transition(pos, ChunkPhase::Ready).unwrap();
        life.transition(pos, ChunkPhase::Dirty).unwrap();
        life.transition(pos, ChunkPhase::Saving).unwrap();
        life.transition(pos, ChunkPhase::Dirty).unwrap();
        assert_eq!(life.phase(pos), Some(ChunkPhase::Dirty));
    }

    #[test]
    fn semantic_palette_roundtrip_survives_runtime_id_remapping_and_negative_coordinates() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "roundtrip").unwrap();
        let mut section = Chunk::new(BlockId(4));
        section.set_state(
            (1, 2, 3),
            BlockState {
                block: BlockId(9),
                variant: 17,
            },
        );
        let pos = ChunkPos { x: -12, z: 31 };
        let stored =
            WorldStorage::encode_runtime_chunk(pos, [(2, section)], &resolver(4, 9)).unwrap();
        let encoded = storage.store_chunk_measured(&stored).unwrap();
        assert_eq!(encoded.compression_method, 1);
        assert!(encoded.stored_file_bytes < encoded.raw_payload_bytes);
        let loaded = storage
            .load_runtime_chunk(pos, &resolver(100, 101))
            .unwrap();
        assert_eq!(loaded[0].0, 2);
        assert_eq!(
            loaded[0].1.state((1, 2, 3)),
            BlockState {
                block: BlockId(101),
                variant: 17
            }
        );
        assert_eq!(loaded[0].1.state((0, 0, 0)).block, BlockId(100));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn metadata_is_versioned_and_atomic_files_have_checksum_validation() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "world_1").unwrap();
        let metadata = WorldMetadata {
            seed: -123,
            game_id: "sample:game".into(),
            profile_fingerprint: "abc".into(),
            persistence_schema_version: PERSISTED_STATE_SCHEMA_VERSION,
            generator_id: "sample:flat".into(),
            generator_version: 3,
        };
        storage.store_metadata(&metadata).unwrap();
        assert_eq!(storage.load_metadata().unwrap(), metadata);
        let path = storage.root().join("world.rcw");
        let mut data = fs::read(&path).unwrap();
        data[10] ^= 0x80;
        fs::write(path, data).unwrap();
        assert!(matches!(
            storage.load_metadata(),
            Err(WorldError::InvalidData("checksum mismatch"))
        ));
        assert!(matches!(
            WorldStorage::open(&root, "../escape"),
            Err(WorldError::InvalidWorldName)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn metadata_and_chunk_overwrites_flush_and_reopen_portably() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "overwrite").unwrap();
        let metadata = WorldMetadata {
            seed: 1,
            game_id: "sample:game".into(),
            profile_fingerprint: "profile-a".into(),
            persistence_schema_version: PERSISTED_STATE_SCHEMA_VERSION,
            generator_id: "sample:flat".into(),
            generator_version: 1,
        };
        storage.store_metadata(&metadata).unwrap();
        let mut replacement = metadata.clone();
        replacement.seed = 2;
        replacement.profile_fingerprint = "profile-b".into();
        storage.store_metadata(&replacement).unwrap();

        let position = ChunkPos { x: -3, z: 5 };
        let chunk = |variant| StoredChunk {
            position,
            sections: vec![StoredSection {
                y: 0,
                states: vec![("sample:stone".into(), variant); CHUNK_VOLUME],
            }],
            spatial_records: Vec::new(),
            spatial_tombstones: Vec::new(),
        };
        storage.store_chunk(&chunk(1)).unwrap();
        storage.store_chunk(&chunk(2)).unwrap();
        let make_player = |revision, value| PlayerRecord {
            player_id: "local-player".into(),
            revision,
            components: vec![PlayerComponent {
                id: "sample:player/state".into(),
                schema_version: 1,
                payload: vec![value],
            }],
            recovered_from_checkpoint: false,
        };
        storage.store_player(&make_player(1, 1)).unwrap();
        storage.store_player(&make_player(2, 2)).unwrap();
        storage.flush().unwrap();
        drop(storage);

        let reopened = WorldStorage::open(&root, "overwrite").unwrap();
        assert_eq!(reopened.load_metadata().unwrap(), replacement);
        assert_eq!(reopened.load_chunk(position).unwrap(), chunk(2));
        assert_eq!(
            reopened.load_player("local-player").unwrap(),
            Some(make_player(2, 2))
        );
        reopened.flush().unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn player_record_is_versioned_bounded_semantic_opaque_and_atomic() {
        let root =
            std::env::temp_dir().join(format!("rustcraft-player-record-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let storage = WorldStorage::open(&root, "world").unwrap();
        let record = PlayerRecord {
            player_id: "local-player".into(),
            revision: 1,
            components: vec![PlayerComponent {
                id: "sample:player/test".into(),
                schema_version: 3,
                payload: vec![1, 2, 3, 4],
            }],
            recovered_from_checkpoint: false,
        };
        storage.store_player(&record).unwrap();
        assert_eq!(
            storage.load_player("local-player").unwrap(),
            Some(record.clone())
        );
        assert!(storage.load_player("../escape").is_err());
        let mut newer = record.clone();
        newer.revision = 2;
        newer.components[0].payload.push(5);
        storage.store_player(&newer).unwrap();
        let path = storage.root().join("players/local-player.0.rcp");
        let valid = fs::read(&path).unwrap();
        fs::write(&path, &valid[..valid.len() - 2]).unwrap();
        let loaded = storage.load_player("local-player").unwrap().unwrap();
        assert_eq!(loaded.revision, 1);
        assert!(loaded.recovered_from_checkpoint);
        fs::write(storage.root().join("players/local-player.1.rcp"), b"bad").unwrap();
        assert!(storage.load_player("local-player").is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn player_checkpoint_recovers_highest_valid_revision_and_migrates_legacy_payload() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "player_slots").unwrap();
        let make = |revision, byte| PlayerRecord {
            player_id: "local-player".into(),
            revision,
            components: vec![PlayerComponent {
                id: "sample:player/test".into(),
                schema_version: 1,
                payload: vec![byte],
            }],
            recovered_from_checkpoint: false,
        };
        storage.store_player(&make(1, 11)).unwrap();
        storage.store_player(&make(2, 22)).unwrap();
        let newest = storage.root().join("players/local-player.0.rcp");
        let mut corrupted = fs::read(&newest).unwrap();
        let last = corrupted.len() - 33;
        corrupted[last] ^= 0x80;
        fs::write(&newest, corrupted).unwrap();
        let recovered = storage.load_player("local-player").unwrap().unwrap();
        assert_eq!(recovered.revision, 1);
        assert!(recovered.recovered_from_checkpoint);

        let legacy_path = storage.root().join("players/legacy-player.rcp");
        let mut legacy = Vec::new();
        legacy.extend_from_slice(PLAYER_MAGIC);
        put_u32(&mut legacy, LEGACY_PLAYER_RECORD_VERSION);
        put_string(&mut legacy, "legacy-player").unwrap();
        put_u32(&mut legacy, 7);
        put_u32(&mut legacy, 3);
        legacy.extend_from_slice(&[4, 5, 6]);
        finish_checksum(&mut legacy);
        fs::write(legacy_path, legacy).unwrap();
        let migrated = storage.load_player("legacy-player").unwrap().unwrap();
        assert_eq!(migrated.revision, 0);
        assert_eq!(migrated.components[0].id, "rustcraft:legacy-player-payload");
        assert_eq!(migrated.components[0].schema_version, 7);
        assert_eq!(migrated.components[0].payload, [4, 5, 6]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn player_save_worker_coalesces_and_stale_completion_cannot_replace_new_revision() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "player_worker").unwrap();
        let make = |revision, byte| PlayerRecord {
            player_id: "local-player".into(),
            revision,
            components: vec![PlayerComponent {
                id: "sample:player/test".into(),
                schema_version: 1,
                payload: vec![byte],
            }],
            recovered_from_checkpoint: false,
        };
        let worker = PlayerSaveScheduler::new();
        for (revision, byte) in [(1, 1), (2, 2), (3, 3)] {
            worker
                .submit(storage.clone(), make(revision, byte))
                .unwrap();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut newest_completed = 0;
        while newest_completed < 3 && std::time::Instant::now() < deadline {
            for completion in worker.take_completed() {
                assert!(completion.result.is_ok());
                newest_completed = newest_completed.max(completion.revision);
            }
            std::thread::yield_now();
        }
        assert_eq!(newest_completed, 3);
        storage.store_player(&make(2, 22)).unwrap();
        let loaded = storage.load_player("local-player").unwrap().unwrap();
        assert_eq!(loaded.revision, 3);
        assert_eq!(loaded.components[0].payload, [3]);
        assert_eq!(worker.metrics().requests, 3);
        drop(worker); // clean worker join is part of the test
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn chunk_coordinate_and_voxel_count_are_validated() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "validation").unwrap();
        let chunk = StoredChunk {
            position: ChunkPos { x: 0, z: 0 },
            sections: vec![StoredSection {
                y: 0,
                states: vec![("sample:air".into(), 0); CHUNK_VOLUME],
            }],
            spatial_records: Vec::new(),
            spatial_tombstones: Vec::new(),
        };
        storage.store_chunk(&chunk).unwrap();
        assert!(storage.load_chunk(ChunkPos { x: 1, z: 0 }).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_semantic_content_is_an_explicit_load_error() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "unknown").unwrap();
        let pos = ChunkPos { x: 4, z: -5 };
        let stored = StoredChunk {
            position: pos,
            sections: vec![StoredSection {
                y: 0,
                states: vec![("missing:block".into(), 0); CHUNK_VOLUME],
            }],
            spatial_records: Vec::new(),
            spatial_tombstones: Vec::new(),
        };
        storage.store_chunk(&stored).unwrap();
        assert!(
            matches!(storage.load_runtime_chunk(pos, &resolver(1, 2)), Err(WorldError::UnknownBlock(key)) if key == "missing:block")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn profile_presentation_evolution_does_not_block_semantic_chunk_loads() {
        let saved = WorldMetadata {
            seed: 731_173,
            game_id: "minecraft_b173:profile/default".into(),
            profile_fingerprint: "8e898bbaff8032b662f0f51e11fc1c821e482d0a1a84cc08ddc9dfa92bfeb4a2"
                .into(),
            persistence_schema_version: PERSISTED_STATE_SCHEMA_VERSION,
            generator_id: "minecraft_b173:overworld".into(),
            generator_version: 1,
        };
        let mut active = saved.clone();
        active.profile_fingerprint =
            "bac99559e8b58bef22ac76860d8f109ac79b05e794b7b1b47329efe529e802d8".into();

        validate_world_compatibility(&saved, &active, false).unwrap();

        let root = temp_root();
        let storage = WorldStorage::open(&root, "profile_evolution").unwrap();
        let pos = ChunkPos { x: 0, z: 0 };
        let stored = StoredChunk {
            position: pos,
            sections: vec![StoredSection {
                y: 0,
                states: vec![("minecraft_b173:water".into(), 0); CHUNK_VOLUME],
            }],
            spatial_records: Vec::new(),
            spatial_tombstones: Vec::new(),
        };
        storage.store_chunk(&stored).unwrap();
        let water_resolver = Resolver(
            HashMap::from([(BlockId(16), "minecraft_b173:water".to_owned())]),
            HashMap::from([("minecraft_b173:water".to_owned(), BlockId(16))]),
        );
        let loaded = storage.load_runtime_chunk(pos, &water_resolver).unwrap();
        assert_eq!(
            loaded[0].1.state((0, 0, 0)),
            BlockState {
                block: BlockId(16),
                variant: 0
            }
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn world_compatibility_reports_actual_hard_incompatibilities() {
        let saved = WorldMetadata {
            seed: 1,
            game_id: "sample:game".into(),
            profile_fingerprint: "old".into(),
            persistence_schema_version: PERSISTED_STATE_SCHEMA_VERSION,
            generator_id: "sample:flat".into(),
            generator_version: 1,
        };
        let mut active = saved.clone();
        active.generator_version = 2;
        assert!(validate_world_compatibility(&saved, &active, false).is_ok());
        assert!(matches!(
            validate_world_compatibility(&saved, &active, true),
            Err(WorldCompatibilityError::Generator {
                saved_version: 1,
                active_version: 2,
                ..
            })
        ));
        active.generator_version = 1;
        active.persistence_schema_version += 1;
        assert!(matches!(
            validate_world_compatibility(&saved, &active, false),
            Err(WorldCompatibilityError::PersistenceSchema { .. })
        ));
        active.persistence_schema_version = saved.persistence_schema_version;
        active.game_id = "other:game".into();
        assert!(matches!(
            validate_world_compatibility(&saved, &active, false),
            Err(WorldCompatibilityError::GameProfile { .. })
        ));
    }

    #[test]
    fn legacy_metadata_v1_is_read_as_schema_v1_and_rewritten_as_v2() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "metadata_migration").unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(WORLD_MAGIC);
        put_u32(&mut bytes, WORLD_FORMAT_VERSION);
        put_u32(&mut bytes, 1);
        put_i64(&mut bytes, 731_173);
        put_string(&mut bytes, "minecraft_b173:profile/default").unwrap();
        put_string(&mut bytes, "legacy-full-profile-hash").unwrap();
        put_string(&mut bytes, "minecraft_b173:overworld").unwrap();
        put_u32(&mut bytes, 1);
        finish_checksum(&mut bytes);
        fs::write(storage.root().join("world.rcw"), bytes).unwrap();

        let legacy = storage.load_metadata().unwrap();
        assert_eq!(
            legacy.persistence_schema_version,
            PERSISTED_STATE_SCHEMA_VERSION
        );
        assert_eq!(legacy.profile_fingerprint, "legacy-full-profile-hash");
        storage.store_metadata(&legacy).unwrap();
        let migrated = storage.load_metadata().unwrap();
        assert_eq!(migrated, legacy);
        let data = fs::read(storage.root().join("world.rcw")).unwrap();
        assert_eq!(
            u32::from_le_bytes(data[12..16].try_into().unwrap()),
            WORLD_METADATA_VERSION
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn bounded_save_worker_persists_and_joins_cleanly() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "worker").unwrap();
        let position = ChunkPos { x: -1, z: -1 };
        let stored = StoredChunk {
            position,
            sections: vec![StoredSection {
                y: 0,
                states: vec![("sample:air".into(), 0); CHUNK_VOLUME],
            }],
            spatial_records: Vec::new(),
            spatial_tombstones: Vec::new(),
        };
        let token = SaveToken {
            position,
            generation: 12,
        };
        let mut scheduler = SaveScheduler::new(1, 1);
        scheduler.submit(storage.clone(), token, stored).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut complete = Vec::new();
        while complete.is_empty() && std::time::Instant::now() < deadline {
            complete.extend(scheduler.take_completed());
            std::thread::yield_now();
        }
        assert_eq!(complete.len(), 1);
        assert_eq!(complete[0].token.generation, 12);
        assert!(complete[0].result.is_ok());
        drop(scheduler);
        assert!(storage.load_chunk(position).is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn spatial_envelopes_roundtrip_and_legacy_v2_means_zero_entities() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "spatial").unwrap();
        let position = ChunkPos { x: -12, z: 19 };
        let entity_id = EntityId::from_parts(7, 11);
        let stored = StoredChunk {
            position,
            sections: Vec::new(),
            spatial_records: vec![SpatialRecord {
                entity_id,
                entity_revision: 4,
                entity_type: "sample:entity/item".into(),
                schema_version: 2,
                payload: vec![1, 2, 3, 4],
            }],
            spatial_tombstones: vec![SpatialTombstone {
                entity_id: EntityId::from_parts(7, 12),
                entity_revision: 5,
                source: ChunkPos { x: -13, z: 19 },
            }],
        };
        storage.store_chunk(&stored).unwrap();
        assert_eq!(storage.load_chunk(position).unwrap(), stored);

        let legacy_position = ChunkPos { x: -2, z: -3 };
        let mut legacy = Vec::new();
        legacy.extend_from_slice(CHUNK_MAGIC);
        put_u32(&mut legacy, LEGACY_CHUNK_FORMAT_VERSION);
        put_i32(&mut legacy, legacy_position.x);
        put_i32(&mut legacy, legacy_position.z);
        put_u16(&mut legacy, 0);
        legacy.push(0); // uncompressed
        put_u32(&mut legacy, 0); // zero-byte raw payload
        finish_checksum(&mut legacy);
        fs::write(storage.chunk_path(legacy_position), legacy).unwrap();
        let migrated = storage.load_chunk(legacy_position).unwrap();
        assert!(migrated.sections.is_empty());
        assert!(migrated.spatial_records.is_empty());
        assert!(migrated.spatial_tombstones.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pre_m4_003_world_keeps_voxel_and_player_then_upgrades_naturally() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "legacy_m4").unwrap();
        let position = ChunkPos { x: -4, z: 6 };
        let mut raw = Vec::new();
        put_i32(&mut raw, 0);
        put_u16(&mut raw, 1);
        put_string(&mut raw, "sample:stone").unwrap();
        put_u16(&mut raw, 77);
        for _ in 0..CHUNK_VOLUME {
            put_u16(&mut raw, 0);
        }
        let mut legacy = Vec::new();
        legacy.extend_from_slice(CHUNK_MAGIC);
        put_u32(&mut legacy, LEGACY_CHUNK_FORMAT_VERSION);
        put_i32(&mut legacy, position.x);
        put_i32(&mut legacy, position.z);
        put_u16(&mut legacy, 1);
        legacy.push(0);
        put_u32(&mut legacy, raw.len() as u32);
        legacy.extend_from_slice(&raw);
        finish_checksum(&mut legacy);
        fs::write(storage.chunk_path(position), legacy).unwrap();
        let player = PlayerRecord {
            player_id: "local-player".into(),
            revision: 4,
            components: vec![PlayerComponent {
                id: "sample:player/state".into(),
                schema_version: 1,
                payload: vec![4, 2],
            }],
            recovered_from_checkpoint: false,
        };
        storage.store_player(&player).unwrap();

        let loaded = storage.load_chunk(position).unwrap();
        assert_eq!(loaded.sections[0].states[0], ("sample:stone".into(), 77));
        assert!(loaded.spatial_records.is_empty());
        assert!(loaded.spatial_tombstones.is_empty());
        assert_eq!(
            storage.load_player("local-player").unwrap().unwrap(),
            player
        );
        assert!(storage.load_world_state().unwrap().is_none());

        storage.store_chunk(&loaded).unwrap();
        let upgraded = fs::read(storage.chunk_path(position)).unwrap();
        assert_eq!(
            u32::from_le_bytes(upgraded[8..12].try_into().unwrap()),
            CHUNK_FORMAT_VERSION
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn world_state_checkpoints_recover_previous_and_reject_both_corrupt() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "global").unwrap();
        let record = |revision, value| WorldStateRecord {
            revision,
            components: vec![WorldStateComponent {
                id: "sample:world/clock".into(),
                schema_version: 1,
                payload: vec![value],
            }],
            recovered_from_checkpoint: false,
        };
        storage.store_world_state(&record(1, 1)).unwrap();
        storage.store_world_state(&record(2, 2)).unwrap();
        assert_eq!(storage.load_world_state().unwrap().unwrap().revision, 2);

        fs::write(storage.world_state_path(0), b"corrupt-newest").unwrap();
        let recovered = storage.load_world_state().unwrap().unwrap();
        assert_eq!(recovered.revision, 1);
        assert!(recovered.recovered_from_checkpoint);

        storage.store_world_state(&record(2, 2)).unwrap();
        let mut bad_checksum = fs::read(storage.world_state_path(0)).unwrap();
        bad_checksum[20] ^= 0x40;
        fs::write(storage.world_state_path(0), bad_checksum).unwrap();
        let recovered_checksum = storage.load_world_state().unwrap().unwrap();
        assert_eq!(recovered_checksum.revision, 1);
        assert!(recovered_checksum.recovered_from_checkpoint);

        fs::write(storage.world_state_path(1), b"corrupt-previous").unwrap();
        assert!(matches!(
            storage.load_world_state(),
            Err(WorldError::InvalidData(_))
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn world_state_worker_coalesces_to_newest_revision() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "global_worker").unwrap();
        let record = |revision| WorldStateRecord {
            revision,
            components: vec![WorldStateComponent {
                id: "sample:world/clock".into(),
                schema_version: 1,
                payload: revision.to_le_bytes().to_vec(),
            }],
            recovered_from_checkpoint: false,
        };
        let scheduler = WorldStateSaveScheduler::new();
        scheduler.submit(storage.clone(), record(1)).unwrap();
        scheduler.submit(storage.clone(), record(2)).unwrap();
        scheduler.submit(storage.clone(), record(3)).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while storage
            .load_world_state()
            .unwrap()
            .is_none_or(|saved| saved.revision < 3)
        {
            let _ = scheduler.take_completed();
            assert!(
                std::time::Instant::now() < deadline,
                "world-state worker did not persist newest revision"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(storage.load_world_state().unwrap().unwrap(), record(3));
        assert!(scheduler.metrics().coalesced >= 1);
        drop(scheduler);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn world_state_worker_reports_failure_and_accepts_retry() {
        let root = temp_root();
        let storage = WorldStorage::open(&root, "global_retry").unwrap();
        let component = WorldStateComponent {
            id: "sample:world/clock".into(),
            schema_version: 1,
            payload: 7u64.to_le_bytes().to_vec(),
        };
        let invalid = WorldStateRecord {
            revision: 1,
            components: vec![component.clone(), component.clone()],
            recovered_from_checkpoint: false,
        };
        let scheduler = WorldStateSaveScheduler::new();
        scheduler.submit(storage.clone(), invalid).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let completed = scheduler.take_completed();
            if let Some(completion) = completed.first() {
                assert!(completion.result.is_err());
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(storage.load_world_state().unwrap().is_none());
        let retry = WorldStateRecord {
            revision: 2,
            components: vec![component],
            recovered_from_checkpoint: false,
        };
        scheduler.submit(storage.clone(), retry.clone()).unwrap();
        while storage.load_world_state().unwrap().is_none() {
            let _ = scheduler.take_completed();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(storage.load_world_state().unwrap().unwrap(), retry);
        drop(scheduler);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn spatial_and_world_component_limits_fail_without_allocation_or_panic() {
        let duplicate = EntityId::from_parts(1, 1);
        let invalid = StoredChunk {
            position: ChunkPos { x: 0, z: 0 },
            sections: Vec::new(),
            spatial_records: vec![
                SpatialRecord {
                    entity_id: duplicate,
                    entity_revision: 1,
                    entity_type: "sample:entity/item".into(),
                    schema_version: 1,
                    payload: Vec::new(),
                },
                SpatialRecord {
                    entity_id: duplicate,
                    entity_revision: 2,
                    entity_type: "sample:entity/item".into(),
                    schema_version: 1,
                    payload: Vec::new(),
                },
            ],
            spatial_tombstones: Vec::new(),
        };
        assert!(matches!(
            encode_chunk_file(&invalid),
            Err(WorldError::InvalidData(
                "zero or duplicate spatial entity id"
            ))
        ));
        let duplicate_components = WorldStateRecord {
            revision: 1,
            components: vec![
                WorldStateComponent {
                    id: "sample:world/x".into(),
                    schema_version: 1,
                    payload: Vec::new(),
                },
                WorldStateComponent {
                    id: "sample:world/x".into(),
                    schema_version: 1,
                    payload: Vec::new(),
                },
            ],
            recovered_from_checkpoint: false,
        };
        let root = temp_root();
        let storage = WorldStorage::open(&root, "invalid_global").unwrap();
        assert!(storage.store_world_state(&duplicate_components).is_err());
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod c1_radius_tests {
    use super::*;
    #[test]
    fn live_radius_changes_preserve_tokens_negative_coordinates_and_pins() {
        let center = ChunkPos { x: -1, z: -1 };
        let old = ChunkPos { x: 8, z: -1 };
        let residents = HashSet::from([center, old]);
        let pins = HashSet::from([old]);
        let mut r = WorldResidency::new(3, 1);
        let first = r.update(center, &residents, &pins);
        let request = first.requests.first().copied().unwrap();
        r.set_radii(6, 7).unwrap();
        let grown = r.update(center, &residents, &pins);
        assert!(r.is_current(request));
        assert!(!grown.evict.contains(&old));
        assert_eq!(r.desired_column_count(), 169);
        r.set_radii(3, 3).unwrap();
        let shrunk = r.update(center, &residents, &pins);
        assert!(!shrunk.evict.contains(&old));
        assert_eq!(r.desired_column_count(), 49);
        // Only after the existing owner clears a save/entity pin may the candidate be evicted.
        let saved = r.update(center, &residents, &HashSet::new());
        assert!(saved.evict.contains(&old));
        assert!(!saved.evict.contains(&center));
        assert!(r.set_radii(6, 5).is_err());
        assert_eq!((r.load_radius(), r.retain_radius()), (3, 3));
        r.set_radii(12, 16).unwrap();
        assert!(r.update(center, &residents, &pins).requests.len() > 500);
        r.set_radii(3, 4).unwrap();
        r.update(center, &residents, &pins);
        assert_eq!(r.desired_column_count(), 49);
    }
}
