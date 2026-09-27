//! Placeholder first-party flat-world module. M0 turns this into a real world provider.

use rustcraft_mod_api::{GameplayModule, ModuleId};

#[derive(Debug, Default)]
pub struct FlatWorldModule;

impl GameplayModule for FlatWorldModule {
    fn id(&self) -> ModuleId {
        ModuleId("core:flat-world")
    }
}
