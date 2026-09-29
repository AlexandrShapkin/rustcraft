use rustcraft_agent_api::{AgentIntent, MoveIntent};
use rustcraft_bot_api::ScriptedBot;
use rustcraft_content::{
    ContentHash, ContentManifest, PackageDescriptor, PackageId, PackageKind, PackageTarget,
    PackageVersion,
};
use rustcraft_engine_core::{BlockId, BlockPos, Vec3, World};
use rustcraft_minecraft_b173::blocks::{BlocksModule, GRASS, STONE};
use rustcraft_minecraft_b173::flat_world::FlatWorldModule;
use rustcraft_runtime::survival::GameMode;
use rustcraft_runtime::{RuntimeBootstrap, Simulation, run_controller};

fn main() {
    rustcraft_minecraft_b173::validate_package()
        .expect("minecraft_b173 must register through the public Game API");
    let smoke = std::env::args().any(|arg| arg == "--smoke");
    if smoke {
        run_smoke();
    } else if std::env::args().any(|arg| arg == "--survival") {
        run_survival();
    } else {
        println!("rustcraft server bootstrap; run with --smoke for the headless scenario");
    }
}

fn run_survival() {
    let blocks = BlocksModule;
    let mut bootstrap = RuntimeBootstrap::new(Default::default());
    bootstrap.register_module(&blocks).unwrap();
    let mut world = World::new(BlockId(0));
    world.set(
        BlockPos { x: 0, y: 0, z: 0 },
        rustcraft_minecraft_b173::blocks::GRASS.id,
    );
    world.set(
        BlockPos { x: 1, y: 1, z: 0 },
        rustcraft_minecraft_b173::blocks::LOG.id,
    );
    let mut sim = Simulation::new(world, bootstrap.registry, Vec3::new(1.5, 1.0, 1.0));
    sim.set_mode(GameMode::Survival);
    assert!(sim.break_block(BlockPos { x: 1, y: 1, z: 0 }));
    for _ in 0..30 {
        sim.step(AgentIntent::default(), 0.05);
    }
    assert_eq!(sim.items.len(), 0, "drop should be collected");
    let log = rustcraft_minecraft_b173::blocks::LOG.item.unwrap();
    sim.crafting_grid[0] = Some(rustcraft_runtime::inventory::ItemStack {
        item: log,
        count: 1,
        damage: 0,
    });
    assert!(sim.take_crafting_output());
    let planks = rustcraft_minecraft_b173::blocks::PLANKS.item.unwrap();
    assert!(
        sim.inventory
            .slots()
            .iter()
            .any(|s| s.is_some_and(|s| s.item == planks && s.count >= 4))
    );
    println!(
        "survival: drop_pickup=ok craft=log_to_planks inventory_slots={}",
        sim.inventory.occupied()
    );
}

fn run_smoke() {
    let payload = b"minecraft_b173:first-party:blocks:v1";
    let manifest = ContentManifest {
        packages: vec![PackageDescriptor {
            id: PackageId("minecraft_b173:blocks".into()),
            version: PackageVersion(1),
            kind: PackageKind::Data,
            hash: ContentHash::from_bytes(payload),
            dependencies: Vec::new(),
            targets: vec![PackageTarget::Server, PackageTarget::Bot],
        }],
    };
    assert!(manifest.validate_bytes(&PackageId("minecraft_b173:blocks".into()), payload));
    assert_eq!(manifest.for_target(PackageTarget::Bot).len(), 1);
    let mut bootstrap = RuntimeBootstrap::new(manifest);
    let blocks = BlocksModule;
    let flat = FlatWorldModule::default();
    bootstrap
        .register_module(&blocks)
        .expect("block module registration");
    bootstrap
        .register_module(&flat)
        .expect("flat-world registration");
    let mut world = World::new(BlockId(0));
    flat.generate(&mut world, -16, 16, -16, 16);
    assert_eq!(world.get(BlockPos { x: 0, y: 0, z: 0 }), GRASS.id);
    let module_count = bootstrap.module_count();
    let mut simulation = Simulation::new(world, bootstrap.registry, Vec3::new(0.5, 3.0, 0.5));
    simulation
        .inventory
        .insert(STONE.item.unwrap(), 64, &simulation.registry);
    for _ in 0..100 {
        simulation.step(AgentIntent::default(), 0.02);
    }
    simulation.player.pitch = 0.9;
    let target = simulation.target().expect("ground in reach").block;
    let intents = [
        AgentIntent::default(),
        AgentIntent {
            movement: MoveIntent::default(),
            break_block: Some(target),
            ..Default::default()
        },
        AgentIntent {
            use_action: true,
            ..Default::default()
        },
    ];
    let mut bot = ScriptedBot::new(intents.to_vec());
    let before = simulation.observe(3);
    assert_eq!(before.api_version, rustcraft_bot_api::BOT_API_VERSION);
    assert!(!before.nearby_blocks.is_empty());
    run_controller(&mut simulation, &mut bot, 1, 0.02);
    run_controller(&mut simulation, &mut bot, 1, 0.02);
    assert_eq!(simulation.world.get(target), BlockId(0));
    let adjacent = simulation.target().expect("new ground target").adjacent;
    run_controller(&mut simulation, &mut bot, 1, 0.02);
    assert_eq!(simulation.world.get(adjacent), STONE.id);
    assert_eq!(simulation.inventory.held().unwrap().count, 63);
    assert_eq!(
        simulation.observe(1).inventory[0].as_ref().unwrap().count,
        63
    );
    println!(
        "smoke: modules={} chunks={} player_y={:.3} observations={} break/place=ok",
        module_count,
        simulation.world.chunk_count(),
        simulation.player.position.y,
        before.nearby_blocks.len()
    );
}
