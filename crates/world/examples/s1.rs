//! S1 disposable physical-backend evaluation. Never used by normal saves.
#![recursion_limit = "256"]
use rustcraft_engine_core::{CHUNK_VOLUME, EntityId};
use rustcraft_world::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path, time::Instant};

mod support;
use support::*;
fn run(columns: usize, cycles: usize, output: &Path) -> Value {
    assert!((16..=100_000).contains(&columns));
    assert!((1..=200).contains(&cycles));
    let root = output.parent().unwrap().join(format!(
        "rustcraft-s1-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = WorldStorage::open(&root, "current").unwrap();
    store
        .store_metadata(&WorldMetadata {
            seed: 731173,
            game_id: "test_a:game".into(),
            profile_fingerprint: "s1-synthetic".into(),
            persistence_schema_version: 1,
            generator_id: "test_a:fixture".into(),
            generator_version: 1,
        })
        .unwrap();
    let mut creation = Writes::default();
    let started = Instant::now();
    let mut scale = Vec::new();
    for i in 0..columns {
        let c = column(i);
        creation.save(&store, &c, (8 * CHUNK_VOLUME * 2 + 96) as u64);
        if [1000, 10000, 50000, 100000].contains(&(i + 1)) {
            scale.push(json!({"columns":i+1,"objects":objects(store.root())}));
        }
    }
    let creation_wall = started.elapsed().as_secs_f64();
    fs::write(output, serde_json::to_vec_pretty(&json!({"status":"creation complete; remaining phases pending","columns":columns,"creation":creation.summary(),"creation_wall_s":creation_wall,"file_scaling":scale})).unwrap()).unwrap();
    eprintln!("S1 baseline created {columns} columns in {creation_wall:.3}s");
    let mut edits = Writes::default();
    let mut entities = Writes::default();
    let mut dense = Writes::default();
    let mut transfers = Writes::default();
    let subset = (columns / 100).max(1);
    for n in 0..subset {
        let i = n * 97 % columns;
        let mut c = store.load_chunk(pos(i)).unwrap();
        c.sections[0].states[n % CHUNK_VOLUME].1 ^= 1;
        edits.save(&store, &c, 2);
        c.spatial_records[0].payload[0] ^= 1;
        c.spatial_records[0].entity_revision += 1;
        entities.save(&store, &c, 1);
    }
    let mut d = store.load_chunk(pos(0)).unwrap();
    d.spatial_records = (0..1000)
        .map(|n| SpatialRecord {
            entity_id: EntityId(1_000_000 + n),
            entity_revision: 1,
            entity_type: "test_c:item".into(),
            schema_version: 1,
            payload: vec![n as u8; 96],
        })
        .collect();
    store.store_chunk(&d).unwrap();
    for n in 0..100 {
        d.spatial_records[n].payload[0] ^= 1;
        d.spatial_records[n].entity_revision += 1;
        dense.save(&store, &d, 1);
    }
    // Destination-before-source with a recovery tombstone, then source cleanup, then pruning.
    for n in 0..subset {
        let mut src = store.load_chunk(pos(1 + n % (columns - 1))).unwrap();
        let mut dst = store.load_chunk(pos((2 + n) % columns)).unwrap();
        let mut entity = src.spatial_records.pop().unwrap();
        entity.entity_revision += 1;
        dst.spatial_tombstones.push(SpatialTombstone {
            entity_id: entity.entity_id,
            entity_revision: entity.entity_revision,
            source: src.position,
        });
        dst.spatial_records.push(entity);
        transfers.save(&store, &dst, 96 + 32);
        transfers.save(&store, &src, 96);
        dst.spatial_tombstones.clear();
        transfers.save(&store, &dst, 32);
    }
    let mut checkpoint = Samples::default();
    let (mut checkpoint_bytes, mut checkpoint_write, mut checkpoint_sync) =
        (0u64, Samples::default(), Samples::default());
    let mut components = (0..48)
        .map(|n| WorldStateComponent {
            id: format!("test_{}:component_{n}", ['a', 'b', 'c'][n % 3]),
            schema_version: n as u32 + 1,
            payload: vec![n as u8; 64 + (n % 4) * 64],
        })
        .collect::<Vec<_>>();
    components.sort_by(|a, b| a.id.cmp(&b.id));
    for rev in 1..=128 {
        let start = Instant::now();
        let m = store
            .store_world_state_measured(&WorldStateRecord {
                revision: rev,
                components: components.clone(),
                recovered_from_checkpoint: false,
            })
            .unwrap();
        checkpoint.record(start);
        checkpoint_bytes += m.encoded_bytes as u64;
        checkpoint_write.0.push(m.write_ms);
        checkpoint_sync.0.push(m.sync_ms);
        let start = Instant::now();
        let m = store
            .store_player_measured(&PlayerRecord {
                player_id: "operator".into(),
                revision: rev,
                components: components
                    .iter()
                    .map(|c| PlayerComponent {
                        id: c.id.clone(),
                        schema_version: c.schema_version,
                        payload: c.payload.clone(),
                    })
                    .collect(),
                recovered_from_checkpoint: false,
            })
            .unwrap();
        checkpoint.record(start);
        checkpoint_bytes += m.encoded_bytes as u64;
        checkpoint_write.0.push(m.write_ms);
        checkpoint_sync.0.push(m.sync_ms);
    }
    let mut r = store.load_world_state().unwrap().unwrap();
    let unknown = r.components[17].clone();
    r.components[0].payload[0] ^= 1;
    r.revision += 1;
    store.store_world_state(&r).unwrap();
    assert_eq!(
        store.load_world_state().unwrap().unwrap().components[17],
        unknown
    );
    let mut churn = Writes::default();
    let mut growth = Vec::new();
    for cycle in 0..cycles {
        for n in 0..subset {
            let i = (n * 97 + cycle * 13) % columns;
            let mut c = store.load_chunk(pos(i)).unwrap();
            c.sections[0].states[(n + cycle) % CHUNK_VOLUME].1 ^= 1;
            if let Some(e) = c.spatial_records.first_mut() {
                e.payload[0] ^= 1;
                e.entity_revision += 1;
            }
            churn.save(&store, &c, 3);
        }
        if cycle == 0 || (cycle + 1).is_multiple_of(10) || cycle + 1 == cycles {
            growth.push(json!({"cycle":cycle+1,"objects":objects(store.root())}));
        }
    }
    let start = Instant::now();
    drop(store);
    let store = WorldStorage::open(&root, "current").unwrap();
    let reopen_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut read_io = Samples::default();
    let mut decompress = Samples::default();
    let mut decode = Samples::default();
    let mut random = Samples::default();
    let mut sequential = Samples::default();
    let mut entity_read = Samples::default();
    for n in 0..columns.min(2000) {
        let i = (n * 7919) % columns;
        let start = Instant::now();
        let (c, m) = store.load_chunk_measured(pos(i)).unwrap();
        read_io.0.push(m.read_ms);
        decompress.0.push(m.decompression_ms);
        decode.0.push(m.decode_ms);
        random.record(start);
        assert_eq!(c.position, pos(i));
        let start = Instant::now();
        let _ = c
            .spatial_records
            .iter()
            .find(|e| e.entity_id == EntityId(i as u128 + 1));
        entity_read.record(start);
    }
    for n in 0..columns.min(1000) {
        let start = Instant::now();
        store.load_chunk(pos(n)).unwrap();
        sequential.record(start);
    }
    // Production bounded workers, finite dirty source; record admission-to-durable acknowledgement.
    let mut worker = SaveScheduler::new(1, 8);
    let start = Instant::now();
    let mut admitted = 0;
    let mut completed = 0;
    let mut arrivals = BTreeMap::new();
    let mut queue = Samples::default();
    let mut queue_wait = Samples::default();
    let mut peak = 0;
    let mut oldest_ms = 0f64;
    while completed < columns.min(1000) {
        for done in worker.take_completed() {
            done.result.unwrap();
            queue_wait.0.push(done.queue_wait_ms);
            queue.record(arrivals.remove(&done.token.generation).unwrap());
            completed += 1;
        }
        if admitted < columns.min(1000) {
            let c = store.load_chunk(pos(admitted)).unwrap();
            let token = SaveToken {
                position: c.position,
                generation: admitted as u64 + 1,
            };
            let t = Instant::now();
            if worker.submit(store.clone(), token, c).is_ok() {
                arrivals.insert(token.generation, t);
                admitted += 1;
            }
        }
        let m = worker.metrics();
        peak = peak.max(m.queued + m.in_flight);
        if let Some(t) = arrivals.values().min() {
            oldest_ms = oldest_ms.max(t.elapsed().as_secs_f64() * 1000.0);
        }
        std::thread::yield_now();
        assert!(start.elapsed().as_secs() < 120);
    }
    let worker_wall = start.elapsed().as_secs_f64();
    drop(worker);
    let start = Instant::now();
    store.flush().unwrap();
    let flush_ms = start.elapsed().as_secs_f64() * 1000.0;
    let process_read = if cfg!(test) || columns < 100 {
        Value::Null
    } else {
        let child = std::process::Command::new(
            std::env::args_os()
                .next()
                .expect("benchmark executable argument"),
        )
        .arg("--read-existing")
        .arg(store.root())
        .output()
        .unwrap();
        assert!(
            child.status.success(),
            "{}",
            String::from_utf8_lossy(&child.stderr)
        );
        serde_json::from_slice::<Value>(&child.stdout).unwrap()
    };
    let paced = paced_queue(&store, columns);
    let sync_probe = sync_calibration(store.root());
    let result = json!({"paced_queue":paced,"isolated_sync_calibration":sync_probe,"process_reopen":process_read,"schema":1,"backend":"current_per_column","columns":columns,"entities_initial":columns,"cycles":cycles,"sections_per_column":8,"cache":"fresh-directory writes; warm and process-storage-reopen reads, OS cache not dropped","compression":"zlib fast if smaller","durability":"atomicwrites file sync + replacement + Unix directory sync; Windows directory sync unsupported",
        "config":rustcraft_config::settings::engine(false).snapshot(),"creation_wall_s":creation_wall,"creation":creation.summary(),"file_scaling":scale,"random_edits":edits.summary(),"entity_only_sparse":entities.summary(),"entity_only_dense":dense.summary(),"transfers":transfers.summary(),"checkpoints":{"latency":checkpoint.summary(),"write":checkpoint_write.summary(),"sync_replace_envelope":checkpoint_sync.summary(),"application_write_bytes":checkpoint_bytes,"file_sync_calls":512,"directory_sync_calls_unix":768},"components":{"namespaces":3,"count":48,"unknown_preserved":true},"churn":churn.summary(),"storage_trend":growth,"read_io":read_io.summary(),"decompression":decompress.summary(),"decode":decode.summary(),"random_read":random.summary(),"sequential_read":sequential.summary(),"entity_lookup_after_decode":entity_read.summary(),"reopen_ms":reopen_ms,"startup_global_index_bytes":0,"queue":{"workers":1,"per_worker_capacity":8,"peak_pending_inflight":peak,"ack_latency":queue.summary(),"queue_wait":queue_wait.summary(),"oldest_admitted_ms":oldest_ms,"wall_s":worker_wall,"writes_per_s":completed as f64/worker_wall,"drained":completed==admitted,"remaining":arrivals.len()},"flush_ms":flush_ms,"final_objects":objects(store.root())});
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    fs::write(output, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    drop(store);
    fs::remove_dir_all(root).unwrap();
    result
}
fn process_reopen(path: &Path) -> Value {
    let start = Instant::now();
    let store = WorldStorage::open(
        path.parent().unwrap(),
        path.file_name().unwrap().to_str().unwrap(),
    )
    .unwrap();
    store.load_metadata().unwrap();
    let open_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut read = Samples::default();
    for i in 0..100 {
        let start = Instant::now();
        store.load_chunk(pos(i)).unwrap();
        read.record(start);
    }
    json!({"open_metadata_ms":open_ms,"first_100_reads":read.summary(),"cache":"new process, OS cache retained"})
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|s| s == "--read-existing") {
        println!("{}", process_reopen(Path::new(&args[2])));
        return;
    }
    if args.get(1).is_some_and(|s| s == "--auxiliary") {
        println!("{}", auxiliary(Path::new(&args[2])));
        return;
    }
    let columns = args.get(1).map_or(1000, |s| s.parse().unwrap());
    let cycles = args.get(2).map_or(100, |s| s.parse().unwrap());
    let output = args.get(3).map_or("target/s1/current.json", String::as_str);
    let result = run(columns, cycles, Path::new(output));
    println!(
        "S1 complete: {} columns; results {output}",
        result["columns"]
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn s1_correctness() {
        let path = std::env::temp_dir().join(format!("s1-results-{}.json", std::process::id()));
        let r = run(16, 2, &path);
        assert_eq!(r["queue"]["remaining"], 0);
        fs::remove_file(path).unwrap();
    }
}
