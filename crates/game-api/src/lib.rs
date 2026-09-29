//! Public native gameplay-extension mechanisms.
//!
//! The engine owns schedules, registries and controlled mutation. A game package owns the
//! definitions and systems it registers. This crate intentionally has no Minecraft policy.

use rustcraft_content::ContentManifest;
use rustcraft_engine_core::{BlockId, BlockPos, BlockState, World};
use std::collections::HashSet;

/// Static semantic identity used by native packages and registries.
///
/// Downloaded/serialized content can later use an owned equivalent at the content boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentId(&'static str);

impl ContentId {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        let Some((namespace, path)) = self.0.split_once(':') else {
            return false;
        };
        !namespace.is_empty()
            && !path.is_empty()
            && namespace
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
            && path.bytes().all(|c| {
                c.is_ascii_lowercase()
                    || c.is_ascii_digit()
                    || matches!(c, b'_' | b'-' | b'/' | b'.')
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionDescriptor {
    Empty,
    FullCube,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialClass {
    Invisible,
    Opaque,
    Cutout,
    Translucent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceResources {
    All(ContentId),
    TopSideBottom {
        top: ContentId,
        side: ContentId,
        bottom: ContentId,
    },
    Faces([ContentId; 6]),
}

impl FaceResources {
    /// Face order is +Z, -Z, +X, -X, +Y, -Y.
    #[must_use]
    pub fn face(self, index: usize) -> ContentId {
        match self {
            Self::All(resource) => resource,
            Self::TopSideBottom { top, side, bottom } => match index {
                4 => top,
                5 => bottom,
                _ => side,
            },
            Self::Faces(resources) => resources[index],
        }
    }

    fn all_valid(self) -> bool {
        (0..6).all(|face| self.face(face).is_valid())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightDescriptor {
    pub emission: u8,
    pub sky_opacity: u8,
    pub block_opacity: u8,
}

impl Default for LightDescriptor {
    fn default() -> Self {
        Self {
            emission: 0,
            sky_opacity: 15,
            block_opacity: 15,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelDefinition {
    pub id: BlockId,
    pub key: ContentId,
    pub collision: CollisionDescriptor,
    pub material: MaterialClass,
    pub textures: FaceResources,
    pub light: LightDescriptor,
    /// Weakly-coupled contracts understood by game systems, not renderer branches.
    pub capabilities: Vec<ContentId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationError {
    InvalidId,
    InvalidDefinition,
    DuplicateBlockId,
    DuplicateContentId,
    DuplicatePackage,
    DuplicateSystem,
}

#[derive(Debug, Default)]
pub struct GameRegistry {
    blocks: Vec<VoxelDefinition>,
    packages: Vec<ContentId>,
}

impl GameRegistry {
    pub fn register_block(&mut self, definition: VoxelDefinition) -> Result<(), RegistrationError> {
        if !definition.key.is_valid()
            || !definition.textures.all_valid()
            || definition.capabilities.iter().any(|id| !id.is_valid())
            || definition.light.emission > 15
            || definition.light.sky_opacity > 15
            || definition.light.block_opacity > 15
        {
            return Err(RegistrationError::InvalidDefinition);
        }
        if self.blocks.iter().any(|block| block.id == definition.id) {
            return Err(RegistrationError::DuplicateBlockId);
        }
        if self.blocks.iter().any(|block| block.key == definition.key) {
            return Err(RegistrationError::DuplicateContentId);
        }
        self.blocks.push(definition);
        self.blocks.sort_by_key(|block| block.id);
        Ok(())
    }

    pub fn register_package(&mut self, package: ContentId) -> Result<(), RegistrationError> {
        if !package.is_valid() {
            return Err(RegistrationError::InvalidId);
        }
        if self.packages.contains(&package) {
            return Err(RegistrationError::DuplicatePackage);
        }
        self.packages.push(package);
        Ok(())
    }

    #[must_use]
    pub fn block(&self, id: BlockId) -> Option<&VoxelDefinition> {
        self.blocks.iter().find(|block| block.id == id)
    }

    #[must_use]
    pub fn block_by_key(&self, key: ContentId) -> Option<&VoxelDefinition> {
        self.blocks.iter().find(|block| block.key == key)
    }

    #[must_use]
    pub fn blocks(&self) -> &[VoxelDefinition] {
        &self.blocks
    }

    #[must_use]
    pub fn packages(&self) -> &[ContentId] {
        &self.packages
    }
}

/// A native game and a native mod use the same definition-registration contract.
pub trait GamePackage {
    fn id(&self) -> ContentId;
    fn register(&self, registry: &mut GameRegistry) -> Result<(), RegistrationError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScheduleStage {
    FixedUpdate,
    Update,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemDescriptor {
    pub id: ContentId,
    pub stage: ScheduleStage,
}

struct RegisteredSystem<C> {
    descriptor: SystemDescriptor,
    run: fn(&mut C, &mut CommandBuffer),
}

/// Native scheduled systems are static function pointers: no per-voxel dynamic dispatch.
pub struct Schedule<C> {
    systems: Vec<RegisteredSystem<C>>,
}

impl<C> Default for Schedule<C> {
    fn default() -> Self {
        Self {
            systems: Vec::new(),
        }
    }
}

impl<C> Schedule<C> {
    pub fn register(
        &mut self,
        descriptor: SystemDescriptor,
        run: fn(&mut C, &mut CommandBuffer),
    ) -> Result<(), RegistrationError> {
        if !descriptor.id.is_valid() {
            return Err(RegistrationError::InvalidId);
        }
        if self
            .systems
            .iter()
            .any(|system| system.descriptor.id == descriptor.id)
        {
            return Err(RegistrationError::DuplicateSystem);
        }
        self.systems.push(RegisteredSystem { descriptor, run });
        Ok(())
    }

    pub fn run(&self, stage: ScheduleStage, context: &mut C, commands: &mut CommandBuffer) {
        for system in &self.systems {
            if system.descriptor.stage == stage {
                (system.run)(context, commands);
            }
        }
    }

    #[must_use]
    pub fn descriptors(&self) -> Vec<SystemDescriptor> {
        self.systems
            .iter()
            .map(|system| system.descriptor)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldCommand {
    SetBlock {
        position: BlockPos,
        state: BlockState,
    },
}

#[derive(Debug, Default)]
pub struct CommandBuffer {
    commands: Vec<WorldCommand>,
}

impl CommandBuffer {
    pub fn set_block(&mut self, position: BlockPos, state: BlockState) {
        self.commands
            .push(WorldCommand::SetBlock { position, state });
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn apply(&mut self, world: &mut World) -> usize {
        let count = self.commands.len();
        for command in self.commands.drain(..) {
            match command {
                WorldCommand::SetBlock { position, state } => world.set_state(position, state),
            }
        }
        count
    }
}

/// Composition metadata. Loading/resolution stays in the content subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameProfile {
    pub id: ContentId,
    pub packages: Vec<ContentId>,
    pub resources: Vec<ContentId>,
    pub systems: Vec<SystemDescriptor>,
    pub manifest: ContentManifest,
}

impl GameProfile {
    pub fn validate(&self) -> Result<(), RegistrationError> {
        if !self.id.is_valid()
            || self.packages.iter().any(|id| !id.is_valid())
            || self.resources.iter().any(|id| !id.is_valid())
            || self.systems.iter().any(|system| !system.id.is_valid())
        {
            return Err(RegistrationError::InvalidId);
        }
        let unique =
            |ids: &[ContentId]| ids.iter().copied().collect::<HashSet<_>>().len() == ids.len();
        if !unique(&self.packages) || !unique(&self.resources) {
            return Err(RegistrationError::DuplicateContentId);
        }
        if self
            .systems
            .iter()
            .map(|system| system.id)
            .collect::<HashSet<_>>()
            .len()
            != self.systems.len()
        {
            return Err(RegistrationError::DuplicateSystem);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Context {
        requested: bool,
    }

    fn test_system(context: &mut Context, commands: &mut CommandBuffer) {
        if context.requested {
            commands.set_block(BlockPos { x: 1, y: 2, z: 3 }, BlockState::new(BlockId(7)));
        }
    }

    #[test]
    fn namespaced_ids_reject_global_or_malformed_names() {
        assert!(ContentId::new("sample:block/crystal").is_valid());
        assert!(!ContentId::new("stone").is_valid());
        assert!(!ContentId::new("Sample:block/stone").is_valid());
    }

    #[test]
    fn schedule_produces_commands_and_only_apply_mutates_world() {
        let mut schedule = Schedule::default();
        schedule
            .register(
                SystemDescriptor {
                    id: ContentId::new("sample:system/pulse"),
                    stage: ScheduleStage::FixedUpdate,
                },
                test_system,
            )
            .unwrap();
        let mut context = Context { requested: true };
        let mut commands = CommandBuffer::default();
        schedule.run(ScheduleStage::FixedUpdate, &mut context, &mut commands);
        let mut world = World::new(BlockId(0));
        assert_eq!(world.get(BlockPos { x: 1, y: 2, z: 3 }), BlockId(0));
        assert_eq!(commands.apply(&mut world), 1);
        assert_eq!(world.get(BlockPos { x: 1, y: 2, z: 3 }), BlockId(7));
    }
}
