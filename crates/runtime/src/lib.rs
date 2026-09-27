//! Runtime/orchestration boundary. The first Codex batch should make this concrete.

use rustcraft_content::ContentManifest;
use rustcraft_mod_api::ModuleId;

#[derive(Debug, Default)]
pub struct RuntimeBootstrap {
    modules: Vec<ModuleId>,
    content: ContentManifest,
}

impl RuntimeBootstrap {
    #[must_use]
    pub fn new(content: ContentManifest) -> Self {
        Self { modules: Vec::new(), content }
    }

    pub fn register_module(&mut self, module: ModuleId) {
        self.modules.push(module);
    }

    #[must_use]
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    #[must_use]
    pub fn content(&self) -> &ContentManifest {
        &self.content
    }
}
