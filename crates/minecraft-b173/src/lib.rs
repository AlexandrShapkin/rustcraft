//! First-party Minecraft Beta 1.7.3 game package composition.
//!
//! Authored definitions are semantic. Historical Beta numeric IDs remain only in the legacy
//! M0-M3 adapter and are resolved explicitly from semantic definitions while that runtime migrates.

use rustcraft_content::{
    PackageId, ResourceId,
    resources::{PixelRect, ResourceError, ResourcePackage, SamplerPolicy},
};
use rustcraft_game_api::{
    BlockKey, CollisionDescriptor, CompiledGameProfile, ContentId, FaceResources, GamePackage,
    GameProfile, GameRegistry, LightDescriptor, MaterialClass, RegistrationError, ScheduleStage,
    SystemDescriptor, TextureKey, VoxelDefinition,
};
use rustcraft_mod_api::{FaceTextures, Material};
use std::{collections::BTreeSet, path::Path};

pub mod blocks {
    pub use rustcraft_gameplay_blocks::*;
    pub struct BlocksModule;
    impl rustcraft_mod_api::GameplayModule for BlocksModule {
        fn id(&self) -> rustcraft_mod_api::ModuleId {
            rustcraft_mod_api::ModuleId("minecraft_b173:blocks")
        }
        fn register(
            &self,
            registry: &mut rustcraft_mod_api::BlockRegistry,
        ) -> Result<(), rustcraft_mod_api::RegistrationError> {
            rustcraft_gameplay_blocks::BlocksModule.register(registry)?;
            registry.bind_profile(
                super::compile_profile()
                    .map_err(|_| rustcraft_mod_api::RegistrationError::InvalidDefinition)?,
            )?;
            super::policy::register(registry)
        }
    }
}
pub mod policy;
pub use rustcraft_gameplay_flat_world as flat_world;
pub mod player_persistence;
pub mod world_persistence;

/// Static-water camera presentation policy consumed by the client renderer adapter.
pub const UNDERWATER_FOG_COLOR: [f32; 3] = [0.20, 0.40, 0.62];
pub const UNDERWATER_FOG_START: f32 = 2.0;
pub const UNDERWATER_FOG_END: f32 = 18.0;
pub mod worldgen;

fn id(value: &str) -> ContentId {
    ContentId::parse(value).expect("first-party semantic ID is valid")
}

fn package_id(value: &str) -> PackageId {
    PackageId::parse(value).expect("first-party package ID is valid")
}

fn resource_id(value: &str) -> ResourceId {
    ResourceId::parse(value).expect("first-party resource ID is valid")
}

fn block_key(value: &str) -> BlockKey {
    BlockKey::parse(value).expect("first-party block key is valid")
}

fn texture_key(legacy: &str) -> TextureKey {
    let (_, path) = legacy
        .split_once(':')
        .expect("legacy first-party texture is namespaced");
    TextureKey::parse(format!("minecraft_b173:textures/block/{path}"))
        .expect("adapted first-party texture key is valid")
}

pub mod resource_keys {
    use rustcraft_content::ResourceId;

    fn key(value: &str) -> ResourceId {
        ResourceId::parse(value).expect("first-party resource ID is valid")
    }

    pub fn inventory() -> ResourceId {
        key("minecraft_b173:textures/gui/inventory")
    }
    pub fn hotbar() -> ResourceId {
        key("minecraft_b173:textures/gui/hotbar")
    }
    pub fn selector() -> ResourceId {
        key("minecraft_b173:textures/gui/selector")
    }
    pub fn player_part(index: usize) -> ResourceId {
        let role = [
            "head",
            "torso",
            "arm_left",
            "arm_right",
            "leg_left",
            "leg_right",
        ][index];
        key(&format!("minecraft_b173:textures/entity/player/{role}"))
    }
    pub fn destroy_stage(stage: u8) -> ResourceId {
        key(&format!(
            "minecraft_b173:textures/effect/destroy_stage_{}",
            stage.min(9)
        ))
    }
}

/// Beta-specific virtual resource importer. The generic compiler only sees semantic textures and
/// file/crop sources; it has no knowledge of terrain tile numbers or historical paths.
pub fn legacy_resource_package(terrain_path: &Path) -> Result<ResourcePackage, ResourceError> {
    let root = terrain_path.parent().unwrap_or_else(|| Path::new("."));
    let terrain_name = terrain_path
        .file_name()
        .map(Path::new)
        .unwrap_or_else(|| Path::new("terrain.png"));
    let mut package =
        ResourcePackage::new(package_id("minecraft_b173:package/legacy_resources"), root);
    let mut seen = BTreeSet::new();
    for block in blocks::BLOCKS {
        for face in 0..6 {
            let texture = block.textures.face(face);
            if !seen.insert(texture) {
                continue;
            }
            let Some((x, y)) = blocks::atlas_tile(texture) else {
                continue;
            };
            package.add_texture(
                resource_id(texture_key(texture).as_str()),
                terrain_name,
                Some(PixelRect {
                    x: u32::from(x) * 16,
                    y: u32::from(y) * 16,
                    width: 16,
                    height: 16,
                }),
                SamplerPolicy::Nearest,
            )?;
        }
    }
    for stage in 0..10 {
        package.add_texture(
            resource_keys::destroy_stage(stage),
            terrain_name,
            Some(PixelRect {
                x: u32::from(stage) * 16,
                y: 15 * 16,
                width: 16,
                height: 16,
            }),
            SamplerPolicy::Nearest,
        )?;
    }
    let imports = [
        (
            resource_keys::inventory(),
            "gui/inventory.png",
            [0, 0, 176, 166],
        ),
        (resource_keys::hotbar(), "gui/gui.png", [0, 0, 182, 22]),
        (resource_keys::selector(), "gui/gui.png", [0, 22, 24, 22]),
        (resource_keys::player_part(0), "mob/char.png", [8, 8, 8, 8]),
        (
            resource_keys::player_part(1),
            "mob/char.png",
            [20, 20, 8, 12],
        ),
        (
            resource_keys::player_part(2),
            "mob/char.png",
            [44, 20, 4, 12],
        ),
        (
            resource_keys::player_part(3),
            "mob/char.png",
            [36, 20, 4, 12],
        ),
        (
            resource_keys::player_part(4),
            "mob/char.png",
            [4, 20, 4, 12],
        ),
        (
            resource_keys::player_part(5),
            "mob/char.png",
            [4, 20, 4, 12],
        ),
    ];
    for (id, path, [x, y, width, height]) in imports {
        package.add_texture(
            id,
            Path::new(path),
            Some(PixelRect {
                x,
                y,
                width,
                height,
            }),
            SamplerPolicy::Nearest,
        )?;
    }
    Ok(package)
}

#[derive(Debug, Default)]
pub struct MinecraftB173Package;

impl GamePackage for MinecraftB173Package {
    fn id(&self) -> PackageId {
        package_id("minecraft_b173:package/game")
    }

    fn register(&self, registry: &mut GameRegistry) -> Result<(), RegistrationError> {
        registry.register_package(self.id())?;
        for block in blocks::BLOCKS {
            let textures = match block.textures {
                FaceTextures::All(resource) => FaceResources::All(texture_key(resource)),
                FaceTextures::TopSideBottom { top, side, bottom } => FaceResources::TopSideBottom {
                    top: texture_key(top),
                    side: texture_key(side),
                    bottom: texture_key(bottom),
                },
                FaceTextures::Faces(resources) => FaceResources::Faces(resources.map(texture_key)),
            };
            registry.register_block(VoxelDefinition {
                geometry: None,
                common: rustcraft_game_api::ContentDefinition::new(
                    block_key(block.name).as_id().clone(),
                )
                .with_capabilities(vec![id("voxel_std:capability/block")]),
                state_schema: None,
                collision: if block.solid {
                    CollisionDescriptor::FullCube
                } else {
                    CollisionDescriptor::Empty
                },
                targetable: block.targetable,
                material: match block.material {
                    Material::Invisible => MaterialClass::Invisible,
                    Material::Opaque => MaterialClass::Opaque,
                    Material::Cutout => MaterialClass::Cutout,
                    Material::Translucent => MaterialClass::Translucent,
                    Material::Liquid => MaterialClass::Liquid,
                },
                textures,
                light: LightDescriptor {
                    emission: block.emission,
                    sky_opacity: block.sky_opacity,
                    block_opacity: block.light_opacity,
                },
                base_rotation: block.base_model_rotation,
                orientation: block.orientation_property,
                face_tints: std::array::from_fn(|face| {
                    blocks::tint(block.id, face == 4)
                        .map(|channel| (channel * f32::from(u16::MAX)).round() as u16)
                }),
            })?;
        }
        let stick = blocks::STICK_ITEM;
        registry.register_item(rustcraft_game_api::ItemDefinition {
            common: rustcraft_game_api::ContentDefinition::new(id(stick.name)),
            max_stack: stick.max_stack,
            icon: None,
        })?;
        Ok(())
    }
}

#[must_use]
pub fn profile() -> GameProfile {
    GameProfile {
        id: id("minecraft_b173:profile/default"),
        packages: vec![
            package_id("voxel_std:package/base"),
            package_id("minecraft_b173:package/game"),
        ],
        resources: vec![resource_id("minecraft_b173:resources/vanilla_local")],
        systems: [
            (
                "minecraft_b173:system/inventory",
                ScheduleStage::FixedUpdate,
            ),
            ("minecraft_b173:system/mining", ScheduleStage::FixedUpdate),
            ("minecraft_b173:system/drops", ScheduleStage::FixedUpdate),
            ("minecraft_b173:system/crafting", ScheduleStage::Update),
        ]
        .map(|(system, stage)| SystemDescriptor {
            id: id(system),
            stage,
        })
        .to_vec(),
        manifest: rustcraft_content::ContentManifest::default(),
        default_block: block_key("minecraft_b173:air"),
    }
}

pub fn compile_profile() -> Result<CompiledGameProfile, RegistrationError> {
    let mut registry = GameRegistry::default();
    registry.register_package(package_id("voxel_std:package/base"))?;
    MinecraftB173Package.register(&mut registry)?;
    registry.compile(&profile())
}

/// Compatibility adapter for the M0-M3 flat-world module. Semantic presence is validated against
/// the compiled profile, then translated to the historical registry expected by the legacy runtime.
pub fn flat_world_module(profile: &CompiledGameProfile) -> flat_world::FlatWorldModule {
    flat_world::FlatWorldModule {
        floor_y: 0,
        dirt_depth: 2,
        stone: legacy_block_id(profile, "minecraft_b173:stone"),
        dirt: legacy_block_id(profile, "minecraft_b173:dirt"),
        grass: legacy_block_id(profile, "minecraft_b173:grass"),
    }
}

fn legacy_block_id(profile: &CompiledGameProfile, name: &str) -> rustcraft_engine_core::BlockId {
    profile
        .block_id(&block_key(name))
        .unwrap_or_else(|| panic!("compiled minecraft profile is missing {name}"));
    blocks::BLOCKS
        .iter()
        .find(|definition| definition.name == name)
        .unwrap_or_else(|| panic!("legacy minecraft registry is missing {name}"))
        .id
}

/// Startup compatibility check while authoritative M0-M3 simulation still consumes its legacy
/// registry. New Game API definitions themselves never author numeric IDs.
pub fn validate_package() -> Result<(), RegistrationError> {
    compile_profile().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_registration_compiles_dense_runtime_handles() {
        let compiled = compile_profile().unwrap();
        assert_eq!(compiled.blocks().len(), blocks::BLOCKS.len());
        assert_eq!(compiled.default_state().block.0, 0);
        assert!(
            compiled
                .blocks()
                .iter()
                .all(|block| block.key.as_str().starts_with("minecraft_b173:"))
        );
    }

    #[test]
    fn generated_water_profile_keeps_liquid_texture_collision_and_targeting_semantics() {
        let profile = compile_profile().unwrap();
        let water_id = profile
            .block_id(&block_key("minecraft_b173:water"))
            .unwrap();
        let water = profile.block(water_id).unwrap();
        assert_eq!(water.key.as_str(), "minecraft_b173:water");
        assert_eq!(
            legacy_block_id(&profile, blocks::WATER.name),
            blocks::WATER.id
        );
        assert_eq!(water.material, MaterialClass::Liquid);
        assert_eq!(water.collision, CollisionDescriptor::Empty);
        assert!(!water.targetable);
        assert_eq!(
            water.textures.face(0).as_str(),
            "minecraft_b173:textures/block/water"
        );
        assert!(blocks::BLOCKS.iter().any(|block| block.id == water_id));
    }

    #[test]
    fn water_texture_key_imports_the_legacy_terrain_crop_semantically() {
        let package = legacy_resource_package(Path::new("/tmp/m4-reference/terrain.png")).unwrap();
        assert_eq!(
            package.id.as_str(),
            "minecraft_b173:package/legacy_resources"
        );
        let key = texture_key("minecraft_b173:water");
        let water = package
            .textures()
            .iter()
            .find(|texture| texture.id.as_str() == key.as_str())
            .expect("water semantic texture is imported");
        assert_eq!(
            water.source.path,
            Path::new("/tmp/m4-reference/terrain.png")
        );
        assert_eq!(
            water.source.crop,
            Some(PixelRect {
                x: 14 * 16,
                y: 0,
                width: 16,
                height: 16,
            })
        );
        assert_eq!(water.sampler, SamplerPolicy::Nearest);
    }

    #[test]
    fn canonical_handles_resolve_through_the_legacy_runtime_adapter() {
        let compiled = compile_profile().unwrap();
        for legacy in blocks::BLOCKS {
            let key = block_key(legacy.name);
            let compiled_id = compiled.block_id(&key).expect("semantic block is compiled");
            assert_eq!(compiled.block(compiled_id).unwrap().key, key);
            assert_eq!(legacy_block_id(&compiled, legacy.name), legacy.id);
        }
        let flat = flat_world_module(&compiled);
        assert_eq!(flat.stone, blocks::STONE.id);
        assert_eq!(flat.dirt, blocks::DIRT.id);
        assert_eq!(flat.grass, blocks::GRASS.id);
    }
}

pub mod control;

#[cfg(test)]
mod transfer_tests;
