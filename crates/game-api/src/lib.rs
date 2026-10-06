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

pub mod definition;
pub mod geometry;
pub use geometry::*;
pub mod state_schema;
use definition::DefinitionCatalog;
pub use definition::*;
use state_schema::{CompiledStateSchema, StateError, StateRecord, StateSchema};

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
semantic_key!(ModelKey);
semantic_key!(ShapeKey);

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
#[derive(Debug, Clone, PartialEq)]
pub struct VoxelDefinition {
    pub geometry: Option<GeometryBinding>,
    pub common: ContentDefinition,
    /// None retains the historical orientation encoding unchanged.
    pub state_schema: Option<StateSchema>,
    pub collision: CollisionDescriptor,
    pub targetable: bool,
    pub material: MaterialClass,
    pub textures: FaceResources,
    pub light: LightDescriptor,
    pub base_rotation: ModelRotation,
    pub orientation: OrientationProperty,
    /// Per-face RGB tint bytes in +Z, -Z, +X, -X, +Y, -Y order.
    pub face_tints: [[u16; 3]; 6],
}

/// Runtime definition indexed directly by its dense profile-local handle.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledVoxelDefinition {
    pub geometry: Option<CompiledGeometryBinding>,
    pub common: CompiledContentDefinition,
    pub state_schema: CompiledStateSchema,
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
    items: Vec<ItemDefinition>,
}

#[derive(Debug, Default, Clone)]
pub struct GameRegistry {
    models: Vec<(PackageId, ModelDefinition)>,
    shapes: Vec<(PackageId, ShapeDefinition)>,
    packages: Vec<AuthoredPackage>,
    active_package: Option<usize>,
    block_keys: HashSet<BlockKey>,
    item_keys: HashSet<ContentId>,
    handlers: Vec<(ContentId, NativeHandler)>,
}

impl GameRegistry {
    pub fn register_model(&mut self, definition: ModelDefinition) -> Result<(), RegistrationError> {
        if !definition.geometry.valid() || self.models.iter().any(|(_, d)| d.key == definition.key)
        {
            return Err(RegistrationError::InvalidDefinition);
        }
        let package = self
            .active_package
            .ok_or(RegistrationError::NoActivePackage)?;
        self.models
            .push((self.packages[package].id.clone(), definition));
        Ok(())
    }
    pub fn register_shape(&mut self, definition: ShapeDefinition) -> Result<(), RegistrationError> {
        if !definition.shape.valid(false)
            || self.shapes.iter().any(|(_, d)| d.key == definition.key)
        {
            return Err(RegistrationError::InvalidDefinition);
        }
        let package = self
            .active_package
            .ok_or(RegistrationError::NoActivePackage)?;
        self.shapes
            .push((self.packages[package].id.clone(), definition));
        Ok(())
    }

    pub fn register_package(&mut self, package: PackageId) -> Result<(), RegistrationError> {
        if self.packages.iter().any(|entry| entry.id == package) {
            return Err(RegistrationError::DuplicatePackage(package));
        }
        self.packages.push(AuthoredPackage {
            id: package,
            blocks: Vec::new(),
            items: Vec::new(),
        });
        self.active_package = Some(self.packages.len() - 1);
        Ok(())
    }

    pub fn register_block(&mut self, definition: VoxelDefinition) -> Result<(), RegistrationError> {
        definition.common.validate()?;
        if let Some(schema) = &definition.state_schema {
            schema.compile()?;
        }
        if definition.state_schema.is_some() && definition.orientation != OrientationProperty::None
        {
            return Err(RegistrationError::InvalidDefinition);
        }
        if definition.light.emission > 15
            || definition.light.sky_opacity > 15
            || definition.light.block_opacity > 15
        {
            return Err(RegistrationError::InvalidDefinition);
        }
        let Some(package_index) = self.active_package else {
            return Err(RegistrationError::NoActivePackage);
        };
        if self.item_keys.contains(&definition.common.key) {
            return Err(RegistrationError::DuplicateContentId(
                definition.common.key.clone(),
            ));
        }
        if !self
            .block_keys
            .insert(BlockKey::from(definition.common.key.clone()))
        {
            return Err(RegistrationError::DuplicateContentId(
                definition.common.key.clone(),
            ));
        }
        let package = &mut self.packages[package_index];
        package.blocks.push(definition);
        Ok(())
    }

    pub fn register_item(&mut self, definition: ItemDefinition) -> Result<(), RegistrationError> {
        definition.common.validate()?;
        if !definition.common.handlers.is_empty()
            || self
                .block_keys
                .contains(&BlockKey::from(definition.common.key.clone()))
            || definition.max_stack == 0
        {
            return Err(RegistrationError::InvalidDefinition);
        }
        let package = self
            .active_package
            .ok_or(RegistrationError::NoActivePackage)?;
        if !self.item_keys.insert(definition.common.key.clone()) {
            return Err(RegistrationError::DuplicateContentId(definition.common.key));
        }
        self.packages[package].items.push(definition);
        Ok(())
    }
    pub fn register_handler(
        &mut self,
        key: ContentId,
        handler: NativeHandler,
    ) -> Result<(), RegistrationError> {
        if self.handlers.iter().any(|(k, _)| k == &key) {
            return Err(RegistrationError::DuplicateContentId(key));
        }
        self.handlers.push((key, handler));
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
            definitions.sort_by(|left, right| left.common.key.cmp(&right.common.key));
            ordered.extend(definitions);
        }
        let Some(default_index) = ordered
            .iter()
            .position(|definition| definition.common.key == *profile.default_block.as_id())
        else {
            return Err(RegistrationError::MissingDefaultBlock(
                profile.default_block.clone(),
            ));
        };
        let default = ordered.remove(default_index);
        ordered.insert(0, default);

        let mut ordered_items = Vec::new();
        for package_id in &profile.packages {
            let package = self.packages.iter().find(|p| &p.id == package_id).unwrap();
            let mut items: Vec<_> = package.items.iter().collect();
            items.sort_by(|a, b| a.common.key.cmp(&b.common.key));
            ordered_items.extend(items);
        }
        let mut handlers = self.handlers.clone();
        handlers.sort_by(|a, b| a.0.cmp(&b.0));
        let catalog = DefinitionCatalog::new(
            ordered
                .iter()
                .map(|d| &d.common)
                .chain(ordered_items.iter().map(|d| &d.common)),
        );
        let mut items = Vec::new();
        let mut item_by_key = HashMap::new();
        for (slot, item) in ordered_items.into_iter().enumerate() {
            let id = ItemDefinitionId(
                u32::try_from(slot).map_err(|_| RegistrationError::InvalidDefinition)?,
            );
            item_by_key.insert(item.common.key.clone(), id);
            items.push(CompiledItemDefinition {
                id,
                common: catalog.compile(&item.common, &handlers)?,
                max_stack: item.max_stack,
                icon: item.icon.clone(),
            });
        }
        let geometry = std::sync::Arc::new(GeometryCatalog::compile(
            &self
                .models
                .iter()
                .filter(|(p, _)| profile.packages.contains(p))
                .map(|(_, d)| d.clone())
                .collect::<Vec<_>>(),
            &self
                .shapes
                .iter()
                .filter(|(p, _)| profile.packages.contains(p))
                .map(|(_, d)| d.clone())
                .collect::<Vec<_>>(),
        )?);
        let mut blocks = Vec::with_capacity(ordered.len());
        let mut by_key = HashMap::with_capacity(ordered.len());
        for (index, authored) in ordered.into_iter().enumerate() {
            let numeric = u32::try_from(index).map_err(|_| RegistrationError::TooManyBlocks)?;
            let id = BlockId(numeric);
            by_key.insert(BlockKey::from(authored.common.key.clone()), id);
            let state_schema = authored
                .state_schema
                .as_ref()
                .map(StateSchema::compile)
                .transpose()?
                .unwrap_or_else(|| CompiledStateSchema::legacy(authored.orientation));
            let binding = authored
                .geometry
                .as_ref()
                .map(|b| geometry.binding(b, &state_schema))
                .transpose()?;
            blocks.push(CompiledVoxelDefinition {
                geometry: binding,
                common: catalog.compile(&authored.common, &handlers)?,
                state_schema,
                id,
                key: BlockKey::from(authored.common.key.clone()),
                collision: authored.collision,
                targetable: authored.targetable,
                material: authored.material,
                textures: authored.textures.clone(),
                light: authored.light,
                base_rotation: authored.base_rotation,
                orientation: authored.orientation,
                face_tints: authored.face_tints,
            });
        }
        Ok(CompiledGameProfile {
            id: profile.id.clone(),
            packages: profile.packages.clone(),
            resources: profile.resources.clone(),
            systems: profile.systems.clone(),
            manifest_digest: profile.manifest.canonical_digest(),
            geometry,
            default_state: BlockState::new(BlockId(0)),
            blocks,
            by_key,
            items,
            item_by_key,
            catalog,
            handlers,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CompiledGameProfile {
    geometry: std::sync::Arc<GeometryCatalog>,
    pub id: ContentId,
    pub packages: Vec<PackageId>,
    pub resources: Vec<ResourceId>,
    pub systems: Vec<SystemDescriptor>,
    pub manifest_digest: ContentHash,
    default_state: BlockState,
    blocks: Vec<CompiledVoxelDefinition>,
    by_key: HashMap<BlockKey, BlockId>,
    items: Vec<CompiledItemDefinition>,
    item_by_key: HashMap<ContentId, ItemDefinitionId>,
    catalog: DefinitionCatalog,
    handlers: Vec<(ContentId, NativeHandler)>,
}

impl CompiledGameProfile {
    pub fn geometry(&self) -> &std::sync::Arc<GeometryCatalog> {
        &self.geometry
    }
    pub fn geometry_for_state(
        &self,
        state: BlockState,
    ) -> Result<Option<ResolvedGeometry>, StateError> {
        let block = self.block(state.block).ok_or(StateError::InvalidVariant)?;
        block.state_schema.validate(state.variant)?;
        block
            .geometry
            .as_ref()
            .map(|b| {
                b.resolve(state.variant).map(|mut g| {
                    g.transform = block.base_rotation.compose(g.transform);
                    g
                })
            })
            .transpose()
    }
    pub fn light_for_state(&self, state: BlockState) -> Result<LightDescriptor, StateError> {
        let mut light = self
            .block(state.block)
            .ok_or(StateError::InvalidVariant)?
            .light;
        if let Some(g) = self.geometry_for_state(state)? {
            let cap = match g.light {
                LightCoverage::Full => 15,
                LightCoverage::Partial => 8,
                LightCoverage::None => 0,
                LightCoverage::Transmitting => 1,
            };
            light.sky_opacity = light.sky_opacity.min(cap);
            light.block_opacity = light.block_opacity.min(cap);
        }
        Ok(light)
    }
    pub fn overlaps_state(
        &self,
        state: BlockState,
        bounds: rustcraft_engine_core::shape::LocalBox,
    ) -> Result<bool, StateError> {
        if let Some(g) = self.geometry_for_state(state)? {
            Ok(self.geometry.shapes[g.collision.0 as usize]
                .shape
                .overlaps(bounds, g.transform))
        } else {
            Ok(self
                .block(state.block)
                .ok_or(StateError::InvalidVariant)?
                .collision
                == CollisionDescriptor::FullCube
                && rustcraft_engine_core::shape::box_overlap(
                    bounds,
                    rustcraft_engine_core::shape::LocalBox::UNIT,
                ))
        }
    }
    pub fn selection_hit(
        &self,
        state: BlockState,
        origin: [f32; 3],
        direction: [f32; 3],
        reach: f32,
    ) -> Result<Option<rustcraft_engine_core::shape::ShapeHit>, StateError> {
        let block = self.block(state.block).ok_or(StateError::InvalidVariant)?;
        let geometry = self.geometry_for_state(state)?;
        if !block.targetable || block.material == MaterialClass::Invisible {
            return Ok(None);
        }
        Ok(if let Some(g) = geometry {
            self.geometry.shapes[g.selection.0 as usize].shape.ray(
                origin,
                direction,
                g.transform,
                reach,
            )
        } else {
            rustcraft_engine_core::shape::Shape::FullCube.ray(
                origin,
                direction,
                ModelRotation::IDENTITY,
                reach,
            )
        })
    }

    pub fn serialize_state(
        &self,
        state: BlockState,
    ) -> Result<state_schema::SemanticVoxelState, StateError> {
        let (block, state) = self.export_state(state)?;
        Ok(state_schema::SemanticVoxelState {
            block: block.as_str().into(),
            state,
        })
    }
    pub fn deserialize_state(
        &self,
        record: &state_schema::SemanticVoxelState,
    ) -> Result<BlockState, StateError> {
        let key = BlockKey::parse(&record.block).map_err(|_| StateError::UnknownField)?;
        self.import_state(&key, &record.state)
    }
    pub fn tag_index(&self, key: &ContentId) -> Option<TagIndex> {
        self.catalog.tags.get(key).copied().map(TagIndex)
    }
    pub fn capability_index(&self, key: &ContentId) -> Option<CapabilityIndex> {
        self.catalog
            .capabilities
            .get(key)
            .copied()
            .map(CapabilityIndex)
    }
    pub fn item_id(&self, key: &ContentId) -> Option<ItemDefinitionId> {
        self.item_by_key.get(key).copied()
    }
    pub fn item(&self, id: ItemDefinitionId) -> Option<&CompiledItemDefinition> {
        self.items.get(id.0 as usize)
    }
    pub fn items(&self) -> &[CompiledItemDefinition] {
        &self.items
    }
    /// Semantic serialization at the boundary, not a new world-save envelope.
    pub fn export_state(&self, state: BlockState) -> Result<(BlockKey, StateRecord), StateError> {
        let definition = self.block(state.block).ok_or(StateError::UnknownField)?;
        Ok((
            definition.key.clone(),
            definition.state_schema.export(state.variant)?,
        ))
    }
    pub fn import_state(
        &self,
        key: &BlockKey,
        record: &StateRecord,
    ) -> Result<BlockState, StateError> {
        let block = self.block_id(key).ok_or(StateError::UnknownField)?;
        Ok(BlockState {
            block,
            variant: self.block(block).unwrap().state_schema.import(record)?,
        })
    }
    /// Native semantic event dispatch has no mutable World. Validate effects before admission.
    pub fn dispatch(
        &self,
        event: ContentEvent,
        position: BlockPos,
        state: BlockState,
    ) -> Result<CommandBuffer, StateError> {
        let definition = self.block(state.block).ok_or(StateError::UnknownField)?;
        definition.state_schema.validate(state.variant)?;
        let mut output = CommandBuffer::default();
        match event {
            ContentEvent::Use => {
                if let Some(index) = definition.common.use_handler {
                    (self.handlers[index].1)(
                        &HandlerContext {
                            position,
                            state,
                            definition,
                        },
                        &mut output,
                    )?;
                }
            }
        }
        if output.len() > 64 {
            return Err(StateError::InvalidValue);
        }
        for command in &output.commands {
            match command {
                WorldCommand::SetBlock {
                    position: target,
                    state,
                } => {
                    if *target != position {
                        return Err(StateError::InvalidValue);
                    }
                    self.block(state.block)
                        .ok_or(StateError::UnknownField)?
                        .state_schema
                        .validate(state.variant)?;
                }
            }
        }
        Ok(output)
    }
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
            if !block.state_schema.is_legacy() {
                block.state_schema.hash_contract(&mut hasher);
            }
            if let Some(binding) = &block.geometry {
                self.geometry.hash_binding(binding, &mut hasher);
            }
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

    pub fn drain(&mut self) -> impl Iterator<Item = WorldCommand> + '_ {
        self.commands.drain(..)
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

    pub(super) fn definition(value: &str) -> VoxelDefinition {
        VoxelDefinition {
            geometry: None,
            common: ContentDefinition::new(block(value).as_id().clone())
                .with_capabilities(Vec::new()),
            state_schema: None,
            collision: CollisionDescriptor::FullCube,
            targetable: true,
            material: MaterialClass::Opaque,
            textures: FaceResources::All(texture("test:textures/debug")),
            light: LightDescriptor::default(),
            base_rotation: ModelRotation::IDENTITY,
            orientation: OrientationProperty::None,
            face_tints: [[u16::MAX; 3]; 6],
        }
    }

    pub(super) fn profile(packages: Vec<PackageId>, default: &str) -> GameProfile {
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
                geometry: None,
                common: ContentDefinition::new(key.as_id().clone()),
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

/// Bounded native work/reward rule. Games choose costs, multipliers and rewards;
/// the engine only advances progress and executes admitted mutations.
#[derive(Debug, Clone)]
pub struct WorkDefinition {
    pub target: BlockKey,
    pub duration_seconds: f32,
    pub item_multipliers: Vec<(ResourceId, f32)>,
    pub reward: Option<(ResourceId, u16)>,
}
#[derive(Debug, Clone)]
pub struct RecipeDefinition {
    pub key: ResourceId,
    /// Explicit retained local recipe naming; not external identity.
    pub local_alias: Option<&'static str>,
    pub shaped: bool,
    pub width: u8,
    pub inputs: Vec<Option<ResourceId>>,
    pub output: ResourceId,
    pub count: u16,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkProgress(pub f32);
impl WorkProgress {
    pub fn advance(&mut self, dt: f32, duration_seconds: f32, multiplier: f32) -> bool {
        if !dt.is_finite()
            || dt < 0.
            || !duration_seconds.is_finite()
            || duration_seconds <= 0.
            || !multiplier.is_finite()
            || multiplier <= 0.
        {
            return false;
        }
        self.0 += dt * multiplier / duration_seconds;
        self.0 >= 1.
    }
}

/// Container matching mechanism; games supply the authored recipe, layout and resulting effect.
pub fn recipe_matches<T: Clone + Ord>(
    shaped: bool,
    grid: &[Option<T>],
    inputs: &[Option<T>],
) -> bool {
    if shaped {
        return grid == inputs;
    }
    let mut actual: Vec<_> = grid.iter().flatten().cloned().collect();
    let mut wanted: Vec<_> = inputs.iter().flatten().cloned().collect();
    actual.sort();
    wanted.sort();
    actual == wanted
}

impl CompiledVoxelDefinition {
    /// Legacy fallback descriptor. General bindings are resolved by `CompiledGameProfile::overlaps_state`.
    pub fn collision_for_state(&self, variant: u16) -> Result<CollisionDescriptor, StateError> {
        self.state_schema.validate(variant)?;
        Ok(self.collision)
    }
}
#[cfg(test)]
mod c2_tests {
    use super::*;
    use state_schema::*;
    fn id(s: &str) -> ContentId {
        ContentId::parse(s).unwrap()
    }
    fn effect(ctx: &HandlerContext<'_>, out: &mut CommandBuffer) -> Result<(), StateError> {
        out.set_block(
            BlockPos {
                x: ctx.position.x + 1,
                ..ctx.position
            },
            ctx.state,
        );
        Ok(())
    }
    #[test]
    fn typed_properties_and_handler_admission_reject_invalid_authored_and_emitted_data() {
        for value in [f32::NAN, f32::INFINITY, -1.] {
            let mut d = tests::definition("test:block/empty");
            d.common.properties.mass_kg = Some(value);
            let mut r = GameRegistry::default();
            r.register_package(PackageId::parse("test:package/main").unwrap())
                .unwrap();
            assert_eq!(
                r.register_block(d),
                Err(RegistrationError::InvalidDefinition)
            );
            assert_eq!(r.authored_block_count(), 0);
        }
        let mut d = tests::definition("test:block/empty");
        d.common.capabilities = vec![id("voxel_std:capability/interactable")];
        d.common.handlers = vec![HandlerBinding {
            event: ContentEvent::Use,
            key: id("test:handler/use"),
        }];
        let mut r = GameRegistry::default();
        let package = PackageId::parse("test:package/main").unwrap();
        r.register_package(package.clone()).unwrap();
        r.register_block(d).unwrap();
        let profile = tests::profile(vec![package], "test:block/empty");
        assert!(r.compile(&profile).is_err(), "unknown handler rejected");
        r.register_handler(id("test:handler/use"), effect).unwrap();
        let compiled = r.compile(&profile).unwrap();
        assert!(
            compiled
                .dispatch(
                    ContentEvent::Use,
                    BlockPos { x: 0, y: 0, z: 0 },
                    compiled.default_state()
                )
                .is_err(),
            "out-of-context effects rejected before application"
        );
    }
    #[test]
    fn semantic_state_json_and_indexes_survive_profile_package_reordering() {
        let mut r = GameRegistry::default();
        let packages = [
            PackageId::parse("a:package/main").unwrap(),
            PackageId::parse("b:package/main").unwrap(),
        ];
        for (slot, p) in packages.iter().enumerate() {
            r.register_package(p.clone()).unwrap();
            let mut d = tests::definition(if slot == 0 {
                "a:block/value"
            } else {
                "b:block/value"
            });
            d.state_schema = Some(StateSchema {
                fields: vec![StateField {
                    key: id("test:state/powered"),
                    domain: StateDomain::Powered,
                    default: StateValue::Powered(false),
                }],
                ..Default::default()
            });
            d.common.tags = vec![id("test:tag/diagnostic")];
            d.common.capabilities = vec![id("test:capability/sample")];
            r.register_block(d).unwrap();
        }
        r.register_block(tests::definition("b:block/empty"))
            .unwrap();
        let forward = r
            .compile(&tests::profile(packages.to_vec(), "b:block/empty"))
            .unwrap();
        let reverse = r
            .compile(&tests::profile(
                packages.into_iter().rev().collect(),
                "b:block/empty",
            ))
            .unwrap();
        let key = BlockKey::parse("a:block/value").unwrap();
        let a = forward.block_id(&key).unwrap();
        let b = reverse.block_id(&key).unwrap();
        assert_ne!(a, b);
        let state = BlockState {
            block: a,
            variant: 1,
        };
        let (semantic, record) = forward.export_state(state).unwrap();
        let semantic_record = forward.serialize_state(state).unwrap();
        let json = serde_json::to_string(&semantic_record).unwrap();
        assert!(json.contains("a:block/value"));
        let whole: state_schema::SemanticVoxelState = serde_json::from_str(&json).unwrap();
        assert_eq!(
            reverse.deserialize_state(&whole).unwrap(),
            BlockState {
                block: b,
                variant: 1
            }
        );
        let json = serde_json::to_string(&record).unwrap();
        let decoded: StateRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(
            reverse.import_state(&semantic, &decoded).unwrap(),
            BlockState {
                block: b,
                variant: 1
            }
        );
        let tag = reverse.tag_index(&id("test:tag/diagnostic")).unwrap();
        assert!(reverse.block(b).unwrap().common.has_tag(tag));
        assert_eq!(forward.tag_index(&id("test:tag/diagnostic")), Some(tag));
        assert_eq!(
            forward.semantic_fingerprint(),
            reverse.semantic_fingerprint()
        );
        assert!(reverse.block(b).unwrap().collision_for_state(2).is_err());
        let mut changed = r.clone();
        changed.packages[0].blocks[0]
            .state_schema
            .as_mut()
            .unwrap()
            .version = 2;
        assert_ne!(
            changed
                .compile(&tests::profile(
                    vec![
                        PackageId::parse("a:package/main").unwrap(),
                        PackageId::parse("b:package/main").unwrap()
                    ],
                    "b:block/empty"
                ))
                .unwrap()
                .semantic_fingerprint(),
            forward.semantic_fingerprint()
        );
    }
}
