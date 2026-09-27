use rustcraft_content::ContentManifest;
use rustcraft_gameplay_blocks::BlocksModule;
use rustcraft_gameplay_flat_world::FlatWorldModule;
use rustcraft_mod_api::GameplayModule;
use rustcraft_runtime::RuntimeBootstrap;

fn main() {
    let smoke = std::env::args().any(|arg| arg == "--smoke");

    let mut runtime = RuntimeBootstrap::new(ContentManifest::default());
    runtime.register_module(BlocksModule.id());
    runtime.register_module(FlatWorldModule.id());

    if smoke {
        println!("smoke: runtime initialized with {} modules", runtime.module_count());
    } else {
        println!("rustcraft server bootstrap; run with --smoke for the bootstrap scenario");
    }
}
