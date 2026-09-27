//! Semantic contracts for gameplay registration. Intentionally minimal at bootstrap.

use rustcraft_engine_core::BlockId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleId(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockDefinition {
    pub id: BlockId,
    pub name: &'static str,
}

pub trait GameplayModule {
    fn id(&self) -> ModuleId;
}
