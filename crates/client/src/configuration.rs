//! Native application boundaries. No setting lookup occurs in voxel/entity loops.
use super::*;
use rustcraft_control::config::{Policy, Registry, settings as keys};
impl ClientApp {
    pub(super) fn apply_config_boundary(&mut self, policy: Policy) {
        if !self.control_state.config.has_pending(policy) {
            return;
        }
        let mut config = std::mem::take(&mut self.control_state.config);
        let tick = self.simulation.as_ref().map_or(0, |s| s.time);
        let boundary = if policy == Policy::NextFrame {
            self.devtools.as_ref().map_or(0, |d| d.frame)
        } else {
            tick
        };
        let result = config.apply(policy, boundary, |candidate| {
            self.adopt_config(candidate, policy)
        });
        self.control_state.config = config;
        self.control_state.sync_config();
        self.control_state.config_status = match &result {
            Ok(_) => format!("{policy:?} applied at boundary {boundary}"),
            Err(e) => e.clone(),
        };
        self.dx_text.clear();
        if let Some(tools) = self.devtools.as_mut() {
            let message = match result {
                Ok(_) => self
                    .control_state
                    .config
                    .history
                    .back()
                    .map(|c| serde_json::to_string(c).unwrap())
                    .unwrap_or_default(),
                Err(e) => e,
            };
            tools.events.push(tick, "config-change", &message);
        }
    }
    fn adopt_config(&mut self, c: &Registry, policy: Policy) -> Result<(), String> {
        if policy == Policy::NextTick {
            self.residency
                .set_radii(
                    c.effective(keys::LOAD_RADIUS).integer() as i32,
                    c.effective(keys::RETAIN_RADIUS).integer() as i32,
                )
                .map_err(str::to_owned)?;
            self.stream_load_radius = self.residency.load_radius();
            self.stream_lookahead_enabled = c.effective(keys::LOOKAHEAD).boolean();
            self.lighting_work_budget = c.effective(keys::LIGHT_WORK).integer() as usize;
            // Keep the last successful checkpoint clocks and dirty/revision/queued state unchanged.
            self.player_autosave_interval =
                Duration::from_millis(c.effective(keys::PLAYER_SAVE_MS).integer() as u64);
            self.world_state_autosave_interval =
                Duration::from_millis(c.effective(keys::WORLD_SAVE_MS).integer() as u64);
        } else if policy == Policy::NextFrame {
            self.mesh_upload_section_budget = c.effective(keys::UPLOAD_SECTIONS).integer() as usize;
            self.mesh_upload_byte_budget = c.effective(keys::UPLOAD_BYTES).integer() as usize;
            self.stream_main_budget =
                Duration::from_secs_f64(c.effective(keys::STREAM_MS).float() / 1000.);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_control::{Action, config::Value};
    fn app() -> ClientApp {
        let mut a = ClientApp::new(None, None);
        let mut bootstrap = RuntimeBootstrap::new(Default::default());
        bootstrap.register_module(&BlocksModule).unwrap();
        a.simulation = Some(Simulation::new(
            rustcraft_engine_core::World::new(rustcraft_minecraft_b173::blocks::AIR.id),
            bootstrap.registry,
            Vec3::new(-0.5, 65., -0.5),
        ));
        a
    }
    fn request(a: &mut ClientApp, keys: &[(&str, &str)]) {
        a.control_state
            .configuration_action(&Action::ConfigSet(
                keys.iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ))
            .unwrap();
    }
    #[test]
    fn native_boundaries_keep_queued_work_and_clocks() {
        let mut a = app();
        let last_player = a.last_player_autosave;
        let last_world = a.last_world_state_autosave;
        a.player_dirty = true;
        a.player_revision = 7;
        a.world_state_dirty = true;
        a.world_state_revision = 9;
        request(
            &mut a,
            &[(keys::LOAD_RADIUS, "6"), (keys::RETAIN_RADIUS, "7")],
        );
        assert_eq!(a.residency.load_radius(), 4);
        a.apply_config_boundary(Policy::NextFrame);
        assert_eq!(a.residency.load_radius(), 4);
        a.apply_config_boundary(Policy::NextTick);
        assert_eq!(
            (a.residency.load_radius(), a.residency.retain_radius()),
            (6, 7)
        );
        request(
            &mut a,
            &[(keys::LOAD_RADIUS, "3"), (keys::RETAIN_RADIUS, "3")],
        );
        a.apply_config_boundary(Policy::NextTick);
        assert_eq!(a.residency.load_radius(), 3);
        assert!(
            a.control_state
                .configuration_action(&Action::ConfigSet(vec![(
                    keys::LOAD_RADIUS.into(),
                    "12".into()
                )]))
                .is_err()
        );
        assert_eq!(a.residency.load_radius(), 3);
        request(
            &mut a,
            &[(keys::PLAYER_SAVE_MS, "1000"), (keys::WORLD_SAVE_MS, "100")],
        );
        a.apply_config_boundary(Policy::NextTick);
        assert_eq!(a.last_player_autosave, last_player);
        assert_eq!(a.last_world_state_autosave, last_world);
        assert!(a.player_dirty && a.world_state_dirty);
        assert_eq!((a.player_revision, a.world_state_revision), (7, 9));
        request(&mut a, &[(keys::LIGHT_WORK, "1")]);
        a.apply_config_boundary(Policy::NextTick);
        assert_eq!(a.lighting_work_budget, 1);
        let pending = a.mesh_scheduler.stats().pending;
        request(
            &mut a,
            &[(keys::UPLOAD_SECTIONS, "1"), (keys::UPLOAD_BYTES, "262144")],
        );
        a.apply_config_boundary(Policy::NextFrame);
        assert_eq!(a.mesh_upload_section_budget, 1);
        assert_eq!(a.mesh_upload_byte_budget, 262144);
        assert_eq!(a.mesh_scheduler.stats().pending, pending);
        request(&mut a, &[(keys::DIAGNOSTIC_MS, "50")]);
        assert_eq!(
            a.control_state.diagnostics.cadence,
            Duration::from_millis(50)
        );
        let now = Instant::now();
        use rustcraft_control::diagnostics::Domain;
        a.control_state.diagnostics.collected(Domain::World, now);
        assert!(
            !a.control_state
                .diagnostics
                .due(Domain::World, now + Duration::from_millis(49))
        );
        assert!(
            a.control_state
                .diagnostics
                .due(Domain::World, now + Duration::from_millis(51))
        );
        assert!(a.control_state.diagnostic_demand().is_empty());
        assert_eq!(
            a.control_state.config.effective(keys::LIGHT_WORK),
            &Value::Integer(1)
        );
    }
    #[test]
    fn shorter_and_longer_dirty_autosave_flushes_and_reopens() {
        let root = std::env::temp_dir().join(format!(
            "c1-autosave-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut a = app();
        let storage = rustcraft_world::WorldStorage::open(&root, "fixture").unwrap();
        a.world_storage = Some(storage.clone());
        for (interval, x) in [("1000", 9.5), ("2000", -9.5)] {
            a.simulation.as_mut().unwrap().player.position.x = x;
            a.simulation.as_mut().unwrap().time += 17;
            request(
                &mut a,
                &[
                    (keys::PLAYER_SAVE_MS, interval),
                    (keys::WORLD_SAVE_MS, interval),
                ],
            );
            a.apply_config_boundary(Policy::NextTick);
            a.last_player_autosave = Instant::now() - Duration::from_secs(3);
            a.last_world_state_autosave = Instant::now() - Duration::from_secs(3);
            a.service_player_autosave();
            a.service_world_state_autosave();
            let deadline = Instant::now() + Duration::from_secs(5);
            while a.player_persisted_revision < a.player_revision
                || a.world_state_persisted_revision < a.world_state_revision
            {
                a.service_player_autosave();
                a.service_world_state_autosave();
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            let reopened = rustcraft_world::WorldStorage::open(&root, "fixture").unwrap();
            let record = reopened
                .load_player(rustcraft_minecraft_b173::player_persistence::LOCAL_PLAYER_ID)
                .unwrap()
                .unwrap();
            let restored = rustcraft_minecraft_b173::player_persistence::decode(
                &record,
                &a.simulation.as_ref().unwrap().registry,
            )
            .unwrap();
            assert_eq!(restored.position.x, x);
            assert_eq!(
                reopened.load_world_state().unwrap().unwrap().revision,
                a.world_state_revision
            );
        }
        a.simulation.as_mut().unwrap().player.position.x = 22.5;
        a.finish_world_saves();
        let record = storage
            .load_player(rustcraft_minecraft_b173::player_persistence::LOCAL_PLAYER_ID)
            .unwrap()
            .unwrap();
        let restored = rustcraft_minecraft_b173::player_persistence::decode(
            &record,
            &a.simulation.as_ref().unwrap().registry,
        )
        .unwrap();
        assert_eq!(restored.position.x, 22.5);
        drop(a);
        std::fs::remove_dir_all(root).unwrap();
    }
}
