//! Public native gameplay-extension mechanisms.
//!
//! Authored content uses stable semantic IDs. Profile compilation assigns compact process-local
//! handles for hot paths. This crate intentionally has no Minecraft policy.

use rustcraft_content::{
    ContentHash, ContentManifest, InvalidNamespacedId, NamespacedId, PackageId, ResourceId,
};
use rustcraft_engine_core::{
    BlockId, BlockPos, BlockState, World,
    orientation::{ModelRotation, OrientationProperty},
};
use std::collections::{HashMap, HashSet};

pub type ContentId = NamespacedId;

macro_rules! semantic_key {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(NamespacedId);

        impl $name {
            pub fn parse(value: impl AsRef<str>) -> Result<Self, InvalidNamespacedId> {
                NamespacedId::parse(value).map(Self)
            }

            #[must_use]
            pub fn as_id(&self) -> &NamespacedId {
                &self.0
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl From<NamespacedId> for $name {
            fn from(value: NamespacedId) -> Self {
                Self(value)
            }
        }
    };
}

semantic_key!(BlockKey);
semantic_key!(TextureKey);

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
    /// Static, alpha-blended liquid geometry with empty collision.
    Liquid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceResources {
    All(TextureKey),
    TopSideBottom {
        top: TextureKey,
        side: TextureKey,
        bottom: TextureKey,
    },
    Faces([TextureKey; 6]),
}

impl FaceResources {
    /// Face order is +Z, -Z, +X, -X, +Y, -Y.
    #[must_use]
    pub fn face(&self, index: usize) -> &TextureKey {
        match self {
            Self::All(resource) => resource,
            Self::TopSideBottom { top, side, bottom } => match index {
                4 => top,
                5 => bottom,
                _ => side,
            },
            Self::Faces(resources) => &resources[index],
        }
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

/// Package-authored definition. It deliberately contains no runtime `BlockId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelDefinition {
    pub key: BlockKey,
    pub collision: CollisionDescriptor,
    pub targetable: bool,
    pub material: MaterialClass,
    pub textures: FaceResources,
    pub light: LightDescriptor,
    pub base_rotation: ModelRotation,
    pub orientation: OrientationProperty,
    /// Per-face RGB tint bytes in +Z, -Z, +X, -X, +Y, -Y order.
    pub face_tints: [[u16; 3]; 6],
    pub capabilities: Vec<ContentId>,
}

/// Runtime definition indexed directly by its dense profile-local handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledVoxelDefinition {
    pub id: BlockId,
    pub key: BlockKey,
    pub collision: CollisionDescriptor,
    pub targetable: bool,
    pub material: MaterialClass,
    pub textures: FaceResources,
    pub light: LightDescriptor,
    pub base_rotation: ModelRotation,
    pub orientation: OrientationProperty,
    pub face_tints: [[u16; 3]; 6],
    pub capabilities: Vec<ContentId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    InvalidDefinition,
    DuplicateContentId(NamespacedId),
    DuplicatePackage(PackageId),
    DuplicateResource(ResourceId),
    DuplicateSystem(NamespacedId),
    PackageNotRegistered(PackageId),
    NoActivePackage,
    MissingDefaultBlock(BlockKey),
    TooManyBlocks,
}

#[derive(Debug, Clone)]
struct AuthoredPackage {
    id: PackageId,
    blocks: Vec<VoxelDefinition>,
}

#[derive(Debug, Default, Clone)]
pub struct GameRegistry {
    packages: Vec<AuthoredPackage>,
    active_package: Option<usize>,
    block_keys: HashSet<BlockKey>,
}

impl GameRegistry {
    pub fn register_package(&mut self, package: PackageId) -> Result<(), RegistrationError> {
        if self.packages.iter().any(|entry| entry.id == package) {
            return Err(RegistrationError::DuplicatePackage(package));
        }
        self.packages.push(AuthoredPackage {
            id: package,
            blocks: Vec::new(),
        });
        self.active_package = Some(self.packages.len() - 1);
        Ok(())
    }

    pub fn register_block(&mut self, definition: VoxelDefinition) -> Result<(), RegistrationError> {
        if definition.light.emission > 15
            || definition.light.sky_opacity > 15
            || definition.light.block_opacity > 15
        {
            return Err(RegistrationError::InvalidDefinition);
        }
        let Some(package_index) = self.active_package else {
            return Err(RegistrationError::NoActivePackage);
        };
        if !self.block_keys.insert(definition.key.clone()) {
            return Err(RegistrationError::DuplicateContentId(
                definition.key.as_id().clone(),
            ));
        }
        let package = &mut self.packages[package_index];
        package.blocks.push(definition);
        Ok(())
    }

    #[must_use]
    pub fn authored_block_count(&self) -> usize {
        self.packages
            .iter()
            .map(|package| package.blocks.len())
            .sum()
    }

    pub fn packages(&self) -> impl Iterator<Item = &PackageId> {
        self.packages.iter().map(|package| &package.id)
    }

    /// Compile definitions in explicit profile package order and lexical block-key order within
    /// each package. Registration/filesystem enumeration order cannot affect runtime handles.
    /// The profile default voxel is assigned first, so its numeric value is an implementation
    /// detail that callers obtain from `default_state()` rather than assuming.
    pub fn compile(&self, profile: &GameProfile) -> Result<CompiledGameProfile, RegistrationError> {
        profile.validate()?;
        let mut ordered = Vec::new();
        for package_id in &profile.packages {
            let Some(package) = self
                .packages
                .iter()
                .find(|package| &package.id == package_id)
            else {
                return Err(RegistrationError::PackageNotRegistered(package_id.clone()));
            };
            let mut definitions = package.blocks.iter().collect::<Vec<_>>();
            definitions.sort_by(|left, right| left.key.cmp(&right.key));
            ordered.extend(definitions);
        }
        let Some(default_index) = ordered
            .iter()
            .position(|definition| definition.key == profile.default_block)
        else {
            return Err(RegistrationError::MissingDefaultBlock(
                profile.default_block.clone(),
            ));
        };
        let default = ordered.remove(default_index);
        ordered.insert(0, default);

        let mut blocks = Vec::with_capacity(ordered.len());
        let mut by_key = HashMap::with_capacity(ordered.len());
        for (index, authored) in ordered.into_iter().enumerate() {
            let numeric = u32::try_from(index).map_err(|_| RegistrationError::TooManyBlocks)?;
            let id = BlockId(numeric);
            by_key.insert(authored.key.clone(), id);
            blocks.push(CompiledVoxelDefinition {
                id,
                key: authored.key.clone(),
                collision: authored.collision,
                targetable: authored.targetable,
                material: authored.material,
                textures: authored.textures.clone(),
                light: authored.light,
                base_rotation: authored.base_rotation,
                orientation: authored.orientation,
                face_tints: authored.face_tints,
                capabilities: authored.capabilities.clone(),
            });
        }
        Ok(CompiledGameProfile {
            id: profile.id.clone(),
            packages: profile.packages.clone(),
            resources: profile.resources.clone(),
            systems: profile.systems.clone(),
            manifest_digest: profile.manifest.canonical_digest(),
            default_state: BlockState::new(BlockId(0)),
            blocks,
            by_key,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CompiledGameProfile {
    pub id: ContentId,
    pub packages: Vec<PackageId>,
    pub resources: Vec<ResourceId>,
    pub systems: Vec<SystemDescriptor>,
    pub manifest_digest: ContentHash,
    default_state: BlockState,
    blocks: Vec<CompiledVoxelDefinition>,
    by_key: HashMap<BlockKey, BlockId>,
}

impl CompiledGameProfile {
    #[must_use]
    pub const fn default_state(&self) -> BlockState {
        self.default_state
    }

    #[must_use]
    pub fn block(&self, id: BlockId) -> Option<&CompiledVoxelDefinition> {
        self.blocks.get(id.0 as usize)
    }

    #[must_use]
    pub fn block_id(&self, key: &BlockKey) -> Option<BlockId> {
        self.by_key.get(key).copied()
    }

    #[must_use]
    pub fn blocks(&self) -> &[CompiledVoxelDefinition] {
        &self.blocks
    }

    /// Stable semantic compatibility identity, intentionally independent of runtime handles,
    /// package ordering, textures, and resource-only manifest changes.
    #[must_use]
    pub fn semantic_fingerprint(&self) -> ContentHash {
        let mut blocks = self.blocks.iter().collect::<Vec<_>>();
        blocks.sort_by(|left, right| left.key.cmp(&right.key));
        let mut hasher = blake3::Hasher::new_derive_key("rustcraft.game-semantic-profile.v1");
        hasher.update(self.id.as_str().as_bytes());
        hasher.update(&[0]);
        for block in blocks {
            hasher.update(block.key.as_str().as_bytes());
            hasher.update(&[
                0,
                block.collision as u8,
                u8::from(block.targetable),
                block.material as u8,
            ]);
            hasher.update(&[
                block.light.emission,
                block.light.sky_opacity,
                block.light.block_opacity,
            ]);
        }
        ContentHash::from_digest_bytes(*hasher.finalize().as_bytes())
    }
}

/// A native game and native mod use the same definition-registration contract.
pub trait GamePackage {
    fn id(&self) -> PackageId;
    fn register(&self, registry: &mut GameRegistry) -> Result<(), RegistrationError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScheduleStage {
    FixedUpdate,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemDescriptor {
    pub id: ContentId,
    pub stage: ScheduleStage,
}

struct RegisteredSystem<C> {
    descriptor: SystemDescriptor,
    run: fn(&mut C, &mut CommandBuffer),
}

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
        if self
            .systems
            .iter()
            .any(|system| system.descriptor.id == descriptor.id)
        {
            return Err(RegistrationError::DuplicateSystem(descriptor.id));
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
            .map(|system| system.descriptor.clone())
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameProfile {
    pub id: ContentId,
    pub packages: Vec<PackageId>,
    pub resources: Vec<ResourceId>,
    pub systems: Vec<SystemDescriptor>,
    pub manifest: ContentManifest,
    pub default_block: BlockKey,
}

impl GameProfile {
    pub fn validate(&self) -> Result<(), RegistrationError> {
        if self.packages.iter().collect::<HashSet<_>>().len() != self.packages.len() {
            let duplicate = first_duplicate(&self.packages).expect("duplicate package exists");
            return Err(RegistrationError::DuplicatePackage(duplicate.clone()));
        }
        if self.resources.iter().collect::<HashSet<_>>().len() != self.resources.len() {
            let duplicate = first_duplicate(&self.resources).expect("duplicate resource exists");
            return Err(RegistrationError::DuplicateResource(duplicate.clone()));
        }
        if self
            .systems
            .iter()
            .map(|system| &system.id)
            .collect::<HashSet<_>>()
            .len()
            != self.systems.len()
        {
            return Err(RegistrationError::DuplicateSystem(self.id.clone()));
        }
        Ok(())
    }
}

fn first_duplicate<T: Eq + std::hash::Hash>(values: &[T]) -> Option<&T> {
    let mut seen = HashSet::with_capacity(values.len());
    values.iter().find(|value| !seen.insert(*value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> ContentId {
        ContentId::parse(value).unwrap()
    }

    fn package(value: &str) -> PackageId {
        PackageId::parse(value).unwrap()
    }

    fn block(value: &str) -> BlockKey {
        BlockKey::parse(value).unwrap()
    }

    fn texture(value: &str) -> TextureKey {
        TextureKey::parse(value).unwrap()
    }

    fn definition(value: &str) -> VoxelDefinition {
        VoxelDefinition {
            key: block(value),
            collision: CollisionDescriptor::FullCube,
            targetable: true,
            material: MaterialClass::Opaque,
            textures: FaceResources::All(texture("test:textures/debug")),
            light: LightDescriptor::default(),
            base_rotation: ModelRotation::IDENTITY,
            orientation: OrientationProperty::None,
            face_tints: [[u16::MAX; 3]; 6],
            capabilities: Vec::new(),
        }
    }

    fn profile(packages: Vec<PackageId>, default: &str) -> GameProfile {
        GameProfile {
            id: id("test:profile/runtime"),
            packages,
            resources: Vec::new(),
            systems: Vec::new(),
            manifest: ContentManifest::default(),
            default_block: block(default),
        }
    }

    #[test]
    fn runtime_owned_id_registers_compiles_and_roundtrips() {
        let dynamic = format!("dynamic_{}:block/reactor", "mod");
        let key = BlockKey::parse(dynamic).unwrap();
        let mut registry = GameRegistry::default();
        registry
            .register_package(package("dynamic_mod:package/main"))
            .unwrap();
        registry
            .register_block(VoxelDefinition {
                key: key.clone(),
                ..definition("dynamic_mod:block/placeholder")
            })
            .unwrap();
        let compiled = registry
            .compile(&profile(
                vec![package("dynamic_mod:package/main")],
                key.as_str(),
            ))
            .unwrap();
        let handle = compiled.block_id(&key).unwrap();
        assert_eq!(compiled.block(handle).unwrap().key, key);
    }

    #[test]
    fn independent_packages_receive_distinct_dense_handles_and_duplicates_fail() {
        let mut registry = GameRegistry::default();
        registry
            .register_package(package("mod_a:package/main"))
            .unwrap();
        registry
            .register_block(definition("mod_a:block/test"))
            .unwrap();
        registry
            .register_package(package("mod_b:package/main"))
            .unwrap();
        registry
            .register_block(definition("mod_b:block/test"))
            .unwrap();
        assert!(matches!(
            registry.register_block(definition("mod_a:block/test")),
            Err(RegistrationError::DuplicateContentId(_))
        ));
        let profile = profile(
            vec![package("mod_a:package/main"), package("mod_b:package/main")],
            "mod_a:block/test",
        );
        let compiled = registry.compile(&profile).unwrap();
        let a = compiled.block_id(&block("mod_a:block/test")).unwrap();
        let b = compiled.block_id(&block("mod_b:block/test")).unwrap();
        assert_eq!(a, compiled.default_state().block);
        assert_ne!(a, b);
        assert_eq!(compiled.block(a).unwrap().key.as_str(), "mod_a:block/test");
        assert_eq!(compiled.block(b).unwrap().key.as_str(), "mod_b:block/test");
    }

    #[test]
    fn compilation_is_deterministic_and_package_order_is_explicit() {
        let mut registry = GameRegistry::default();
        registry
            .register_package(package("one:package/main"))
            .unwrap();
        registry
            .register_block(definition("one:block/empty"))
            .unwrap();
        registry
            .register_block(definition("one:block/later"))
            .unwrap();
        registry
            .register_package(package("two:package/main"))
            .unwrap();
        registry
            .register_block(definition("two:block/first"))
            .unwrap();
        let profile = profile(
            vec![package("one:package/main"), package("two:package/main")],
            "one:block/empty",
        );
        let first = registry.compile(&profile).unwrap();
        let second = registry.compile(&profile).unwrap();
        for key in ["one:block/empty", "one:block/later", "two:block/first"] {
            let key = block(key);
            assert_eq!(first.block_id(&key), second.block_id(&key));
        }
    }

    #[test]
    fn definition_registration_order_cannot_change_runtime_handles() {
        fn build(order: [&str; 3]) -> CompiledGameProfile {
            let mut registry = GameRegistry::default();
            registry
                .register_package(package("order_test:package/main"))
                .unwrap();
            for key in order {
                registry.register_block(definition(key)).unwrap();
            }
            registry
                .compile(&profile(
                    vec![package("order_test:package/main")],
                    "order_test:block/empty",
                ))
                .unwrap()
        }

        let forward = build([
            "order_test:block/empty",
            "order_test:block/alpha",
            "order_test:block/zeta",
        ]);
        let reverse = build([
            "order_test:block/zeta",
            "order_test:block/alpha",
            "order_test:block/empty",
        ]);
        for key in [
            "order_test:block/empty",
            "order_test:block/alpha",
            "order_test:block/zeta",
        ] {
            let key = block(key);
            assert_eq!(forward.block_id(&key), reverse.block_id(&key));
        }
        assert_eq!(
            forward.block_id(&block("order_test:block/alpha")),
            Some(BlockId(1))
        );
        assert_eq!(
            forward.block_id(&block("order_test:block/zeta")),
            Some(BlockId(2))
        );
    }

    #[test]
    fn semantic_fingerprint_ignores_profile_local_runtime_handle_reordering() {
        let mut registry = GameRegistry::default();
        registry
            .register_package(package("fingerprint:one"))
            .unwrap();
        registry
            .register_block(definition("fingerprint:air"))
            .unwrap();
        registry
            .register_block(definition("fingerprint:stone"))
            .unwrap();
        registry
            .register_package(package("fingerprint:two"))
            .unwrap();
        registry
            .register_block(definition("fingerprint:wood"))
            .unwrap();
        let compile = |packages| {
            registry
                .compile(&profile(packages, "fingerprint:air"))
                .unwrap()
        };
        let first = compile(vec![package("fingerprint:one"), package("fingerprint:two")]);
        let reordered = compile(vec![package("fingerprint:two"), package("fingerprint:one")]);
        assert_ne!(
            first.block_id(&block("fingerprint:stone")),
            reordered.block_id(&block("fingerprint:stone"))
        );
        assert_eq!(
            first.semantic_fingerprint(),
            reordered.semantic_fingerprint()
        );
    }

    #[test]
    fn registration_failures_are_atomic() {
        let key = "atomic:block/value";

        let mut no_package = GameRegistry::default();
        assert_eq!(
            no_package.register_block(definition(key)),
            Err(RegistrationError::NoActivePackage)
        );
        no_package
            .register_package(package("atomic:package/no_active"))
            .unwrap();
        no_package.register_block(definition(key)).unwrap();
        assert_eq!(no_package.authored_block_count(), 1);

        let mut invalid = GameRegistry::default();
        invalid
            .register_package(package("atomic:package/invalid"))
            .unwrap();
        let mut invalid_definition = definition("atomic:block/invalid_then_valid");
        invalid_definition.light.emission = 16;
        assert_eq!(
            invalid.register_block(invalid_definition),
            Err(RegistrationError::InvalidDefinition)
        );
        invalid
            .register_block(definition("atomic:block/invalid_then_valid"))
            .unwrap();
        assert_eq!(invalid.authored_block_count(), 1);

        assert!(matches!(
            invalid.register_block(definition("atomic:block/invalid_then_valid")),
            Err(RegistrationError::DuplicateContentId(_))
        ));
        assert_eq!(invalid.authored_block_count(), 1);

        let duplicate_package = package("atomic:package/invalid");
        assert_eq!(
            invalid.register_package(duplicate_package.clone()),
            Err(RegistrationError::DuplicatePackage(duplicate_package))
        );
        invalid
            .register_block(definition("atomic:block/after_duplicate_package"))
            .unwrap();
        assert_eq!(invalid.authored_block_count(), 2);
        assert_eq!(invalid.packages().count(), 1);
    }

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
    fn schedule_produces_commands_and_only_apply_mutates_world() {
        let mut schedule = Schedule::default();
        schedule
            .register(
                SystemDescriptor {
                    id: id("sample:system/pulse"),
                    stage: ScheduleStage::FixedUpdate,
                },
                test_system,
            )
            .unwrap();
        let mut context = Context { requested: true };
        let mut commands = CommandBuffer::default();
        schedule.run(ScheduleStage::FixedUpdate, &mut context, &mut commands);
        let default = BlockId(91);
        let mut world = World::new(default);
        assert_eq!(world.get(BlockPos { x: 1, y: 2, z: 3 }), default);
        assert_eq!(commands.apply(&mut world), 1);
        assert_eq!(world.get(BlockPos { x: 1, y: 2, z: 3 }), BlockId(7));
    }
}
