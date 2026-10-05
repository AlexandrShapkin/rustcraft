//! Crash tests exercise actual checkpoints and game codecs, with no scheduler timing races.
use crate::{blocks, player_persistence as player, world_persistence as spatial};
use rustcraft_engine_core::{ChunkPos, ItemId, Vec3, World};
use rustcraft_mod_api::{BlockRegistry, GameplayModule};
use rustcraft_runtime::{
    Simulation,
    inventory::{Inventory, ItemStack},
};
use rustcraft_world::{StoredChunk, WorldStorage};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const A: ChunkPos = ChunkPos { x: 0, z: 0 };
const B: ChunkPos = ChunkPos { x: 1, z: 0 };
static NEXT: AtomicU64 = AtomicU64::new(1);
struct Fixture {
    root: PathBuf,
    storage: WorldStorage,
    sim: Simulation,
    total: u32,
}
impl Fixture {
    fn new(partial: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "rustcraft-transfer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let storage = WorldStorage::open(&root, "test").unwrap();
        let mut registry = BlockRegistry::default();
        blocks::BlocksModule.register(&mut registry).unwrap();
        let mut sim = Simulation::new_with_entity_namespace(
            World::new(blocks::AIR.id),
            registry,
            Vec3::new(0.5, 3., 0.5),
            7,
        );
        if partial {
            let stack = ItemStack {
                item: ItemId(1),
                count: 64,
                damage: 0,
            };
            let mut slots = [Some(stack); 36];
            slots[0].as_mut().unwrap().count = 52;
            sim.inventory = Inventory::from_slots(slots, 0).unwrap();
        }
        sim.spawn_item(ItemId(1), 30, sim.player.position);
        sim.items[0].pickup_delay = 0.;
        let total = quantity(&sim);
        let mut fixture = Self {
            root,
            storage,
            sim,
            total,
        };
        fixture.save_column(A);
        fixture
            .storage
            .store_player(&player::encode_revision(&fixture.sim, 1, &[]).unwrap())
            .unwrap();
        fixture
    }
    fn capture(&self, pos: ChunkPos) -> (StoredChunk, rustcraft_runtime::EntityColumnSnapshot) {
        let snapshot = self.sim.entity_column_snapshot(pos);
        let records = snapshot
            .records
            .iter()
            .map(|entity| spatial::encode_item_entity(entity, &self.sim.registry).unwrap())
            .collect();
        let markers = snapshot
            .tombstones
            .iter()
            .map(|(id, source, revision)| rustcraft_world::SpatialTombstone {
                entity_id: *id,
                source: *source,
                entity_revision: *revision,
            })
            .collect();
        (
            StoredChunk {
                position: pos,
                sections: vec![],
                spatial_records: records,
                spatial_tombstones: markers,
            },
            snapshot,
        )
    }
    fn save_column(&mut self, pos: ChunkPos) {
        let (chunk, snapshot) = self.capture(pos);
        self.storage.store_chunk(&chunk).unwrap();
        self.sim.note_entity_column_persisted(&snapshot);
    }
    fn player_durable(&mut self) {
        let receipts = self.sim.pickup_receipts();
        self.storage
            .store_player(&player::encode_revision(&self.sim, 2, &[]).unwrap())
            .unwrap();
        self.sim.commit_pickup_receipts(&receipts);
    }
    fn reopen(&self, order: &[ChunkPos]) -> Simulation {
        let storage = WorldStorage::open(&self.root, "test").unwrap();
        let mut sim = Simulation::new_with_entity_namespace(
            World::new(blocks::AIR.id),
            self.sim.registry.clone(),
            Vec3::ZERO,
            8,
        );
        let record = storage
            .load_player(player::LOCAL_PLAYER_ID)
            .unwrap()
            .unwrap();
        let state = player::decode(&record, &sim.registry).unwrap();
        player::apply(&mut sim, state);
        for pos in order {
            let chunk = storage.load_chunk(*pos).unwrap();
            let records = chunk
                .spatial_records
                .iter()
                .map(|record| {
                    spatial::decode_item_entity(record, &sim.registry, "test", *pos).unwrap()
                })
                .collect();
            let markers = chunk
                .spatial_tombstones
                .iter()
                .map(|marker| (marker.entity_id, marker.source, marker.entity_revision))
                .collect::<Vec<_>>();
            sim.activate_entity_column(*pos, records, &markers).unwrap();
        }
        assert_eq!(quantity(&sim), self.total);
        sim
    }
    fn fail_write(&self, directory: &str, write: impl FnOnce() -> bool) {
        let path = self.storage.root().join(directory);
        let held = self.storage.root().join(format!("{directory}-held"));
        std::fs::rename(&path, &held).unwrap();
        std::fs::write(&path, b"deterministic I/O failure").unwrap();
        assert!(write());
        std::fs::remove_file(&path).unwrap();
        std::fs::rename(&held, &path).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
fn quantity(sim: &Simulation) -> u32 {
    sim.inventory
        .slots()
        .iter()
        .flatten()
        .map(|stack| u32::from(stack.count))
        .sum::<u32>()
        + sim
            .items
            .iter()
            .map(|entity| u32::from(entity.stack.count))
            .sum::<u32>()
}
#[test]
fn full_pickup_source_before_player_and_failed_player_checkpoint() {
    let mut f = Fixture::new(false);
    f.sim.step::<()>(Default::default(), 0.);
    assert!(f.sim.items.is_empty());
    f.save_column(A);
    assert_eq!(f.reopen(&[A]).items[0].stack.count, 30);
    let player = player::encode_revision(&f.sim, 2, &[]).unwrap();
    f.fail_write("players", || f.storage.store_player(&player).is_err());
    f.save_column(A);
    assert_eq!(f.reopen(&[A]).items[0].stack.count, 30);
    f.player_durable();
    assert!(f.reopen(&[A]).items.is_empty());
    f.save_column(A);
    assert!(f.sim.pickup_receipts().is_empty());
    assert!(f.reopen(&[A]).items.is_empty());
}
#[test]
fn partial_pickup_both_save_orders_and_repeated_recovery() {
    for source_first in [false, true] {
        let mut f = Fixture::new(true);
        f.sim.step::<()>(Default::default(), 0.);
        assert_eq!(f.sim.items[0].stack.count, 18);
        let receipt = f.sim.pickup_receipts()[0];
        assert_eq!(
            (
                receipt.before_count,
                receipt.accepted_count,
                receipt.remaining_count
            ),
            (30, 12, 18)
        );
        if source_first {
            f.save_column(A);
            assert_eq!(f.reopen(&[A]).items[0].stack.count, 30);
        }
        f.player_durable();
        for _ in 0..3 {
            let mut recovered = f.reopen(&[A]);
            let chunk = f.storage.load_chunk(A).unwrap();
            let entity = spatial::decode_item_entity(
                &chunk.spatial_records[0],
                &recovered.registry,
                "test",
                A,
            )
            .unwrap();
            recovered
                .activate_entity_column(A, vec![entity], &[])
                .unwrap();
            assert_eq!(recovered.items.len(), 1);
            assert_eq!(recovered.items[0].stack.count, 18);
            assert_eq!(quantity(&recovered), f.total);
        }
        f.save_column(A);
        assert!(f.sim.pickup_receipts().is_empty());
        assert_eq!(f.reopen(&[A]).items[0].stack.count, 18);
        // A previous receipt-bearing checkpoint may complete again after pruning.
        f.sim.commit_pickup_receipts(&[receipt]);
        assert!(f.sim.pickup_receipts().is_empty());
    }
}
#[test]
fn migration_source_first_destination_failure_and_destination_before_cleanup() {
    let mut f = Fixture::new(false);
    let id = f.sim.items[0].id;
    f.sim.items[0].position.x = 16.5;
    f.sim.items[0].persistence_revision += 1;
    let (destination, snapshot) = f.capture(B);
    f.save_column(A);
    assert_eq!(f.reopen(&[A]).items[0].id, id);
    f.fail_write("chunks", || f.storage.store_chunk(&destination).is_err());
    assert_eq!(f.reopen(&[A]).items[0].id, id);
    f.storage.store_chunk(&destination).unwrap();
    for order in [[A, B], [B, A]] {
        let recovered = f.reopen(&order);
        assert_eq!(recovered.items.len(), 1);
        assert_eq!(recovered.items[0].id, id);
        assert_eq!(recovered.observe(1).nearby_items[0].id, id);
        assert_eq!(recovered.items[0].column(), B);
    }
    f.sim.note_entity_column_persisted(&snapshot);
    f.save_column(A);
    assert!(f.storage.load_chunk(A).unwrap().spatial_records.is_empty());
    assert_eq!(f.reopen(&[A, B]).items.len(), 1);
}

#[test]
fn late_source_ack_keeps_migration_marker_until_actual_cleanup() {
    let mut f = Fixture::new(false);
    f.sim.items[0].position.x = 16.5;
    f.sim.items[0].persistence_revision += 1;
    let (source, source_snapshot) = f.capture(A);
    f.save_column(B);
    f.storage.store_chunk(&source).unwrap();
    f.sim.note_entity_column_persisted(&source_snapshot);
    assert!(!f.sim.entity_column_snapshot(B).tombstones.is_empty());
    assert!(f.sim.entity_transfer_pending(A));
    assert_eq!(f.reopen(&[A, B]).items.len(), 1);
    f.save_column(A);
    assert!(f.sim.entity_column_snapshot(B).tombstones.is_empty());
    assert!(!f.sim.entity_transfer_pending(A));
}

#[test]
fn old_player_ack_cannot_authorize_another_partial_transition() {
    let mut f = Fixture::new(true);
    f.sim.step::<()>(Default::default(), 0.);
    let old = f.sim.pickup_receipts()[0];
    f.player_durable();
    f.save_column(A);
    f.sim.inventory.remove(0, 12);
    f.sim.step::<()>(Default::default(), 0.);
    assert_eq!(f.sim.items[0].stack.count, 6);
    f.sim.commit_pickup_receipts(&[old]);
    f.save_column(A);
    assert_eq!(f.reopen(&[A]).items[0].stack.count, 18);
    assert_eq!(f.sim.pickup_receipts().len(), 1);
}

#[test]
fn migration_return_to_source_retires_superseded_marker() {
    let mut f = Fixture::new(false);
    let id = f.sim.items[0].id;
    f.sim.items[0].position.x = 16.5;
    f.sim.items[0].persistence_revision += 1;
    let (destination, snapshot) = f.capture(B);
    f.sim.items[0].position.x = 0.5;
    f.sim.items[0].persistence_revision += 1;
    f.storage.store_chunk(&destination).unwrap();
    f.sim.note_entity_column_persisted(&snapshot);
    f.save_column(A);
    f.save_column(B);
    f.save_column(A);
    assert!(!f.sim.entity_transfer_pending(A));
    assert!(!f.sim.entity_transfer_pending(B));
    let recovered = f.reopen(&[B, A]);
    assert_eq!(recovered.items.len(), 1);
    assert_eq!(recovered.items[0].id, id);
    assert_eq!(recovered.items[0].column(), A);
}

#[test]
fn restored_full_receipt_schedules_loaded_source_cleanup_and_prunes() {
    let mut f = Fixture::new(false);
    f.sim.step::<()>(Default::default(), 0.);
    f.player_durable();
    let mut recovered = f.reopen(&[A]);
    assert!(recovered.items.is_empty());
    assert!(recovered.take_persistence_dirty_chunks().contains(&A));
    f.sim = recovered;
    f.save_column(A);
    assert!(f.sim.pickup_receipts().is_empty());
    f.storage
        .store_player(&player::encode_revision(&f.sim, 3, &[]).unwrap())
        .unwrap();
    assert!(f.reopen(&[A]).pickup_receipts().is_empty());
}
