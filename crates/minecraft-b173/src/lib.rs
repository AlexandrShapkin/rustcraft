//! First-party Minecraft Beta 1.7.3 game package composition.
//!
//! Existing M0-M3 content modules are re-exported during incremental migration, while this
//! package also registers through the same public Game API used by other games and native mods.

use rustcraft_game_api::{
    CollisionDescriptor, ContentId, FaceResources, GamePackage, GameProfile, GameRegistry,
    LightDescriptor, MaterialClass, RegistrationError, ScheduleStage, SystemDescriptor,
    VoxelDefinition,
};
use rustcraft_mod_api::{FaceTextures, Material};

pub use rustcraft_gameplay_blocks as blocks;
pub use rustcraft_gameplay_flat_world as flat_world;

pub const PACKAGE_ID: ContentId = ContentId::new("minecraft_b173:package/game");
pub const PROFILE_ID: ContentId = ContentId::new("minecraft_b173:profile/default");

#[derive(Debug, Default)]
pub struct MinecraftB173Package;

impl GamePackage for MinecraftB173Package {
    fn id(&self) -> ContentId {
        PACKAGE_ID
    }

    fn register(&self, registry: &mut GameRegistry) -> Result<(), RegistrationError> {
        registry.register_package(self.id())?;
        for block in blocks::BLOCKS {
            let textures = match block.textures {
                FaceTextures::All(resource) => FaceResources::All(ContentId::new(resource)),
                FaceTextures::TopSideBottom { top, side, bottom } => FaceResources::TopSideBottom {
                    top: ContentId::new(top),
                    side: ContentId::new(side),
                    bottom: ContentId::new(bottom),
                },
                FaceTextures::Faces(resources) => {
                    FaceResources::Faces(resources.map(ContentId::new))
                }
            };
            registry.register_block(VoxelDefinition {
                id: block.id,
                key: ContentId::new(block.name),
                collision: if block.solid {
                    CollisionDescriptor::FullCube
                } else {
                    CollisionDescriptor::Empty
                },
                material: match block.material {
                    Material::Invisible => MaterialClass::Invisible,
                    Material::Opaque => MaterialClass::Opaque,
                    Material::Cutout => MaterialClass::Cutout,
                    Material::Translucent => MaterialClass::Translucent,
                },
                textures,
                light: LightDescriptor {
                    emission: block.emission,
                    sky_opacity: block.sky_opacity,
                    block_opacity: block.light_opacity,
                },
                capabilities: vec![ContentId::new("voxel_std:capability/block")],
            })?;
        }
        Ok(())
    }
}

#[must_use]
pub fn profile() -> GameProfile {
    GameProfile {
        id: PROFILE_ID,
        packages: vec![ContentId::new("voxel_std:package/base"), PACKAGE_ID],
        resources: vec![ContentId::new("minecraft_b173:resources/vanilla_local")],
        systems: [
            (
                "minecraft_b173:system/inventory",
                ScheduleStage::FixedUpdate,
            ),
            ("minecraft_b173:system/mining", ScheduleStage::FixedUpdate),
            ("minecraft_b173:system/drops", ScheduleStage::FixedUpdate),
            ("minecraft_b173:system/crafting", ScheduleStage::Update),
        ]
        .map(|(id, stage)| SystemDescriptor {
            id: ContentId::new(id),
            stage,
        })
        .to_vec(),
        manifest: rustcraft_content::ContentManifest::default(),
    }
}

/// Validate first-party composition through the same public registration surface used by any
/// native game package. Legacy M0-M3 runtime wiring remains behind this boundary while it is
/// migrated incrementally.
pub fn validate_package() -> Result<(), RegistrationError> {
    let mut registry = GameRegistry::default();
    MinecraftB173Package.register(&mut registry)?;
    profile().validate()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minecraft_registers_through_the_public_game_api() {
        let mut registry = GameRegistry::default();
        MinecraftB173Package.register(&mut registry).unwrap();
        assert_eq!(registry.blocks().len(), blocks::BLOCKS.len());
        assert!(
            registry
                .blocks()
                .iter()
                .all(|block| block.key.as_str().starts_with("minecraft_b173:"))
        );
        validate_package().unwrap();
    }
}
