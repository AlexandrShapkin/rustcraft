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
                key: block_key(name),
                collision,
                targetable: true,
                material,
                textures: FaceResources::All(texture_key(texture)),
                light: LightDescriptor::default(),
                base_rotation: ModelRotation::IDENTITY,
                orientation: OrientationProperty::None,
                face_tints: [[u16::MAX; 3]; 6],
                capabilities,
            })?;
        }
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
        registry.register_block(VoxelDefinition {
            key: block_key("sandbox_mod:block/reactor"),
            collision: CollisionDescriptor::FullCube,
            targetable: true,
            material: MaterialClass::Opaque,
            textures: FaceResources::All(texture_key("sandbox_mod:textures/block/reactor")),
            light: LightDescriptor {
                emission: 12,
                ..LightDescriptor::default()
            },
            base_rotation: ModelRotation::IDENTITY,
            orientation: OrientationProperty::None,
            face_tints: [[u16::MAX; 3]; 6],
            capabilities: vec![id("sandbox_mod:capability/reactor")],
        })
    }
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
    if context.intent.primary_action {
        context.pulse_on = !context.pulse_on;
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
    let schedule = schedule();
    let profile = compile_profile(schedule.descriptors());
    let ids = RuntimeIds::resolve(&profile);
    let (resources, materials) = compile_render_resources(&profile);
    let mut world = build_world(ids);
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
        assert_eq!(profile.blocks().len(), 5);
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
}
