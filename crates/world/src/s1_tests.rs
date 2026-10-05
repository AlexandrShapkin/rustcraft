//! Deterministic IO interruption tests. No timing-dependent process kills or format changes.
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};
fn fixture() -> (PathBuf, WorldStorage) {
    let root = std::env::temp_dir().join(format!(
        "s1-fault-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = WorldStorage::open(&root, "fault").unwrap();
    (root, store)
}
#[test]
fn s1_atomic_publication_interruption_matrix() {
    let (root, store) = fixture();
    let position = ChunkPos { x: -3, z: 8 };
    let old = StoredChunk {
        position,
        sections: vec![StoredSection {
            y: 0,
            states: vec![("test_a:stone".into(), 2); CHUNK_VOLUME],
        }],
        spatial_records: vec![SpatialRecord {
            entity_id: EntityId(91),
            entity_revision: 1,
            entity_type: "test_b:item".into(),
            schema_version: 1,
            payload: vec![7; 96],
        }],
        spatial_tombstones: vec![],
    };
    let mut new = old.clone();
    new.spatial_records[0].entity_revision = 2;
    new.spatial_records[0].payload[0] = 42;
    let old_bytes = encode_chunk_file(&old).unwrap().0;
    let new_bytes = encode_chunk_file(&new).unwrap().0;
    let path = store.chunk_path(position);
    for phase in 0..4 {
        atomic_write(&path, &old_bytes).unwrap();
        let failed = atomicwrites::AtomicFile::new(&path, atomicwrites::AllowOverwrite).write(
            |file| -> io::Result<()> {
                if phase == 0 {
                    return Err(io::Error::other("before write"));
                }
                let half = new_bytes.len() / 2;
                file.write_all(&new_bytes[..half])?;
                if phase == 1 {
                    return Err(io::Error::other("partial temp write"));
                }
                file.write_all(&new_bytes[half..])?;
                if phase == 2 {
                    return Err(io::Error::other("before sync"));
                }
                file.sync_all()?;
                Err(io::Error::other("after file sync, before replacement"))
            },
        );
        assert!(failed.is_err());
        assert_eq!(store.load_chunk(position).unwrap(), old);
        assert_eq!(fs::read(&path).unwrap(), old_bytes);
    }
    atomic_write(&path, &new_bytes).unwrap();
    assert_eq!(store.load_chunk(position).unwrap(), new);
    assert!(fs::read_dir(store.root()).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".atomicwrite")
    }));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn s1_unknown_components_and_checkpoint_partial_publication() {
    let (root, store) = fixture();
    let mut components = (0..48)
        .map(|n| WorldStateComponent {
            id: format!("test_{}:component_{n}", ['a', 'b', 'c'][n % 3]),
            schema_version: n as u32 + 1,
            payload: vec![n as u8; 128],
        })
        .collect::<Vec<_>>();
    components.sort_by(|a, b| a.id.cmp(&b.id));
    let mut world = WorldStateRecord {
        revision: 1,
        components: components.clone(),
        recovered_from_checkpoint: false,
    };
    store.store_world_state(&world).unwrap();
    world.revision = 2;
    world.components[0].payload[0] = 255;
    store.store_world_state(&world).unwrap();
    fs::write(store.world_state_path(0), b"incomplete-checkpoint").unwrap();
    let recovered = store.load_world_state().unwrap().unwrap();
    assert_eq!(recovered.revision, 1);
    assert!(recovered.recovered_from_checkpoint);
    assert_eq!(recovered.components, components);
    let mut next = recovered;
    next.revision = 3;
    next.components[0].payload[0] = 42;
    store.store_world_state(&next).unwrap();
    assert_eq!(
        store.load_world_state().unwrap().unwrap().components[17],
        components[17]
    );
    let mut dirty = PersistenceDirtyTracker::default();
    let p = ChunkPos { x: -1, z: -1 };
    dirty.mark_dirty(p);
    let token = dirty.begin_save(p).unwrap();
    dirty.mark_dirty(p);
    dirty.complete_save(token, true);
    assert!(dirty.is_dirty(p));
    assert_eq!(dirty.dirty_since.len(), 1);
    let token = dirty.begin_save(p).unwrap();
    dirty.complete_save(token, false);
    assert!(dirty.is_dirty(p));
    let token = dirty.begin_save(p).unwrap();
    dirty.complete_save(token, true);
    assert!(dirty.dirty_since.is_empty());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn s1_measured_writes_keep_format_and_clone_totals() {
    let (root, store) = fixture();
    let clone = store.clone();
    let c = StoredChunk {
        position: ChunkPos { x: -5, z: 9 },
        sections: vec![],
        spatial_records: vec![SpatialRecord {
            entity_id: EntityId(42),
            entity_revision: 7,
            entity_type: "test_a:opaque".into(),
            schema_version: 9,
            payload: vec![3, 7, 9],
        }],
        spatial_tombstones: vec![],
    };
    let m = clone.store_chunk_measured(&c).unwrap();
    assert_eq!(store.load_chunk(c.position).unwrap(), c);
    let totals = store.io_metrics();
    assert_eq!(totals.columns, 1);
    assert_eq!(totals.application_write_bytes, m.stored_file_bytes as u64);
    assert!(m.encode_ms >= 0.0 && m.durability_ms >= 0.0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn s1_transfer_reopen_at_each_acknowledgement() {
    let (root, store) = fixture();
    let source = ChunkPos { x: -1, z: 0 };
    let destination = ChunkPos { x: 0, z: 0 };
    let original = SpatialRecord {
        entity_id: EntityId(77),
        entity_revision: 1,
        entity_type: "test_b:item".into(),
        schema_version: 1,
        payload: vec![42; 96],
    };
    let mut src = StoredChunk {
        position: source,
        sections: vec![],
        spatial_records: vec![original.clone()],
        spatial_tombstones: vec![],
    };
    let mut dst = StoredChunk {
        position: destination,
        sections: vec![],
        spatial_records: vec![],
        spatial_tombstones: vec![],
    };
    store.store_chunk(&src).unwrap();
    store.store_chunk(&dst).unwrap();
    let mut moved = original;
    moved.entity_revision = 2;
    dst.spatial_records.push(moved);
    dst.spatial_tombstones.push(SpatialTombstone {
        entity_id: EntityId(77),
        entity_revision: 2,
        source,
    });
    // Before destination acknowledgement: source remains the only durable record.
    assert_eq!(store.load_chunk(source).unwrap().spatial_records.len(), 1);
    store.store_chunk(&dst).unwrap();
    for phase in 0..3 {
        let reopened = WorldStorage::open(&root, "fault").unwrap();
        let a = reopened.load_chunk(source).unwrap();
        let b = reopened.load_chunk(destination).unwrap();
        let newest = a
            .spatial_records
            .iter()
            .chain(&b.spatial_records)
            .max_by_key(|r| r.entity_revision)
            .unwrap();
        assert_eq!(
            (newest.entity_id, newest.entity_revision),
            (EntityId(77), 2)
        );
        assert_eq!(b.spatial_tombstones.len(), usize::from(phase < 2));
        if phase == 0 {
            src.spatial_records.clear();
            store.store_chunk(&src).unwrap();
        }
        if phase == 1 {
            dst.spatial_tombstones.clear();
            store.store_chunk(&dst).unwrap();
        }
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn s1_transfer_marker_normalization_matches_encoded_fields() {
    let mut c = StoredChunk {
        position: ChunkPos { x: 0, z: 0 },
        sections: vec![],
        spatial_records: vec![],
        spatial_tombstones: vec![],
    };
    let before = encode_chunk_file(&c).unwrap().1.raw_payload_bytes;
    c.spatial_tombstones.push(SpatialTombstone {
        entity_id: EntityId(1),
        entity_revision: 1,
        source: ChunkPos { x: -1, z: 0 },
    });
    let after = encode_chunk_file(&c).unwrap().1.raw_payload_bytes;
    assert_eq!(after - before, 16 + 8 + 4 + 4);
}
