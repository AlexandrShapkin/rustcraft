//! Tiny non-Minecraft architectural integration game.

use rustcraft_agent_api::{AgentIntent, Controller, MoveIntent};
use rustcraft_engine_core::{BlockId, BlockPos, BlockState, Vec3, World};
use rustcraft_game_api::{
    CollisionDescriptor, CommandBuffer, ContentId, FaceResources, GamePackage, GameProfile,
    GameRegistry, LightDescriptor, MaterialClass, RegistrationError, Schedule, ScheduleStage,
    SystemDescriptor, VoxelDefinition,
};
use rustcraft_render::{
    BlockTextureResolver, Face, RenderWorld, TextureTile, build_chunk_mesh,
    offscreen::{self, Scene},
};
use std::path::Path;

const EMPTY: BlockId = BlockId(0);
const FOUNDATION: BlockId = BlockId(1);
const CRYSTAL: BlockId = BlockId(2);
const PULSE: BlockId = BlockId(3);
const TARGET: BlockPos = BlockPos { x: 0, y: 1, z: 0 };

struct SandboxPackage;

impl GamePackage for SandboxPackage {
    fn id(&self) -> ContentId {
        ContentId::new("sandbox_test:package/game")
    }

    fn register(&self, registry: &mut GameRegistry) -> Result<(), RegistrationError> {
        registry.register_package(self.id())?;
        for (id, name, collision, material, texture, capabilities) in [
            (
                EMPTY,
                "sandbox_test:block/empty",
                CollisionDescriptor::Empty,
                MaterialClass::Invisible,
                "sandbox_test:texture/empty",
                vec![],
            ),
            (
                FOUNDATION,
                "sandbox_test:block/foundation",
                CollisionDescriptor::FullCube,
                MaterialClass::Opaque,
                "sandbox_test:texture/foundation",
                vec![ContentId::new("voxel_std:capability/support")],
            ),
            (
                CRYSTAL,
                "sandbox_test:block/crystal",
                CollisionDescriptor::FullCube,
                MaterialClass::Opaque,
                "sandbox_test:texture/crystal",
                vec![ContentId::new("sandbox_test:capability/pulse_source")],
            ),
            (
                PULSE,
                "sandbox_test:block/pulse",
                CollisionDescriptor::FullCube,
                MaterialClass::Opaque,
                "sandbox_test:texture/pulse",
                vec![ContentId::new("sandbox_test:capability/pulse_source")],
            ),
        ] {
            registry.register_block(VoxelDefinition {
                id,
                key: ContentId::new(name),
                collision,
                material,
                textures: FaceResources::All(ContentId::new(texture)),
                light: LightDescriptor::default(),
                capabilities,
            })?;
        }
        Ok(())
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
            BlockState::new(if context.pulse_on { PULSE } else { CRYSTAL }),
        );
    }
}

struct SandboxMaterials<'a>(&'a GameRegistry);

impl BlockTextureResolver for SandboxMaterials<'_> {
    fn texture(&self, block: BlockId, _face: Face) -> Option<TextureTile> {
        self.0.block(block).map(|_| TextureTile { x: 0, y: 0 })
    }

    fn opaque(&self, block: BlockId) -> bool {
        self.0
            .block(block)
            .is_some_and(|definition| definition.material == MaterialClass::Opaque)
    }

    fn visible(&self, block: BlockId) -> bool {
        self.0
            .block(block)
            .is_some_and(|definition| definition.material != MaterialClass::Invisible)
    }

    fn tint(&self, block: BlockId, face: Face) -> [f32; 3] {
        let color = match block {
            FOUNDATION => [0.16, 0.22, 0.30],
            CRYSTAL => [0.08, 0.78, 0.95],
            PULSE => [1.0, 0.25, 0.72],
            _ => [1.0; 3],
        };
        let light = match face {
            Face::Top => 1.0,
            Face::Bottom => 0.55,
            Face::East | Face::West => 0.75,
            Face::North | Face::South => 0.88,
        };
        color.map(|channel| channel * light)
    }
}

fn profile(descriptors: Vec<SystemDescriptor>) -> GameProfile {
    GameProfile {
        id: ContentId::new("sandbox_test:profile/default"),
        packages: vec![
            ContentId::new("voxel_std:package/base"),
            ContentId::new("sandbox_test:package/game"),
        ],
        resources: vec![ContentId::new("sandbox_test:resources/generated_debug")],
        systems: descriptors,
        manifest: rustcraft_content::ContentManifest::default(),
    }
}

fn build_world() -> World {
    let mut world = World::new(EMPTY);
    world.fill_box(
        BlockPos { x: -3, y: 0, z: -3 },
        BlockPos { x: 3, y: 0, z: 3 },
        FOUNDATION,
    );
    world.set(TARGET, CRYSTAL);
    world.set(BlockPos { x: -2, y: 1, z: 1 }, CRYSTAL);
    world.set(BlockPos { x: 2, y: 1, z: -1 }, PULSE);
    world
}

fn to_scene(world: &World, registry: &GameRegistry, observer: Vec3) -> Scene {
    let presentation = RenderWorld::from_world(world);
    let materials = SandboxMaterials(registry);
    let mut vertices = Vec::new();
    for chunk in presentation.chunks() {
        let mesh = build_chunk_mesh(&presentation, chunk, &materials);
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

fn main() {
    let mut registry = GameRegistry::default();
    SandboxPackage.register(&mut registry).unwrap();

    let mut schedule = Schedule::default();
    schedule
        .register(
            SystemDescriptor {
                id: ContentId::new("sandbox_test:system/observer"),
                stage: ScheduleStage::FixedUpdate,
            },
            observer_system,
        )
        .unwrap();
    schedule
        .register(
            SystemDescriptor {
                id: ContentId::new("sandbox_test:system/pulse"),
                stage: ScheduleStage::FixedUpdate,
            },
            pulse_system,
        )
        .unwrap();
    let game_profile = profile(schedule.descriptors());
    game_profile.validate().unwrap();

    let mut world = build_world();
    let mut controller = DemoController::default();
    let mut context = SandboxContext {
        intent: controller.next_intent(),
        observer: Vec3::new(0.0, 2.5, 5.0),
        pulse_on: false,
    };
    let mut commands = CommandBuffer::default();
    schedule.run(ScheduleStage::FixedUpdate, &mut context, &mut commands);
    assert_eq!(
        world.get(TARGET),
        CRYSTAL,
        "systems cannot mutate World directly"
    );
    assert_eq!(commands.apply(&mut world), 1);
    assert_eq!(world.get(TARGET), PULSE);

    let output = Path::new("target/sample-game/sandbox-test.png");
    pollster::block_on(offscreen::render(
        &to_scene(&world, &registry, context.observer),
        None,
        output,
    ))
    .expect("render sandbox_test offscreen scene");
    println!(
        "sample-game: profile={} packages={} blocks={} systems={} observer=({:.2},{:.2},{:.2}) command=set_block output={}",
        game_profile.id.as_str(),
        game_profile.packages.len(),
        registry.blocks().len(),
        game_profile.systems.len(),
        context.observer.x,
        context.observer.y,
        context.observer.z,
        output.display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_profile_and_custom_interaction_are_minecraft_independent() {
        let mut registry = GameRegistry::default();
        SandboxPackage.register(&mut registry).unwrap();
        assert_eq!(registry.blocks().len(), 4);
        assert!(
            registry
                .blocks()
                .iter()
                .all(|block| block.key.as_str().starts_with("sandbox_test:"))
        );
        let mut schedule = Schedule::default();
        schedule
            .register(
                SystemDescriptor {
                    id: ContentId::new("sandbox_test:system/pulse"),
                    stage: ScheduleStage::FixedUpdate,
                },
                pulse_system,
            )
            .unwrap();
        let mut context = SandboxContext {
            intent: AgentIntent {
                primary_action: true,
                ..Default::default()
            },
            observer: Vec3::ZERO,
            pulse_on: false,
        };
        let mut commands = CommandBuffer::default();
        let mut world = build_world();
        schedule.run(ScheduleStage::FixedUpdate, &mut context, &mut commands);
        assert_eq!(commands.apply(&mut world), 1);
        assert_eq!(world.get(TARGET), PULSE);
    }
}
