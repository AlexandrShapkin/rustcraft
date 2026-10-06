//! Tiny non-Minecraft architectural integration game.

use rustcraft_agent_api::{AgentIntent, Controller, MoveIntent};
use rustcraft_content::{
    PackageId, ResourceId,
    resources::{
        AtlasPolicy, ResourceLimits, compile_resources, discover_package, write_atlas_debug,
    },
};
use rustcraft_engine_core::orientation::{ModelRotation, OrientationProperty};
use rustcraft_engine_core::{BlockId, BlockPos, BlockState, Vec3, World};
use rustcraft_game_api::{
    BlockKey, CollisionDescriptor, CommandBuffer, CompiledGameProfile, ContentId, FaceResources,
    GamePackage, GameProfile, GameRegistry, LightDescriptor, MaterialClass, RegistrationError,
    Schedule, ScheduleStage, SystemDescriptor, TextureKey, VoxelDefinition,
};
use rustcraft_render::{
    RenderWorld, build_chunk_mesh,
    offscreen::{self, Scene},
};
use rustcraft_render_profile::{CompiledTextureRegistry, CompiledVoxelRenderRegistry};
use std::path::{Path, PathBuf};

mod bg1;

const TARGET: BlockPos = BlockPos { x: 0, y: 1, z: 0 };

fn id(value: &str) -> ContentId {
    ContentId::parse(value).unwrap()
}

fn package_id(value: &str) -> PackageId {
    PackageId::parse(value).unwrap()
}

fn resource_id(value: &str) -> ResourceId {
    ResourceId::parse(value).unwrap()
}

fn block_key(value: &str) -> BlockKey {
    BlockKey::parse(value).unwrap()
}

fn texture_key(value: &str) -> TextureKey {
    TextureKey::parse(value).unwrap()
}

struct SandboxPackage;

impl GamePackage for SandboxPackage {
    fn id(&self) -> PackageId {
        package_id("sandbox_test:package/game")
    }

    fn register(&self, registry: &mut GameRegistry) -> Result<(), RegistrationError> {
        registry.register_package(self.id())?;
        for (name, collision, material, texture, capabilities) in [
            (
                "sandbox_test:block/empty",
                CollisionDescriptor::Empty,
                MaterialClass::Invisible,
                "sandbox_test:textures/block/empty",
                vec![],
            ),
            (
                "sandbox_test:block/foundation",
                CollisionDescriptor::FullCube,
                MaterialClass::Opaque,
                "sandbox_test:textures/block/foundation",
                vec![id("voxel_std:capability/support")],
            ),
            (
                "sandbox_test:block/crystal",
                CollisionDescriptor::FullCube,
                MaterialClass::Opaque,
                "sandbox_test:textures/block/crystal",
                vec![id("sandbox_test:capability/pulse_source")],
            ),
            (
                "sandbox_test:block/pulse",
                CollisionDescriptor::FullCube,
                MaterialClass::Opaque,
                "sandbox_test:textures/block/pulse",
                vec![id("sandbox_test:capability/pulse_source")],
            ),
        ] {
            registry.register_block(VoxelDefinition {
                geometry: None,
                common: rustcraft_game_api::ContentDefinition::new(block_key(name).as_id().clone())
                    .with_capabilities(capabilities),
                state_schema: None,
                collision,
                targetable: true,
                material,
                textures: FaceResources::All(texture_key(texture)),
                light: LightDescriptor::default(),
                base_rotation: ModelRotation::IDENTITY,
                orientation: OrientationProperty::None,
                face_tints: [[u16::MAX; 3]; 6],
            })?;
        }
        bg1::register(registry)?;
        Ok(())
    }
}

struct SandboxModPackage;

impl GamePackage for SandboxModPackage {
    fn id(&self) -> PackageId {
        package_id("sandbox_mod:package/game")
    }

    fn register(&self, registry: &mut GameRegistry) -> Result<(), RegistrationError> {
        registry.register_package(self.id())?;
        use rustcraft_game_api::state_schema::{StateDomain, StateField, StateSchema, StateValue};
        use rustcraft_game_api::{
            ContentDefinition, ContentEvent, DefinitionProperties, HandlerBinding, ItemDefinition,
        };
        registry.register_handler(id("sandbox_mod:handler/reactor_use"), reactor_use)?;
        let mut common = ContentDefinition::new(id("sandbox_mod:block/reactor"));
        common.properties = DefinitionProperties {
            mass_kg: Some(2.),
            friction: Some(0.4),
        };
        common.tags = vec![id("sandbox_mod:tag/diagnostic")];
        common.capabilities = vec![
            id("voxel_std:capability/interactable"),
            id("sandbox_mod:capability/reactor"),
        ];
        common.resources = vec![resource_id("sandbox_mod:textures/block/reactor")];
        common.handlers = vec![HandlerBinding {
            event: ContentEvent::Use,
            key: id("sandbox_mod:handler/reactor_use"),
        }];
        registry.register_block(VoxelDefinition {
            geometry: Some(bg1::reactor_binding()),
            common,
            state_schema: Some(StateSchema {
                fields: vec![
                    StateField {
                        key: id("voxel_std:state/facing"),
                        domain: StateDomain::Facing,
                        default: StateValue::Facing(
                            rustcraft_engine_core::orientation::Facing::North,
                        ),
                    },
                    StateField {
                        key: id("voxel_std:state/powered"),
                        domain: StateDomain::Powered,
                        default: StateValue::Powered(false),
                    },
                ],
                ..Default::default()
            }),
            collision: CollisionDescriptor::FullCube,
            targetable: true,
            material: MaterialClass::Opaque,
            textures: FaceResources::All(texture_key("sandbox_mod:textures/block/reactor")),
            light: LightDescriptor {
                emission: 12,
                ..Default::default()
            },
            base_rotation: ModelRotation::IDENTITY,
            orientation: OrientationProperty::None,
            face_tints: [[u16::MAX; 3]; 6],
        })?;
        let mut common = ContentDefinition::new(id("sandbox_mod:item/charge"));
        common.properties.mass_kg = Some(0.25);
        common.tags = vec![id("sandbox_mod:tag/diagnostic")];
        common.capabilities = vec![id("voxel_std:capability/stackable")];
        common.resources = vec![resource_id("sandbox_mod:textures/block/reactor")];
        registry.register_item(ItemDefinition {
            common,
            max_stack: 16,
            icon: Some(texture_key("sandbox_mod:textures/block/reactor")),
        })
    }
}

fn reactor_use(
    context: &rustcraft_game_api::HandlerContext<'_>,
    commands: &mut CommandBuffer,
) -> Result<(), rustcraft_game_api::state_schema::StateError> {
    use rustcraft_game_api::state_schema::StateValue;
    let schema = &context.definition.state_schema;
    // Typed immutable parameter; neither a classification tag nor a security grant.
    if context
        .definition
        .common
        .properties
        .mass_kg
        .is_none_or(|mass| mass <= 0.)
    {
        return Ok(());
    }
    let mut values = schema.decode(context.state.variant)?;
    for assignment in &mut values {
        if let StateValue::Powered(value) = &mut assignment.value {
            *value = !*value;
        }
        if let StateValue::Facing(value) = &mut assignment.value {
            *value = rustcraft_engine_core::orientation::Facing::East;
        }
    }
    commands.set_block(
        context.position,
        BlockState {
            variant: schema.encode(&values)?,
            ..context.state
        },
    );
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct RuntimeIds {
    empty: BlockId,
    foundation: BlockId,
    crystal: BlockId,
    pulse: BlockId,
    reactor: BlockId,
}

impl RuntimeIds {
    fn resolve(profile: &CompiledGameProfile) -> Self {
        let get = |key| profile.block_id(&block_key(key)).unwrap();
        Self {
            empty: profile.default_state().block,
            foundation: get("sandbox_test:block/foundation"),
            crystal: get("sandbox_test:block/crystal"),
            pulse: get("sandbox_test:block/pulse"),
            reactor: get("sandbox_mod:block/reactor"),
        }
    }
}

#[derive(Default)]
struct DemoController {
    emitted: bool,
}

impl Controller for DemoController {
    fn next_intent(&mut self) -> AgentIntent {
        let intent = AgentIntent {
            movement: MoveIntent {
                forward: 1.0,
                strafe: 0.25,
            },
            primary_action: !self.emitted,
            ..Default::default()
        };
        self.emitted = true;
        intent
    }
}

struct SandboxContext {
    intent: AgentIntent,
    observer: Vec3,
    pulse_on: bool,
    crystal: BlockId,
    pulse: BlockId,
}

fn observer_system(context: &mut SandboxContext, _commands: &mut CommandBuffer) {
    context.observer.x += context.intent.movement.strafe * 0.05;
    context.observer.z += context.intent.movement.forward * 0.05;
}

fn pulse_system(context: &mut SandboxContext, commands: &mut CommandBuffer) {
    if context.intent.primary_action || context.intent.secondary_action {
        context.pulse_on = !context.intent.secondary_action && !context.pulse_on;
        commands.set_block(
            TARGET,
            BlockState::new(if context.pulse_on {
                context.pulse
            } else {
                context.crystal
            }),
        );
    }
}

fn authored_profile(descriptors: Vec<SystemDescriptor>) -> GameProfile {
    GameProfile {
        id: id("sandbox_test:profile/default"),
        packages: vec![
            package_id("sandbox_test:package/game"),
            package_id("sandbox_mod:package/game"),
        ],
        resources: vec![
            resource_id("sandbox_test:resources/native"),
            resource_id("sandbox_mod:resources/native"),
        ],
        systems: descriptors,
        manifest: rustcraft_content::ContentManifest::default(),
        default_block: block_key("sandbox_test:block/empty"),
    }
}

fn compile_profile(descriptors: Vec<SystemDescriptor>) -> CompiledGameProfile {
    let mut registry = GameRegistry::default();
    SandboxPackage.register(&mut registry).unwrap();
    SandboxModPackage.register(&mut registry).unwrap();
    registry.compile(&authored_profile(descriptors)).unwrap()
}

fn build_world(ids: RuntimeIds) -> World {
    let mut world = World::new(ids.empty);
    world.fill_box(
        BlockPos { x: -3, y: 0, z: -3 },
        BlockPos { x: 3, y: 0, z: 3 },
        ids.foundation,
    );
    world.set(TARGET, ids.crystal);
    world.set(BlockPos { x: -2, y: 1, z: 1 }, ids.crystal);
    world.set(BlockPos { x: 2, y: 1, z: -1 }, ids.pulse);
    world.set(BlockPos { x: 0, y: 1, z: -2 }, ids.reactor);
    world
}

fn to_scene(world: &World, materials: &CompiledVoxelRenderRegistry, observer: Vec3) -> Scene {
    let presentation = RenderWorld::from_world(world);
    let mut vertices = Vec::new();
    for chunk in presentation.chunks() {
        let mesh = build_chunk_mesh(&presentation, chunk, materials);
        for index in mesh.indices {
            let mut vertex = mesh.vertices[index as usize];
            let [x, y, z] = vertex.position;
            let relative_x = x - observer.x;
            let relative_z = z - (observer.z - 5.0);
            vertex.position = [
                (relative_x - relative_z) * 0.11,
                (relative_x + relative_z) * 0.055 + y * 0.14 - 0.35,
                0.5 - (x + y + z) * 0.006,
            ];
            vertex.shade = 1.0;
            vertices.push(vertex);
        }
    }
    Scene {
        vertices,
        width: 640,
        height: 360,
        textured: false,
        cull: false,
    }
}

fn compile_render_resources(
    profile: &CompiledGameProfile,
) -> (
    rustcraft_content::resources::CompiledResources,
    CompiledVoxelRenderRegistry,
) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-packages");
    let base = discover_package(
        root.join("base"),
        package_id("sandbox_test:package/resources"),
        ResourceLimits::default(),
    )
    .unwrap();
    let extension = discover_package(
        root.join("mod"),
        package_id("sandbox_mod:package/resources"),
        ResourceLimits::default(),
    )
    .unwrap();
    let resources = compile_resources(
        &[base, extension],
        AtlasPolicy {
            page_width: 128,
            page_height: 128,
            padding: 2,
            ..AtlasPolicy::default()
        },
        ResourceLimits::default(),
        Some(Path::new("target/resource-cache/sandbox-test")),
    )
    .unwrap();
    let textures = CompiledTextureRegistry::from_resources(&resources).unwrap();
    let render = CompiledVoxelRenderRegistry::compile(profile, &textures).unwrap();
    (resources, render)
}

fn schedule() -> Schedule<SandboxContext> {
    let mut schedule = Schedule::default();
    schedule
        .register(
            SystemDescriptor {
                id: id("sandbox_test:system/observer"),
                stage: ScheduleStage::FixedUpdate,
            },
            observer_system,
        )
        .unwrap();
    schedule
        .register(
            SystemDescriptor {
                id: id("sandbox_test:system/pulse"),
                stage: ScheduleStage::FixedUpdate,
            },
            pulse_system,
        )
        .unwrap();
    schedule
}

fn main() {
    let mut text =
        rustcraft_render::text::TextSystem::new(rustcraft_content::fonts::FontResources::builtin())
            .expect("bundled text");
    text.update(
        &[rustcraft_render::text::TextRun {
            text: "Sandbox: Українська Ελληνικά".into(),
            position: [0., 0.],
            pixels: 12.,
            role: "rustcraft:font/ui".into(),
            color: [255; 3],
        }],
        640,
        64,
        None,
    );
    assert!(text.rgba.iter().any(|b| *b != 0));
    assert_eq!(text.metrics.replacements, 0);
    println!("SANDBOX_TEXT bundled semantic Unicode pass");

    use rustcraft_control::config::{Kind, Policy, Registry as ConfigRegistry, Spec, Value};
    let mut config = ConfigRegistry::default();
    config
        .register(Spec {
            key: "sandbox_test:settings/pulse_enabled".into(),
            kind: Kind::Bool,
            default: Value::Bool(true),
            owner: "sandbox_test:package/game".into(),
            description: "Independent package-owned operational setting".into(),
            unit: "".into(),
            policy: Policy::Immediate,
            persist: true,
            reason: "".into(),
            availability: "available".into(),
        })
        .unwrap();
    config.open();
    config
        .request(&[(
            "sandbox_test:settings/pulse_enabled".into(),
            Some(Value::Bool(false)),
        )])
        .unwrap();
    config.apply(Policy::Immediate, 0, |_| Ok(())).unwrap();
    assert!(
        !config
            .effective("sandbox_test:settings/pulse_enabled")
            .boolean()
    );
    println!("SANDBOX_CONFIG independent registration/apply/readback=pass");

    let context = rustcraft_control::Context::read_only(rustcraft_control::Source::Script);
    let mut runtime = rustcraft_scripting_rhai::RhaiRuntime::new(context, Default::default());
    let result = runtime
        .eval(
            &mut rustcraft_scripting_rhai::RhaiSession::new("sandbox"),
            "control_version()",
            Default::default(),
        )
        .expect("game-neutral scripting");
    assert_eq!(result.0, rustcraft_control::CONTROL_API_VERSION.to_string());

    // Independent native game registration and shared observation proof, without runtime/Minecraft.
    use rustcraft_control::diagnostics::{Cost, DebugView, Domain, ViewKind};
    let mut diagnostics = rustcraft_control::ControlState::default();
    diagnostics
        .diagnostics
        .registry
        .register(DebugView {
            id: "sandbox_test:debug/pulse".into(),
            name: "pulse".into(),
            title: "Sandbox pulse".into(),
            description: "Native sandbox semantic world state".into(),
            owner: "sandbox_test:package/game".into(),
            kind: ViewKind::Page,
            cost: Cost::Low,
            shortcut: None,
            requirements: vec![Domain::World],
        })
        .unwrap();
    diagnostics
        .view_action(&rustcraft_control::Action::DebugPage(
            "sandbox_test:debug/pulse".into(),
        ))
        .unwrap();
    diagnostics.domains.world = serde_json::json!({"pulse":true});
    assert_eq!(diagnostics.diagnostic_demand(), [Domain::World]);
    assert!(diagnostics.diagnostic_text().contains("pulse: true"));
    let observed = runtime
        .eval(
            &mut rustcraft_scripting_rhai::RhaiSession::new("sandbox-diagnostics"),
            "world().pulse",
            diagnostics.domains.clone(),
        )
        .unwrap();
    assert_eq!(observed.0, "true");
    println!("SANDBOX_DEBUG shared native page + Rhai world().pulse = true");

    let schedule = schedule();
    let profile = compile_profile(schedule.descriptors());
    let ids = RuntimeIds::resolve(&profile);
    let (resources, materials) = compile_render_resources(&profile);
    let mut world = build_world(ids);
    bg1::add_scene(&mut world, &profile);
    let mut controller = DemoController::default();
    let mut context = SandboxContext {
        intent: controller.next_intent(),
        observer: Vec3::new(0.0, 2.5, 5.0),
        pulse_on: false,
        crystal: ids.crystal,
        pulse: ids.pulse,
    };
    let mut commands = CommandBuffer::default();
    schedule.run(ScheduleStage::FixedUpdate, &mut context, &mut commands);
    assert_eq!(world.get(TARGET), ids.crystal);
    assert_eq!(commands.apply(&mut world), 1);
    assert_eq!(world.get(TARGET), ids.pulse);

    let reactor_position = BlockPos { x: 2, y: 1, z: 0 };
    world.set_state(reactor_position, BlockState::new(ids.reactor));
    let mut effects = profile
        .dispatch(
            rustcraft_game_api::ContentEvent::Use,
            reactor_position,
            world.state(reactor_position),
        )
        .expect("admit reactor use");
    assert_eq!(effects.apply(&mut world), 1);

    let output = Path::new("target/sample-game/sandbox-test.png");
    let atlas_outputs = write_atlas_debug(&resources, Path::new("target/sample-game/atlas"))
        .expect("write sandbox atlas diagnostics");
    let page = &resources.pages[0];
    pollster::block_on(offscreen::render_rgba(
        &to_scene(&world, &materials, context.observer),
        page.width,
        page.height,
        &page.rgba,
        output,
    ))
    .expect("render sandbox_test offscreen scene");
    bg1::render_proof_scene(&profile, &materials, &resources);
    println!(
        "sample-game: profile={} packages={} blocks={} systems={} textures={} pages={} override={} cache={} observer=({:.2},{:.2},{:.2}) command=set_block output={} atlas_debug={}",
        profile.id,
        profile.packages.len(),
        profile.blocks().len(),
        profile.systems.len(),
        resources.textures.len(),
        resources.pages.len(),
        resources.overrides.len(),
        if resources.metrics.cache_hit {
            "hit"
        } else {
            "miss"
        },
        context.observer.x,
        context.observer.y,
        context.observer.z,
        output.display(),
        atlas_outputs[0].display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TinySandboxGenerator {
        air: BlockId,
        foundation: BlockId,
    }
    impl rustcraft_world::ChunkGenerator for TinySandboxGenerator {
        fn id(&self) -> &str {
            "sandbox_test:tiny"
        }
        fn version(&self) -> u32 {
            1
        }
        fn generate(
            &self,
            _seed: i64,
            _position: rustcraft_engine_core::ChunkPos,
        ) -> Result<Vec<(i32, rustcraft_engine_core::Chunk)>, rustcraft_world::WorldError> {
            let mut section = rustcraft_engine_core::ChunkBuilder::new(BlockState::new(self.air));
            for z in 0..16 {
                for x in 0..16 {
                    section.set((x, 0, z), BlockState::new(self.foundation));
                }
            }
            Ok(vec![(0, section.finish())])
        }
    }

    #[test]
    fn semantic_profile_compiles_without_minecraft_or_authored_numeric_ids() {
        let schedule = schedule();
        let profile = compile_profile(schedule.descriptors());
        let ids = RuntimeIds::resolve(&profile);
        let (resources, render) = compile_render_resources(&profile);
        assert_eq!(profile.blocks().len(), 10);
        assert_eq!(profile.default_state().block, ids.empty);
        assert!(
            profile
                .blocks()
                .iter()
                .all(|block| block.key.as_str().starts_with("sandbox_test:")
                    || block.key.as_str().starts_with("sandbox_mod:"))
        );
        let crystal = ResourceId::parse("sandbox_test:textures/block/crystal").unwrap();
        assert_eq!(
            resources.texture(&crystal).unwrap().provider,
            package_id("sandbox_mod:package/resources")
        );
        assert!(render.block(ids.reactor).is_some_and(|block| block.visible));

        let mut context = SandboxContext {
            intent: AgentIntent {
                primary_action: true,
                ..Default::default()
            },
            observer: Vec3::ZERO,
            pulse_on: false,
            crystal: ids.crystal,
            pulse: ids.pulse,
        };
        let mut commands = CommandBuffer::default();
        let mut world = build_world(ids);
        schedule.run(ScheduleStage::FixedUpdate, &mut context, &mut commands);
        assert_eq!(commands.apply(&mut world), 1);
        assert_eq!(world.get(TARGET), ids.pulse);
        context.intent = AgentIntent {
            secondary_action: true,
            ..Default::default()
        };
        schedule.run(ScheduleStage::FixedUpdate, &mut context, &mut commands);
        assert_eq!(commands.apply(&mut world), 1);
        assert_eq!(world.get(TARGET), ids.crystal);
    }

    #[test]
    fn a1_independent_game_uses_public_work_and_recipe_mechanisms() {
        use rustcraft_game_api::{RecipeDefinition, WorkDefinition, WorkProgress, recipe_matches};
        let crystal = ResourceId::parse("sandbox_test:crystal").unwrap();
        let charge = ResourceId::parse("sandbox_test:charge").unwrap();
        let work = WorkDefinition {
            target: BlockKey::parse("sandbox_test:pulse").unwrap(),
            duration_seconds: 0.1,
            item_multipliers: vec![(crystal.clone(), 2.)],
            reward: Some((charge.clone(), 1)),
        };
        let mut progress = WorkProgress::default();
        assert!(progress.advance(0.05, work.duration_seconds, work.item_multipliers[0].1));
        let recipe = RecipeDefinition {
            key: ResourceId::parse("sandbox_test:recipe/charge").unwrap(),
            local_alias: None,
            shaped: false,
            width: 1,
            inputs: vec![Some(crystal.clone())],
            output: charge,
            count: 1,
        };
        assert!(recipe_matches(
            recipe.shaped,
            &[None, Some(crystal)],
            &recipe.inputs
        ));
        assert!(!recipe_matches(
            recipe.shaped,
            &[] as &[Option<ResourceId>],
            &recipe.inputs
        ));
        assert!(!WorkProgress::default().advance(f32::NAN, 1., 1.));
    }

    #[test]
    fn sandbox_game_supplies_its_own_generic_chunk_generation_policy() {
        let profile = compile_profile(schedule().descriptors());
        let ids = RuntimeIds::resolve(&profile);
        let generator = TinySandboxGenerator {
            air: ids.empty,
            foundation: ids.foundation,
        };
        let output = rustcraft_world::ChunkGenerator::generate(
            &generator,
            55,
            rustcraft_engine_core::ChunkPos { x: -2, z: 3 },
        )
        .unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].1.get((0, 0, 0)), ids.foundation);
        assert_eq!(output[0].1.get((0, 1, 0)), ids.empty);
    }
    #[test]
    fn c2_common_categories_state_handler_and_render_consumer() {
        use rustcraft_game_api::ContentEvent;
        let profile = compile_profile(schedule().descriptors());
        let ids = RuntimeIds::resolve(&profile);
        let definition = profile.block(ids.reactor).unwrap();
        let tag = profile
            .tag_index(&id("sandbox_mod:tag/diagnostic"))
            .unwrap();
        let capability = profile
            .capability_index(&id("voxel_std:capability/interactable"))
            .unwrap();
        assert!(definition.common.has_tag(tag));
        assert!(definition.common.has_capability(capability));
        let item = profile
            .item(profile.item_id(&id("sandbox_mod:item/charge")).unwrap())
            .unwrap();
        assert!(item.common.has_tag(tag));
        assert!(!item.common.has_capability(capability));
        assert_eq!(item.max_stack, 16);
        let mut world = build_world(ids);
        world.set_state(TARGET, BlockState::new(ids.reactor));
        let mut commands = profile
            .dispatch(ContentEvent::Use, TARGET, world.state(TARGET))
            .unwrap();
        assert_eq!(
            world.state(TARGET).variant,
            0,
            "handler cannot mutate world directly"
        );
        assert_eq!(commands.apply(&mut world), 1);
        let (_, render) = compile_render_resources(&profile);
        let state = world.state(TARGET);
        assert_eq!(
            render.block_model(state).unwrap().rotation,
            ModelRotation::facing(rustcraft_engine_core::orientation::Facing::East)
        );
        assert_eq!(
            profile
                .import_state(&definition.key, &profile.export_state(state).unwrap().1)
                .unwrap(),
            state
        );
    }

    #[test]
    fn c2_existing_chunk_codec_reopens_canonical_variant_with_reordered_profile() {
        use rustcraft_world::{SemanticBlockResolver, WorldStorage};
        struct Resolver(CompiledGameProfile);
        impl SemanticBlockResolver for Resolver {
            fn key_for(&self, state: BlockState) -> Option<&str> {
                let definition = self.0.block(state.block)?;
                definition.state_schema.validate(state.variant).ok()?;
                Some(definition.key.as_str())
            }
            fn state_for(&self, key: &str, variant: u16) -> Option<BlockState> {
                let block = self.0.block_id(&block_key(key))?;
                self.0.block(block)?.state_schema.validate(variant).ok()?;
                Some(BlockState { block, variant })
            }
        }
        let mut registry = GameRegistry::default();
        SandboxPackage.register(&mut registry).unwrap();
        SandboxModPackage.register(&mut registry).unwrap();
        let authored = authored_profile(vec![]);
        let forward = Resolver(registry.compile(&authored).unwrap());
        let mut reordered = authored;
        reordered.packages.reverse();
        let reverse = Resolver(registry.compile(&reordered).unwrap());
        let a = RuntimeIds::resolve(&forward.0);
        let b = RuntimeIds::resolve(&reverse.0);
        assert_ne!(a.reactor, b.reactor);
        let mut commands = forward
            .0
            .dispatch(
                rustcraft_game_api::ContentEvent::Use,
                TARGET,
                BlockState::new(a.reactor),
            )
            .unwrap();
        let mut world = World::new(a.empty);
        commands.apply(&mut world);
        let state = world.state(TARGET);
        let mut section = rustcraft_engine_core::Chunk::new(a.empty);
        section.set_state((0, 1, 0), state);
        let pos = rustcraft_engine_core::ChunkPos { x: 0, z: 0 };
        let stored = WorldStorage::encode_runtime_chunk(pos, [(0, section)], &forward).unwrap();
        let root = std::env::temp_dir().join(format!("rustcraft-c2-codec-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let storage = WorldStorage::open(&root, "disposable").unwrap();
        storage.store_chunk_measured(&stored).unwrap();
        drop(storage);
        let reopened = WorldStorage::open(&root, "disposable").unwrap();
        let sections = reopened.load_runtime_chunk(pos, &reverse).unwrap();
        assert_eq!(
            sections[0].1.state((0, 1, 0)),
            BlockState {
                block: b.reactor,
                variant: state.variant
            }
        );
        assert_eq!(
            forward.0.export_state(state).unwrap(),
            reverse
                .0
                .export_state(sections[0].1.state((0, 1, 0)))
                .unwrap()
        );
        assert!(
            reverse
                .state_for("sandbox_mod:block/reactor", 999)
                .is_none()
        );
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }
}
