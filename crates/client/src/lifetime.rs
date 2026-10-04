//! Demand-driven scalar ownership ledger. No snapshots/Arcs or unbounded history are retained.
use super::ClientApp;
use serde_json::{Value, json};
impl ClientApp {
    pub(super) fn lifetime_ledger(&mut self) -> Value {
        self.process.sample();
        if let Some(provider) = self.gpu_metrics.as_mut() {
            provider.sample();
        }
        let Some(s) = self.simulation.as_ref() else {
            return json!({"unavailable":"simulation not open"});
        };
        let columns = s.world.column_positions().collect::<Vec<_>>();
        let outside = columns
            .iter()
            .filter(|p| !self.residency.is_retained_by_radius(**p));
        let persistence_pinned = outside
            .clone()
            .filter(|p| {
                self.persistence_dirty.is_dirty(**p) || self.persistence_dirty.is_saving(**p)
            })
            .count();
        let lighting_pin = s.lighting.integrating_column();
        let lighting_pinned = outside
            .clone()
            .filter(|p| lighting_pin == Some(**p))
            .count();
        let outside_count = outside.count();
        let entities = s.entity_lifetime_counts();
        let m = self.mesh_scheduler.stats();
        let gpu = self.renderer.as_ref().map_or(json!({"unavailable":"no graphical renderer"}), |r| {
            let (sections,pages,capacity,retired) = r.mesh_lifetime_counts();
            let bytes=r.gpu_mesh_byte_components();
            json!({"adapter":r.adapter_info.name,"backend":format!("{:?}",r.adapter_info.backend),"global_atlas_capacity_bytes":r.texture_bytes(),"sections":sections,"mesh_pages":pages,"vertex_buffers":pages,"index_buffers":pages,
                "logical_vertex_bytes":bytes[0],"logical_index_bytes":bytes[1],"vertex_capacity_bytes":bytes[2],"index_capacity_bytes":bytes[3],
                "logical_bytes":r.gpu_mesh_logical_bytes(),"capacity_bytes":r.gpu_mesh_allocated_bytes(),
                "map_capacity":capacity,"created":r.mesh_buffer_allocations()+r.mesh_buffer_reallocations(),
                "reused":r.mesh_buffer_reuses(),"retired":retired,
                "fixed_text_capacity_bytes":2048*2048*4,"fixed_text_pages":1,
                "staging":"wgpu internal staging lifetime unavailable; upload enqueue is not VRAM"})
        });
        let load = self.load_scheduler.metrics();
        let generation = self.generation_scheduler.metrics();
        let render = self.presentation.as_ref().map_or(json!({"unavailable":"no render world"}), |w|json!({"sections":w.chunks().count(),"snapshots":w.chunks().count(),"snapshot_bytes":w.snapshot_bytes()}));
        let center = rustcraft_engine_core::ChunkPos {
            x: (s.player.position.x.floor() as i32).div_euclid(16),
            z: (s.player.position.z.floor() as i32).div_euclid(16),
        };
        let core_ready = (-1..=1).all(|dx| {
            (-1..=1).all(|dz| {
                let p = rustcraft_engine_core::ChunkPos {
                    x: center.x + dx,
                    z: center.z + dz,
                };
                s.world.column_available(p) && self.render_ready_columns.contains(&p)
            })
        });
        let light_columns = s.lighting.integration_columns();
        let core_light_pending = light_columns.iter().any(|p| {
            (i64::from(p.x) - i64::from(center.x))
                .abs()
                .max((i64::from(p.z) - i64::from(center.z)).abs())
                <= 1
        });
        let idle = core_ready
            && self.residency.pending_column_count() == 0
            && load.queued + load.in_flight == 0
            && generation.pending + generation.in_flight == 0
            && self.initial_lighting_scheduler.outstanding() == 0
            && self.mesh_scheduler.is_idle()
            && self.snapshot_dirty_sections.is_empty()
            && self.mesh_dirty_sections.is_empty()
            && self.persistence_dirty.dirty_count() == 0
            && outside_count == lighting_pinned;
        let mut effective_config = serde_json::Map::new();
        for key in [
            rustcraft_control::config::settings::LOAD_RADIUS,
            rustcraft_control::config::settings::RETAIN_RADIUS,
            rustcraft_control::config::settings::LIGHT_WORK,
            rustcraft_control::config::settings::UPLOAD_BYTES,
            rustcraft_control::config::settings::UPLOAD_SECTIONS,
            rustcraft_control::config::settings::STREAM_MS,
            rustcraft_control::config::settings::DIAGNOSTIC_MS,
        ] {
            effective_config.insert(key.into(), self.control_state.config.effective(key).plain());
        }
        let light_work = s.lighting.work_counters();
        let light_lifetime = s.lighting.lifetime_counts();
        let value = json!({
            "lighting_lifetime":{"columns":light_lifetime[0],"retired_source_columns":s.lighting.retired_source_columns(&s.world),"cleanup_backpressured":s.lighting.cleanup_backpressured(),"direct_sections":light_lifetime[1],"completed_columns":light_lifetime[2],"queued_integration":light_lifetime[3],"queued_cleanup":light_lifetime[4],"active_queue":light_lifetime[5],"active_queue_capacity":light_lifetime[6],"active_column":s.lighting.integrating_column().map(|p|[p.x,p.z])},
            "lighting_progress":{"started":light_work.columns_started,"completed":light_work.columns_completed,"queue_pushes":light_work.propagation_queue_pushes,"queue_pops":light_work.propagation_queue_pops},
            "idle":idle,"core_ready":core_ready,"lighting_frontier_columns":light_columns.len(),"lighting_core_pending":core_light_pending,"settle_boundary_note":"bulk-lit Safe/Visible core; bounded neighbor reconciliation may continue; one active eviction pin allowed","player_chunk":[center.x,center.z],
            "world":{"desired":self.residency.desired_column_count(),"retained_resident":columns.iter().filter(|p|self.residency.is_retained_by_radius(**p)).count(),
                "safe":s.world.safe_column_positions().count(),"visible":self.render_ready_columns.len(),
                "light_sections":s.world.light_lifetime_counts().0,"orphan_light_sections":s.world.light_lifetime_counts().1,
                "resident_columns":columns.len(),"resident_sections":s.world.section_positions().count(),
                "load_completed_unconsumed":(load.completed+load.missing+load.failed).saturating_sub(self.load_results_applied),"generation_completed_unconsumed":generation.completed.saturating_sub(self.generation_results_applied),"initial_lighting_outstanding":self.initial_lighting_scheduler.outstanding(),
                "pending_load":load.queued+load.in_flight,"pending_generation":generation.pending+generation.in_flight,
                "dirty":self.persistence_dirty.dirty_count(),"persistence_pinned":persistence_pinned,
                "lighting_pinned":lighting_pinned,"outside_retained":outside_count,
                "other_or_transient_eviction_blocked":outside_count.saturating_sub(persistence_pinned+lighting_pinned),
                "pending_evictions":self.pending_evictions.len()},
            "simulation":{"sections":s.world.section_positions().count(),"active_entities":entities[0],"dropped_items":entities[0],
                "durable_entities":entities[1],"tombstones":entities[2],"pickup_receipts":entities[3],
                "save_snapshot_records":self.entity_save_snapshots.len(),"player_checkpoint_receipt_batches":self.player_checkpoint_receipts.len(),
                "timing_records":self.request_started_at.len()+self.generation_started_at.len()+self.light_latency_started_at.len()+self.light_queue_started_at.len()+self.light_work_started_at.len()+self.light_cpu_accumulated_ms.len()+self.visible_latency_started_at.len()+self.first_section_latency_started_at.len()+self.visible_expected_sections.len(),
                "frozen_columns":entities[4],"durable_capacity":entities[5],
                "entity_presentation":"derived each frame; no separate retained entity map"},
            "render":render,
            "meshing":{"generations":m.generation_entries,"generation_capacity":m.generation_capacity,
                "pipeline_limit":m.pipeline_limit,"result_logical_byte_limit":m.result_logical_byte_limit,"result_capacity_byte_limit":m.result_capacity_byte_limit,
                "pending":m.pending,"inflight_submitted_unconsumed":m.in_flight,"retired_inflight":m.retired_inflight,"cancelled_pending":m.cancelled_pending,
                "completed_unconsumed":m.completed_unconsumed,"completed_bytes":m.completed_cpu_bytes,"completed_capacity_bytes":m.completed_capacity_bytes,"ready_capacity_bytes":m.ready_capacity_bytes,"pending_order_entries":m.pending_order_entries,
                "ready":m.ready,"ready_bytes":m.ready_cpu_bytes,"ready_capacity":m.ready_capacity,
                "live_job_snapshots":m.live_snapshots,"live_job_snapshot_bytes":m.live_snapshot_bytes,
                "pending_snapshot_bytes":m.pending_snapshot_bytes,"inflight_snapshot_bytes_upper_bound":m.in_flight_snapshot_bytes,
                "stale":m.mesh_jobs_discarded_stale,"coalesced":m.mesh_jobs_coalesced,"workers":m.worker_count},
            "gpu":gpu,
            "process":{"provider":"existing ProcessSampler; Linux /proc/self/status VmRSS", "rss_bytes":self.process.snapshot.rss_mib.map(|v| (v*1024.*1024.) as u64),"status":if self.process.snapshot.rss_mib.is_some(){"available"}else{"unavailable"}},
            "vram":{"status":"unavailable","process_specific":true,"bytes":null,"reason":"no trustworthy process GPU-memory provider; device-wide counters are not substituted", "device_wide_bytes":self.gpu_metrics.as_ref().and_then(|p|p.snapshot.used_mib).map(|v|(v*1048576.) as u64),"device_wide_provider":"optional Linux DRM sysfs mem_info_vram_used; whole adapter, NOT process VRAM; 500ms cadence"},
            "config":effective_config,
            "tick":s.time
        });
        value
    }
}
