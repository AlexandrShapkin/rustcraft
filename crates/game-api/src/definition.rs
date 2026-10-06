//! Typed composition shared by categories. Content contracts are not host permission grants.
use crate::{ContentId, RegistrationError, TextureKey};
use rustcraft_content::ResourceId;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DefinitionProperties {
    /// Physical mass in kg; games decide whether/how to use it.
    pub mass_kg: Option<f32>,
    /// Nonnegative physical surface friction coefficient.
    pub friction: Option<f32>,
}
impl DefinitionProperties {
    pub fn validate(&self) -> Result<(), RegistrationError> {
        if [self.mass_kg, self.friction]
            .into_iter()
            .flatten()
            .any(|v| !v.is_finite() || v < 0.)
        {
            Err(RegistrationError::InvalidDefinition)
        } else {
            Ok(())
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefinitionMetadata {
    pub display_name: Option<String>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ContentDefinition {
    pub key: ContentId,
    pub metadata: DefinitionMetadata,
    pub properties: DefinitionProperties,
    pub tags: Vec<ContentId>,
    pub capabilities: Vec<ContentId>,
    pub resources: Vec<ResourceId>,
    pub handlers: Vec<HandlerBinding>,
}
impl ContentDefinition {
    pub fn new(key: ContentId) -> Self {
        Self {
            key,
            metadata: Default::default(),
            properties: Default::default(),
            tags: vec![],
            capabilities: vec![],
            resources: vec![],
            handlers: vec![],
        }
    }
    pub fn with_capabilities(mut self, capabilities: Vec<ContentId>) -> Self {
        self.capabilities = capabilities;
        self
    }
    pub fn validate(&self) -> Result<(), RegistrationError> {
        self.properties.validate()?;
        if self
            .metadata
            .display_name
            .as_ref()
            .is_some_and(|v| v.len() > 256)
            || self.handlers.len() > 1
            || self.tags.len() > 256
            || self.capabilities.len() > 256
            || self.resources.len() > 256
        {
            return Err(RegistrationError::InvalidDefinition);
        }
        if !self.handlers.is_empty()
            && !self
                .capabilities
                .iter()
                .any(|id| id.as_str() == "voxel_std:capability/interactable")
        {
            return Err(RegistrationError::InvalidDefinition);
        }
        Ok(())
    }
}
/// Item category adds stack semantics; no voxel geometry, Minecraft tool tier or host grants.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemDefinition {
    pub common: ContentDefinition,
    pub max_stack: u16,
    pub icon: Option<TextureKey>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledItemDefinition {
    pub id: ItemDefinitionId,
    pub common: CompiledContentDefinition,
    pub max_stack: u16,
    pub icon: Option<TextureKey>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ItemDefinitionId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TagIndex(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityIndex(pub u32);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedMembership(Vec<u64>);
impl IndexedMembership {
    fn compile(ids: &[ContentId], catalog: &HashMap<ContentId, u32>) -> Self {
        let mut bits = vec![0; catalog.len().div_ceil(64)];
        for id in ids {
            let slot = catalog[id] as usize;
            bits[slot / 64] |= 1 << (slot % 64);
        }
        Self(bits)
    }
    pub fn contains(&self, index: u32) -> bool {
        self.0
            .get(index as usize / 64)
            .is_some_and(|word| word & (1 << (index % 64)) != 0)
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledContentDefinition {
    pub key: ContentId,
    pub metadata: DefinitionMetadata,
    pub properties: DefinitionProperties,
    tags: IndexedMembership,
    capabilities: IndexedMembership,
    pub resources: Vec<ResourceId>,
    pub(crate) use_handler: Option<usize>,
}
impl CompiledContentDefinition {
    pub fn has_tag(&self, index: TagIndex) -> bool {
        self.tags.contains(index.0)
    }
    pub fn has_capability(&self, index: CapabilityIndex) -> bool {
        self.capabilities.contains(index.0)
    }
}
#[derive(Debug, Clone, Default)]
pub(crate) struct DefinitionCatalog {
    pub tags: HashMap<ContentId, u32>,
    pub capabilities: HashMap<ContentId, u32>,
}
impl DefinitionCatalog {
    pub fn new<'a>(definitions: impl Iterator<Item = &'a ContentDefinition>) -> Self {
        let defs: Vec<_> = definitions.collect();
        let compile = |items: BTreeSet<ContentId>| {
            items
                .into_iter()
                .enumerate()
                .map(|(i, k)| (k, i as u32))
                .collect()
        };
        Self {
            tags: compile(defs.iter().flat_map(|d| d.tags.iter().cloned()).collect()),
            capabilities: compile(
                defs.iter()
                    .flat_map(|d| d.capabilities.iter().cloned())
                    .collect(),
            ),
        }
    }
    pub fn compile(
        &self,
        d: &ContentDefinition,
        handlers: &[(ContentId, NativeHandler)],
    ) -> Result<CompiledContentDefinition, RegistrationError> {
        let use_handler = d
            .handlers
            .first()
            .map(|binding| {
                handlers
                    .iter()
                    .position(|(key, _)| key == &binding.key)
                    .ok_or(RegistrationError::InvalidDefinition)
            })
            .transpose()?;
        let mut resources = d.resources.clone();
        resources.sort();
        resources.dedup();
        Ok(CompiledContentDefinition {
            key: d.key.clone(),
            metadata: d.metadata.clone(),
            properties: d.properties.clone(),
            tags: IndexedMembership::compile(&d.tags, &self.tags),
            capabilities: IndexedMembership::compile(&d.capabilities, &self.capabilities),
            resources,
            use_handler,
        })
    }
}
/// Only use is added: demonstrated by the independent reactor consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentEvent {
    Use,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerBinding {
    pub event: ContentEvent,
    pub key: ContentId,
}
pub struct HandlerContext<'a> {
    pub position: rustcraft_engine_core::BlockPos,
    pub state: rustcraft_engine_core::BlockState,
    pub definition: &'a crate::CompiledVoxelDefinition,
}
pub type NativeHandler = fn(
    &HandlerContext<'_>,
    &mut crate::CommandBuffer,
) -> Result<(), crate::state_schema::StateError>;
