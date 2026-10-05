//! Benchmark-only split terrain/entity physical model. No production backend registration.
#![recursion_limit = "256"]
use rustcraft_engine_core::EntityId;
use rustcraft_world::*;
use serde_json::{Value, json};
use std::{fs, path::Path, time::Instant};
mod support;
use support::*;
fn split(mut c: StoredChunk) -> (StoredChunk, StoredChunk) {
    let entity = StoredChunk {
        position: c.position,
        sections: vec![],
        spatial_records: std::mem::take(&mut c.spatial_records),
        spatial_tombstones: std::mem::take(&mut c.spatial_tombstones),
    };
    (c, entity)
}
fn run(columns: usize, cycles: usize, output: &Path) -> Value {
    assert!((16..=100_000).contains(&columns));
    assert!((1..=200).contains(&cycles));
    let root = output.parent().unwrap().join(format!(
        "rustcraft-s1-split-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let terrain = WorldStorage::open(&root, "terrain").unwrap();
    let entities = WorldStorage::open(&root, "entities").unwrap();
    let mut creation = Writes::default();
    let start = Instant::now();
    for i in 0..columns {
        let (t, e) = split(column(i));
        creation.save(&terrain, &t, (8 * 4096 * 2) as u64);
        creation.save(&entities, &e, 96);
    }
    let creation_wall = start.elapsed().as_secs_f64();
    eprintln!("S1 split created {columns} in {creation_wall:.3}s");
    let mut edits = Writes::default();
    let mut sparse = Writes::default();
    let mut dense = Writes::default();
    let mut transfers = Writes::default();
    let subset = (columns / 100).max(1);
    for n in 0..subset {
        let i = n * 97 % columns;
        let mut t = terrain.load_chunk(pos(i)).unwrap();
        t.sections[0].states[n % 4096].1 ^= 1;
        edits.save(&terrain, &t, 2);
        let mut e = entities.load_chunk(pos(i)).unwrap();
        e.spatial_records[0].payload[0] ^= 1;
        e.spatial_records[0].entity_revision += 1;
        sparse.save(&entities, &e, 1);
    }
    let mut d = entities.load_chunk(pos(0)).unwrap();
    d.spatial_records = (0..1000)
        .map(|n| SpatialRecord {
            entity_id: EntityId(1_000_000 + n),
            entity_revision: 1,
            entity_type: "test_c:item".into(),
            schema_version: 1,
            payload: vec![n as u8; 96],
        })
        .collect();
    entities.store_chunk(&d).unwrap();
    for n in 0..100 {
        d.spatial_records[n].payload[0] ^= 1;
        d.spatial_records[n].entity_revision += 1;
        dense.save(&entities, &d, 1);
    }
    for n in 0..subset {
        let mut src = entities.load_chunk(pos(1 + n % (columns - 1))).unwrap();
        let mut dst = entities.load_chunk(pos((2 + n) % columns)).unwrap();
        let mut entity = src.spatial_records.pop().unwrap();
        entity.entity_revision += 1;
        dst.spatial_tombstones.push(SpatialTombstone {
            entity_id: entity.entity_id,
            entity_revision: entity.entity_revision,
            source: src.position,
        });
        dst.spatial_records.push(entity);
        transfers.save(&entities, &dst, 128);
        transfers.save(&entities, &src, 96);
        dst.spatial_tombstones.clear();
        transfers.save(&entities, &dst, 32);
    }
    let mut churn = Writes::default();
    let mut trend = Vec::new();
    for cycle in 0..cycles {
        for n in 0..subset {
            let i = (n * 97 + cycle * 13) % columns;
            let mut t = terrain.load_chunk(pos(i)).unwrap();
            t.sections[0].states[(n + cycle) % 4096].1 ^= 1;
            churn.save(&terrain, &t, 2);
            let mut e = entities.load_chunk(pos(i)).unwrap();
            if let Some(e) = e.spatial_records.first_mut() {
                e.payload[0] ^= 1;
                e.entity_revision += 1;
            }
            churn.save(&entities, &e, 1);
        }
        if cycle == 0 || (cycle + 1).is_multiple_of(10) || cycle + 1 == cycles {
            trend.push(json!({"cycle":cycle+1,"objects":objects(&root)}));
        }
    }
    let mut read = Samples::default();
    let mut entity_read = Samples::default();
    for n in 0..columns.min(2000) {
        let i = n * 7919 % columns;
        let start = Instant::now();
        terrain.load_chunk(pos(i)).unwrap();
        entities.load_chunk(pos(i)).unwrap();
        read.record(start);
        let start = Instant::now();
        entities.load_chunk(pos(i)).unwrap();
        entity_read.record(start);
    }
    let start = Instant::now();
    let _ = WorldStorage::open(&root, "terrain").unwrap();
    let _ = WorldStorage::open(&root, "entities").unwrap();
    let reopen = start.elapsed().as_secs_f64() * 1000.0;
    // Conversion spike at 1k columns, source retained until full semantic validation.
    let source = WorldStorage::open(&root, "migration_source").unwrap();
    let target_t = WorldStorage::open(&root, "migration_terrain").unwrap();
    let target_e = WorldStorage::open(&root, "migration_entities").unwrap();
    let migration_count = columns.min(1000);
    for i in 0..migration_count {
        source.store_chunk(&column(i)).unwrap();
    }
    let source_bytes = objects(source.root()).2;
    let start = Instant::now();
    let mut migration = Writes::default();
    for i in 0..migration_count {
        let c = source.load_chunk(pos(i)).unwrap();
        let (t, e) = split(c);
        migration.save(&target_t, &t, 8 * 4096 * 2);
        migration.save(&target_e, &e, 96);
    }
    let convert_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    for i in 0..migration_count {
        let mut t = target_t.load_chunk(pos(i)).unwrap();
        let e = target_e.load_chunk(pos(i)).unwrap();
        t.spatial_records = e.spatial_records;
        t.spatial_tombstones = e.spatial_tombstones;
        assert!(
            t == source.load_chunk(pos(i)).unwrap(),
            "migration column {i} mismatch"
        );
    }
    let validation_ms = start.elapsed().as_secs_f64() * 1000.0;
    let destination_bytes = objects(target_t.root()).2 + objects(target_e.root()).2;
    // Interrupted build never mutates source; there is no production switch/migration code.
    let mut corrupt = fs::read(target_e.root().join("chunks/-50.-50.rcc")).unwrap();
    corrupt[0] ^= 1;
    fs::write(target_e.root().join("chunks/-50.-50.rcc"), corrupt).unwrap();
    assert!(target_e.load_chunk(pos(0)).is_err());
    assert!(
        source.load_chunk(pos(0)).unwrap() == column(0),
        "conversion source changed"
    );
    drop(source);
    drop(target_t);
    drop(target_e);
    for name in [
        "migration_source",
        "migration_terrain",
        "migration_entities",
    ] {
        fs::remove_dir_all(root.join(name)).unwrap();
    }
    let paced = paced_queue(&entities, columns);
    let sync = sync_calibration(entities.root());
    let start = Instant::now();
    terrain.flush().unwrap();
    entities.flush().unwrap();
    let flush_ms = start.elapsed().as_secs_f64() * 1000.0;
    let result = json!({"schema":1,"backend":"benchmark_only_split_blobs","columns":columns,"cycles":cycles,"durability":"same AtomicFile sync/replace per blob; ordered transfer as current; no cross-blob transaction","creation_wall_s":creation_wall,"creation":creation.summary(),"random_edits":edits.summary(),"entity_only_sparse":sparse.summary(),"entity_only_dense":dense.summary(),"transfers":transfers.summary(),"churn":churn.summary(),"storage_trend":trend,"random_read":read.summary(),"entity_lookup_physical":entity_read.summary(),"reopen_ms":reopen,"startup_global_index_bytes":0,"paced_queue":paced,"isolated_sync_calibration":sync,"flush_ms":flush_ms,"final_objects":objects(&root),"migration":{"columns":migration_count,"convert_ms":convert_ms,"validate_ms":validation_ms,"source_bytes":source_bytes,"destination_bytes":destination_bytes,"peak_bytes":source_bytes+destination_bytes,"source_intact":true,"all_columns_semantically_validated":true,"application_io":migration.summary()},"recovery":"same bounded checksummed blobs and transfer order; corrupted conversion destination rejected; source valid","compaction":"not applicable; replacement","limitations":"additional file/recovery domains; terrain and entity changes are separate durable operations; not a production-ready backend"});
    fs::write(output, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    drop(terrain);
    drop(entities);
    fs::remove_dir_all(root).unwrap();
    result
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|s| s == "--auxiliary") {
        println!("{}", auxiliary(Path::new(&args[2])));
        return;
    }
    let n = args.get(1).map_or(1000, |s| s.parse().unwrap());
    let cycles = args.get(2).map_or(100, |s| s.parse().unwrap());
    let output = args.get(3).map_or("target/s1/split.json", String::as_str);
    let r = run(n, cycles, Path::new(output));
    println!("S1 split complete: {} columns", r["columns"]);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn s1_split_roundtrip() {
        let p = std::env::temp_dir().join(format!("s1-split-results-{}.json", std::process::id()));
        let r = run(16, 2, &p);
        assert_eq!(r["migration"]["all_columns_semantically_validated"], true);
        fs::remove_file(p).unwrap();
    }
}
