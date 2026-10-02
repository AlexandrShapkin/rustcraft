//! Generic deterministic generation contracts and versioned world storage.
//! This crate stores semantic identities and never interprets game-specific terrain policy.

use rustcraft_engine_core::{BlockId, BlockState, CHUNK_VOLUME, Chunk, ChunkPos};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File, OpenOptions},
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
pub const WORLD_FORMAT_VERSION: u32 = 1;
pub const WORLD_METADATA_VERSION: u32 = 2;
pub const PERSISTED_STATE_SCHEMA_VERSION: u32 = 1;
pub const CHUNK_FORMAT_VERSION: u32 = 2;
pub const MAX_METADATA_BYTES: usize = 64 * 1024;
pub const MAX_CHUNK_BYTES: usize = 2 * 1024 * 1024;
pub const PLAYER_RECORD_VERSION: u32 = 2;
const LEGACY_PLAYER_RECORD_VERSION: u32 = 1;
pub const MAX_PLAYER_RECORD_BYTES: usize = 64 * 1024;
pub const MAX_PLAYER_COMPONENTS: usize = 64;
pub const MAX_PLAYER_COMPONENT_ID_BYTES: usize = 256;
pub const MAX_PLAYER_COMPONENT_BYTES: usize = 32 * 1024;
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
pub trait SemanticBlockResolver {
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

struct GenerationJob {
    generator: Arc<dyn ChunkGenerator>,
    seed: i64,
    position: ChunkPos,
    generation: u64,
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
    results: Receiver<GeneratedColumn>,
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
    results: Receiver<SaveCompletion>,
    next_worker: usize,
    queued: Arc<AtomicUsize>,
    in_flight: Arc<AtomicUsize>,
    completed: Arc<AtomicU64>,
    failed: Arc<AtomicU64>,
}

impl SaveScheduler {
    pub fn new(worker_count: usize, queue_capacity_per_worker: usize) -> Self {
        let (result_tx, results) = mpsc::channel();
        let queued = Arc::new(AtomicUsize::new(0));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicU64::new(0));
        let mut workers = Vec::new();
        for _ in 0..worker_count.max(1) {
            let (sender, receiver) =
                mpsc::sync_channel::<SaveJob>(queue_capacity_per_worker.max(1));
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
            results,
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
        while let Ok(completion) = self.results.try_recv() {
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
    pub fn dirty_count(&self) -> usize {
        self.dirty.len()
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
        let (result_tx, results) = mpsc::channel();
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
            results,
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
        let mut ready = Vec::new();
        while let Ok(result) = self.results.try_recv() {
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
        if version != CHUNK_FORMAT_VERSION {
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
        r.finish()?;
        Ok(StoredChunk { position, sections })
    }

    pub fn load_runtime_chunk(
        &self,
        position: ChunkPos,
        resolver: &impl SemanticBlockResolver,
    ) -> Result<Vec<(i32, Chunk)>, WorldError> {
        let stored = self.load_chunk(position)?;
        stored
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
            .collect()
    }

    pub fn encode_runtime_chunk(
        position: ChunkPos,
        sections: impl IntoIterator<Item = (i32, Chunk)>,
        resolver: &impl SemanticBlockResolver,
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
        })
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

fn encode_chunk_file(chunk: &StoredChunk) -> Result<(Vec<u8>, ChunkEncodingMetrics), WorldError> {
    if chunk.sections.len() > MAX_SECTIONS {
        return Err(WorldError::InvalidData("too many sections"));
    }
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
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    let write_started = std::time::Instant::now();
    file.write_all(data)?;
    let write_ms = write_started.elapsed().as_secs_f64() * 1000.0;
    let sync_started = std::time::Instant::now();
    file.sync_all()?;
    drop(file);
    if let Some(parent) = path.parent() {
        // Persist a newly created directory entry where directory sync is supported. Some
        // platforms reject opening directories; the checkpoint file itself is still synced.
        sync_directory(parent)?;
    }
    let sync_ms = sync_started.elapsed().as_secs_f64() * 1000.0;
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
}
