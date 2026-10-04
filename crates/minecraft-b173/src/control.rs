//! First-party composition adapter and package-owned commands. Generic control stays policy-free.
use rustcraft_control::{
    Action, CommandResult, CommandSpec, ControlResult, ControlState, Host, Registry, Snapshot,
};
use rustcraft_engine_core::{BlockPos, BlockState, Vec3};
use rustcraft_runtime::Simulation;
use serde_json::{Value, json};
pub struct MinecraftHost<'a> {
    pub simulation: &'a mut Simulation,
    pub state: &'a mut ControlState,
}
impl MinecraftHost<'_> {
    /// Cheap current authoritative values, shared by headless and graphical adapters.
    pub fn live_snapshot(simulation: &Simulation, state: &ControlState) -> Snapshot {
        let s = simulation;
        let mut snapshot = state.domains.clone();
        snapshot.tick = s.time;
        snapshot.runtime = json!({"tick":s.time,"fixed_paused":state.fixed.paused,"controller":if state.leased {"Scenario"} else {"Human"}});
        snapshot.player = json!({"position":[s.player.position.x,s.player.position.y,s.player.position.z],
            "velocity":[s.player.velocity.x,s.player.velocity.y,s.player.velocity.z],
            "orientation":{"yaw":s.player.yaw,
            "pitch":s.player.pitch},
            "chunk":[(s.player.position.x.floor() as i32).div_euclid(16),(s.player.position.z.floor() as i32).div_euclid(16)],
            "mode":format!("{:?}",s.mode),
            "grounded":s.player.on_ground,
            "selected":s.inventory.selected(),
            "held":s.inventory.held().map(|stack| json!({"key":s.registry.item(stack.item).map(|i|i.name),
            "count":stack.count})),
            "paused":state.fixed.paused});
        snapshot
    }
    pub fn collect_diagnostic(&self, domain: rustcraft_control::diagnostics::Domain) -> Value {
        use rustcraft_control::diagnostics::Domain;
        let s = &self.simulation;
        let mut snapshot = Snapshot::default();
        if domain == Domain::World {
            snapshot.world = self.state.domains.world.clone();
        }
        match domain {
            Domain::World => {
                if snapshot.world.is_null() {
                    snapshot.world = json!({});
                }
                snapshot.world["time"] = json!(s.time);
                snapshot.world["dirty_sections"] = json!(s.dirty_section_count());
                // Small semantic neighborhood; never a whole-world scan.
                let center = [
                    s.player.position.x.floor() as i32,
                    s.player.position.y.floor() as i32,
                    s.player.position.z.floor() as i32,
                ];
                let mut blocks = serde_json::Map::new();
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            let p = [center[0] + dx, center[1] + dy, center[2] + dz];
                            if let Ok(key) = self.block(p) {
                                blocks.insert(format!("{},{},{}", p[0], p[1], p[2]), json!(key));
                            }
                        }
                    }
                }
                snapshot.world["blocks"] = Value::Object(blocks);
                snapshot.world["block_query_radius"] = json!(1);
                snapshot.world["resident_columns"] = json!(s.world.column_positions().count());
                snapshot.world["player_chunk"] =
                    json!([center[0].div_euclid(16), center[2].div_euclid(16)]);
                snapshot.world["target"]=s.target().map_or(json!({"status":"no raycast hit"}),|h|json!({"position":[h.block.x,h.block.y,h.block.z],"key":self.block([h.block.x,h.block.y,h.block.z]).ok()}));
                snapshot.world
            }
            Domain::Entities => {
                snapshot.entities=json!(s.items.iter().take(64).map(|e|json!({"id":format!("{:032x}",e.id.0),
                    "type":"minecraft_b173:item",
                    "position":[e.position.x,e.position.y,e.position.z],
                    "velocity":[e.velocity.x,e.velocity.y,e.velocity.z],
                    "owner_chunk":[(e.position.x.floor() as i32).div_euclid(16),(e.position.z.floor() as i32).div_euclid(16)],
                    "age":e.age,
                    "revision":e.persistence_revision})).collect::<Vec<_>>());
                snapshot.entities
            }
            Domain::Entity => {
                let Some(id) = self.state.selected_entity.as_deref() else {
                    return json!({"unavailable":"select an EntityId (E or /debug entity HEX_ID)"});
                };
                let cap = 4096;
                s.items.iter().take(cap).find(|e| format!("{:032x}",e.id.0)==id).map_or_else(
                    || json!({"id":id,"status":"not found in active entities; may be unloaded or removed","coverage":cap,"truncated":s.items.len()>cap}),
                    |e|json!({"id":id,
                        "type":"minecraft_b173:item",
                        "position":[e.position.x,e.position.y,e.position.z],
                        "velocity":[e.velocity.x,e.velocity.y,e.velocity.z],
                        "owner_chunk":[(e.position.x.floor() as i32).div_euclid(16),(e.position.z.floor() as i32).div_euclid(16)],
                        "age":e.age,
                        "revision":e.persistence_revision,
                        "durability":"active record; revision is logical, not confirmation of fsync"}))
            }
            _ => json!({"unavailable":"provider belongs to client composition"}),
        }
    }
}
impl Host for MinecraftHost<'_> {
    fn diagnostic_due(&self, domain: rustcraft_control::diagnostics::Domain) -> bool {
        self.state
            .diagnostics
            .samples
            .get(&domain)
            .and_then(|s| s.at)
            .is_none_or(|t| t.elapsed() >= rustcraft_control::diagnostics::Diagnostics::CADENCE)
    }
    fn prepare_diagnostics(&mut self, domains: &[rustcraft_control::diagnostics::Domain]) {
        use rustcraft_control::diagnostics::Domain;
        self.state.diagnostics.managed = true;
        for &d in domains {
            if matches!(d, Domain::World | Domain::Entities | Domain::Entity)
                && self.state.diagnostics.due(d, std::time::Instant::now())
            {
                let started = std::time::Instant::now();
                let value = self.collect_diagnostic(d);
                d.set(&mut self.state.domains, value);
                self.state.diagnostics.collected(d, started);
            }
        }
    }
    fn publish_diagnostic(
        &mut self,
        domain: rustcraft_control::diagnostics::Domain,
        value: Value,
        started: std::time::Instant,
    ) {
        if self
            .state
            .diagnostics
            .due(domain, std::time::Instant::now())
        {
            domain.set(&mut self.state.domains, value);
            self.state.diagnostics.collected(domain, started);
        }
    }
    fn snapshot(&self) -> Snapshot {
        let mut snapshot = Self::live_snapshot(self.simulation, self.state);
        if !self.state.diagnostics.managed {
            snapshot.world = self.collect_diagnostic(rustcraft_control::diagnostics::Domain::World);
            snapshot.entities =
                self.collect_diagnostic(rustcraft_control::diagnostics::Domain::Entities);
        }
        snapshot.debug = self.state.debug_metadata();
        snapshot
    }
    fn block(&self, p: [i32; 3]) -> ControlResult<String> {
        let p = BlockPos {
            x: p[0],
            y: p[1],
            z: p[2],
        };
        if !self
            .simulation
            .world
            .column_available(rustcraft_engine_core::split_block(p).0)
        {
            return Err("column unavailable".into());
        }
        self.simulation
            .registry
            .get(self.simulation.world.get(p))
            .map(|d| d.name.to_owned())
            .ok_or("unregistered block".into())
    }
    fn apply(&mut self, a: &Action) -> ControlResult<Value> {
        if let Some(result) = self.state.tooling_action(a) {
            return result;
        }
        match a {
            Action::Teleport(p) => {
                let position = Vec3::new(p[0], p[1], p[2]);
                let chunk = rustcraft_engine_core::split_block(BlockPos {
                    x: p[0].floor() as i32,
                    y: p[1].floor() as i32,
                    z: p[2].floor() as i32,
                })
                .0;
                if !self.simulation.world.column_available(chunk) {
                    return Err("teleport destination unavailable".into());
                }
                self.simulation.player.position = position;
                self.simulation.player.velocity = Vec3::ZERO;
                Ok(Value::Null)
            }
            Action::SetBlock {
                position: p,
                key,
                variant,
            } => {
                let position = BlockPos {
                    x: p[0],
                    y: p[1],
                    z: p[2],
                };
                if !(0..128).contains(&p[1])
                    || !self
                        .simulation
                        .world
                        .column_available(rustcraft_engine_core::split_block(position).0)
                {
                    return Err("setblock requires resident column and y=0..127".into());
                }
                let block = self
                    .simulation
                    .registry
                    .definitions()
                    .iter()
                    .find(|d| d.name == key)
                    .map(|d| d.id)
                    .ok_or("unknown semantic block key")?;
                let mut commands = rustcraft_game_api::CommandBuffer::default();
                commands.set_block(
                    position,
                    BlockState {
                        block,
                        variant: *variant,
                    },
                );
                self.simulation.apply_world_commands(&mut commands);
                Ok(json!({"key":key,"position":p}))
            }
            _ => Err("unsupported action".into()),
        }
    }
    fn intent(&mut self, intent: rustcraft_agent_api::AgentIntent) -> ControlResult<()> {
        self.state.intent = intent;
        Ok(())
    }
}
fn tp(args: &[String], _: &Snapshot) -> ControlResult<CommandResult> {
    let [x, y, z] = args else {
        return Err("usage: /tp X Y Z".into());
    };
    let number = |s: &str| {
        s.parse::<f32>()
            .map_err(|_| "invalid coordinate".to_owned())
    };
    Ok(CommandResult::action(Action::Teleport([
        number(x)?,
        number(y)?,
        number(z)?,
    ])))
}
fn setblock(args: &[String], _: &Snapshot) -> ControlResult<CommandResult> {
    let [x, y, z, key] = args else {
        return Err("usage: /setblock X Y Z SEMANTIC_KEY".into());
    };
    let number = |s: &str| {
        s.parse::<i32>()
            .map_err(|_| "invalid coordinate".to_owned())
    };
    Ok(CommandResult::action(Action::SetBlock {
        position: [number(x)?, number(y)?, number(z)?],
        key: key.clone(),
        variant: 0,
    }))
}
pub fn register_commands(registry: &mut Registry) -> ControlResult<()> {
    for (name, usage, cap, handler) in [
        (
            "tp",
            "X Y Z",
            "player.control",
            tp as rustcraft_control::CommandHandler,
        ),
        ("setblock", "X Y Z SEMANTIC_KEY", "world.write", setblock),
    ] {
        registry.register(
            CommandSpec {
                id: format!("minecraft_b173:{name}"),
                name: name.into(),
                aliases: vec![],
                usage: usage.into(),
                help: format!("First-party {name} through semantic control"),
                capability: cap.into(),
            },
            handler,
        )?;
    }
    registry.cache_completion(
        "setblock",
        crate::blocks::BLOCKS.iter().map(|d| d.name.to_owned()),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_control::{Context, Source};
    fn simulation() -> Simulation {
        let profile = crate::compile_profile().unwrap();
        let mut world = rustcraft_engine_core::World::new(profile.default_state().block);
        crate::flat_world_module(&profile).generate(&mut world, -16, 16, -16, 16);
        let mut registry = rustcraft_mod_api::BlockRegistry::default();
        use rustcraft_mod_api::GameplayModule;
        crate::blocks::BlocksModule.register(&mut registry).unwrap();
        Simulation::new(world, registry, Vec3::new(0.5, 3., 0.5))
    }
    #[test]
    fn control_slash_mutation_equivalence_and_dirty_domains() {
        let mut direct = simulation();
        let mut slash = simulation();
        let mut a = ControlState::default();
        let mut b = ControlState::default();
        let context = Context::developer(Source::Scenario);
        let action = Action::SetBlock {
            position: [0, 10, 0],
            key: "minecraft_b173:stone".into(),
            variant: 0,
        };
        rustcraft_control::execute(
            &mut MinecraftHost {
                simulation: &mut direct,
                state: &mut a,
            },
            &context,
            &action,
        )
        .unwrap();
        let mut registry = rustcraft_control::engine_registry();
        register_commands(&mut registry).unwrap();
        let result = registry
            .dispatch(
                &context,
                "/setblock 0 10 0 minecraft_b173:stone",
                &Snapshot::default(),
            )
            .unwrap();
        rustcraft_control::execute(
            &mut MinecraftHost {
                simulation: &mut slash,
                state: &mut b,
            },
            &context,
            &result.action.unwrap(),
        )
        .unwrap();
        let mut rhai_sim = simulation();
        let mut rhai_state = ControlState::default();
        let mut runtime =
            rustcraft_scripting_rhai::RhaiRuntime::new(context.clone(), Default::default());
        let (_, actions) = runtime
            .eval(
                &mut rustcraft_scripting_rhai::RhaiSession::new("equivalence"),
                "set_block(0,10,0,\"minecraft_b173:stone\")",
                Snapshot::default(),
            )
            .unwrap();
        for action in actions {
            rustcraft_control::execute(
                &mut MinecraftHost {
                    simulation: &mut rhai_sim,
                    state: &mut rhai_state,
                },
                &context,
                &action,
            )
            .unwrap();
        }
        assert_eq!(
            rhai_sim.world.state(BlockPos { x: 0, y: 10, z: 0 }),
            direct.world.state(BlockPos { x: 0, y: 10, z: 0 })
        );
        assert_eq!(rhai_sim.dirty_section_count(), direct.dirty_section_count());
        assert!(!rhai_sim.take_persistence_dirty_chunks().is_empty());
        assert_eq!(
            direct.world.state(BlockPos { x: 0, y: 10, z: 0 }),
            slash.world.state(BlockPos { x: 0, y: 10, z: 0 })
        );
        assert_eq!(
            direct.take_dirty_sections().len(),
            slash.take_dirty_sections().len()
        );
        assert_eq!(
            direct.take_persistence_dirty_chunks(),
            slash.take_persistence_dirty_chunks()
        );
        assert!(!slash.take_dirty_chunks().is_empty());
    }
}
