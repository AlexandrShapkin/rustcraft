use rustcraft_engine_core::{CHUNK_VOLUME, ChunkPos, EntityId};
use rustcraft_world::*;
use serde_json::{Value, json};
use std::{fs, path::Path, time::Instant};
#[derive(Default)]
pub struct Samples(pub Vec<f64>);
impl Samples {
    pub fn record(&mut self, start: Instant) {
        self.0.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    pub fn summary(&self) -> Value {
        let mut a = self.0.clone();
        a.sort_by(f64::total_cmp);
        if a.is_empty() {
            return Value::Null;
        }
        let p = |v: f64| a[((a.len() - 1) as f64 * v).round() as usize];
        json!({"count":a.len(),"sum_ms":a.iter().sum::<f64>(),"p50_ms":p(0.5),"p95_ms":p(0.95),"p99_ms":p(0.99),"max_ms":a.last()})
    }
}
#[derive(Default)]
pub struct Writes {
    latency: Samples,
    encode: Samples,
    compress: Samples,
    write: Samples,
    durability: Samples,
    logical: u64,
    raw: u64,
    bytes: u64,
}
impl Writes {
    pub fn save(&mut self, store: &WorldStorage, column: &StoredChunk, changed: u64) {
        let start = Instant::now();
        let m = store.store_chunk_measured(column).unwrap();
        self.latency.record(start);
        self.encode.0.push(m.encode_ms);
        self.compress.0.push(m.compression_ms);
        self.write.0.push(m.write_ms);
        self.durability.0.push(m.durability_ms);
        self.logical += changed;
        self.raw += m.raw_payload_bytes as u64;
        self.bytes += m.stored_file_bytes as u64;
    }
    pub fn summary(&self) -> Value {
        json!({"logical_changed_bytes":self.logical,"encoded_raw_bytes":self.raw,"application_write_bytes":self.bytes,
        "write_amplification":self.bytes as f64/self.logical.max(1) as f64,"operation":self.latency.summary(),"encode":self.encode.summary(),"compression":self.compress.summary(),"write_all":self.write.summary(),"durability_envelope":self.durability.summary(),
        "file_sync_calls":self.latency.0.len(),"directory_sync_calls_unix":self.latency.0.len()*3,"atomic_replaces":self.latency.0.len(),"write_all_calls":self.latency.0.len()})
    }
}
pub fn pos(i: usize) -> ChunkPos {
    ChunkPos {
        x: (i % 100) as i32 - 50,
        z: (i / 100) as i32 - 50,
    }
}
pub fn column(i: usize) -> StoredChunk {
    let mut seed = i as u64 + 731173;
    let sections = (0..8)
        .map(|y| StoredSection {
            y,
            states: (0..CHUNK_VOLUME)
                .map(|v| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    // Layered strata, caves, varied palette and nonzero state variants. No worldgen cost.
                    let k = if y >= 5 {
                        0
                    } else if y == 4 {
                        1 + (v / 256) % 3
                    } else if seed >> 60 == 0 {
                        0
                    } else {
                        4 + ((seed >> 56) % 5) as usize
                    };
                    (
                        format!("test_a:block_{k}"),
                        (seed >> 48) as u16 % if i.is_multiple_of(7) { 8 } else { 2 },
                    )
                })
                .collect(),
        })
        .collect();
    StoredChunk {
        position: pos(i),
        sections,
        spatial_records: vec![SpatialRecord {
            entity_id: EntityId(i as u128 + 1),
            entity_revision: 1,
            entity_type: "test_b:item".into(),
            schema_version: 1,
            payload: (0..96).map(|b| ((i + b) % 251) as u8).collect(),
        }],
        spatial_tombstones: vec![],
    }
}
pub fn objects(path: &Path) -> (u64, u64, u64) {
    let (mut files, mut dirs, mut bytes) = (0, 0, 0);
    for e in fs::read_dir(path).unwrap() {
        let e = e.unwrap();
        let m = e.metadata().unwrap();
        if m.is_dir() {
            let (f, d, b) = objects(&e.path());
            files += f;
            dirs += d + 1;
            bytes += b;
        } else {
            files += 1;
            bytes += m.len();
        }
    }
    (files, dirs, bytes)
}

/// Isolated sync calibration on the benchmark filesystem, not per-column fsync telemetry.
pub fn sync_calibration(root: &Path) -> Value {
    use std::io::Write;
    let path = root.join("sync-calibration");
    let mut file = fs::File::create(&path).unwrap();
    let mut write = Samples::default();
    let mut sync = Samples::default();
    #[cfg(unix)]
    let mut directory = Samples::default();
    #[cfg(not(unix))]
    let directory = Samples::default();
    for n in 0..128 {
        let start = Instant::now();
        file.write_all(&[n as u8; 4096]).unwrap();
        write.record(start);
        let start = Instant::now();
        file.sync_all().unwrap();
        sync.record(start);
        #[cfg(unix)]
        {
            let start = Instant::now();
            fs::File::open(root).unwrap().sync_all().unwrap();
            directory.record(start);
        }
    }
    drop(file);
    fs::remove_file(path).unwrap();
    json!({"scope":"isolated 4KiB append and sync; not actual atomic-write internal fsync timing","file_sync":sync.summary(),"write":write.summary(),"directory_sync_unix":directory.summary()})
}

/// Finite server-like source at 50 dirty columns/s with production 1x8 worker capacity.
/// Backlog contains IDs/timestamps only and admission stays nonblocking.
pub fn paced_queue(store: &WorldStorage, columns: usize) -> Value {
    use std::{
        collections::{BTreeMap, VecDeque},
        time::Duration,
    };
    let total = columns.min(300);
    let mut scheduler = SaveScheduler::new(1, 8);
    let start = Instant::now();
    let mut produced = 0;
    let mut completed = 0;
    let mut pending = VecDeque::new();
    let mut inflight = BTreeMap::new();
    let mut waits = Samples::default();
    let mut dirty_age = Samples::default();
    let mut peak = 0;
    let mut trends = Vec::new();
    while completed < total {
        while produced < total && start.elapsed() >= Duration::from_millis(produced as u64 * 20) {
            pending.push_back((
                produced,
                start + Duration::from_millis(produced as u64 * 20),
            ));
            produced += 1;
        }
        for done in scheduler.take_completed() {
            done.result.unwrap();
            waits.0.push(done.queue_wait_ms);
            dirty_age.record(inflight.remove(&done.token.generation).unwrap());
            completed += 1;
        }
        if let Some(&(i, born)) = pending.front() {
            let c = store.load_chunk(pos(i)).unwrap();
            if scheduler
                .submit(
                    store.clone(),
                    SaveToken {
                        position: c.position,
                        generation: i as u64 + 1,
                    },
                    c,
                )
                .is_ok()
            {
                pending.pop_front();
                inflight.insert(i as u64 + 1, born);
            }
        }
        peak = peak.max(pending.len() + inflight.len());
        if trends.len() < completed / 50 {
            trends.push(json!({"completed":completed,"dirty":pending.len()+inflight.len()}));
        }
        assert!(start.elapsed().as_secs() < 120);
        std::thread::sleep(Duration::from_millis(1));
    }
    json!({"source_writes_per_s":50,"count":total,"workers":1,"capacity":8,"dirty_peak":peak,"queue_wait":waits.summary(),"dirty_to_ack":dirty_age.summary(),"trend":trends,"remaining":pending.len()+inflight.len(),"elapsed_s":start.elapsed().as_secs_f64()})
}

pub fn auxiliary(output: &Path) -> Value {
    let root = output.parent().unwrap().join(format!(
        "rustcraft-s1-aux-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    assert!(!root.exists());
    let store = WorldStorage::open(&root, "aux").unwrap();
    for i in 0..16 {
        store.store_chunk(&column(i)).unwrap();
    }
    let before = objects(store.root());
    let mut c = store.load_chunk(pos(0)).unwrap();
    let mut deletion = Writes::default();
    for n in 0..100 {
        c.spatial_records.push(SpatialRecord {
            entity_id: EntityId(2_000_000 + n),
            entity_revision: 1,
            entity_type: "test_c:item".into(),
            schema_version: 1,
            payload: vec![n as u8; 96],
        });
        deletion.save(&store, &c, 96);
        c.spatial_records.pop();
        deletion.save(&store, &c, 96);
    }
    assert!(
        store.load_chunk(pos(0)).unwrap() == column(0),
        "delete churn must restore column zero"
    );
    let after = objects(store.root());
    let mut recovery = Samples::default();
    let components = vec![WorldStateComponent {
        id: "test_c:unknown".into(),
        schema_version: 19,
        payload: vec![42; 512],
    }];
    for cycle in 0..20 {
        for revision in [cycle * 2 + 1, cycle * 2 + 2] {
            store
                .store_world_state(&WorldStateRecord {
                    revision,
                    components: components.clone(),
                    recovered_from_checkpoint: false,
                })
                .unwrap();
        }
        fs::write(
            store.root().join(format!("world-state.{}.rcs", cycle % 2)),
            b"incomplete",
        )
        .unwrap();
        let start = Instant::now();
        let loaded = store.load_world_state().unwrap().unwrap();
        recovery.record(start);
        assert_eq!(loaded.revision, cycle * 2 + 1);
        assert_eq!(loaded.components, components);
        assert!(loaded.recovered_from_checkpoint);
    }
    let mut workers = SaveScheduler::new(1, 8);
    let mut expected = Vec::new();
    for i in 0..8 {
        let mut c = store.load_chunk(pos(i)).unwrap();
        c.sections[0].states[0].1 ^= 1;
        workers
            .submit(
                store.clone(),
                SaveToken {
                    position: c.position,
                    generation: i as u64 + 1,
                },
                c.clone(),
            )
            .unwrap();
        expected.push(c);
    }
    let pending = workers.metrics();
    let start = Instant::now();
    let mut completions = 0;
    while completions < 8 {
        for done in workers.take_completed() {
            done.result.unwrap();
            completions += 1;
        }
        assert!(start.elapsed().as_secs() < 120);
        std::thread::yield_now();
    }
    drop(workers);
    store.flush().unwrap();
    let shutdown_ms = start.elapsed().as_secs_f64() * 1000.0;
    let reopened = WorldStorage::open(&root, "aux").unwrap();
    for c in expected {
        assert!(
            reopened.load_chunk(c.position).unwrap() == c,
            "graceful drain must publish each accepted snapshot"
        );
    }
    // Finite scalar observation cost; metadata snapshots never retain payloads.
    let mut reads = Samples::default();
    for _ in 0..128 {
        let start = Instant::now();
        for _ in 0..1024 {
            std::hint::black_box(store.io_metrics());
        }
        reads.record(start);
    }
    let mut dirty = PersistenceDirtyTracker::default();
    for i in 0..121 {
        dirty.mark_dirty(pos(i));
    }
    let mut active = Samples::default();
    for _ in 0..128 {
        let start = Instant::now();
        std::hint::black_box(dirty.oldest_dirty_ms());
        active.record(start);
    }
    let result = json!({"delete_churn":{"cycles":100,"io":deletion.summary(),"before":before,"after":after,"identity_restored":true},"checkpoint_recovery":{"count":20,"latency":recovery.summary(),"unknown_preserved":true},"shutdown":{"queued_at_start":pending.queued,"inflight_at_start":pending.in_flight,"drain_flush_ms":shutdown_ms,"acknowledged_on_reopen":8},"diagnostic_cost":{"counter_read_batch_1024":reads.summary(),"oldest_dirty_121_entries":active.summary(),"inactive":"no dirty timestamp scan; fixed accounting only on dirty/write/queue events"}});
    fs::write(output, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    drop(reopened);
    drop(store);
    fs::remove_dir_all(root).unwrap();
    result
}
