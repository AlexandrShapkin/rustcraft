//! Minecraft-owned codecs for generic durable spatial and world-global envelopes.

use rustcraft_engine_core::{ChunkPos, EntityId, Vec3};
use rustcraft_mod_api::BlockRegistry;
use rustcraft_runtime::{inventory::ItemStack, survival::ItemEntity};
use rustcraft_world::{SpatialRecord, WorldStateComponent, WorldStateRecord};

pub const ITEM_ENTITY_TYPE: &str = "minecraft_b173:entity/item";
pub const ITEM_ENTITY_SCHEMA: u32 = 1;
pub const WORLD_CLOCK_COMPONENT: &str = "minecraft_b173:world/clock";
pub const WORLD_CLOCK_SCHEMA: u32 = 1;
const MAX_ITEM_KEY_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldCodecError {
    Invalid(&'static str),
    UnknownEntityType {
        world: String,
        column: ChunkPos,
        entity_id: EntityId,
        entity_type: String,
    },
    UnsupportedEntitySchema {
        world: String,
        column: ChunkPos,
        entity_id: EntityId,
        version: u32,
    },
    MissingItem {
        world: String,
        column: ChunkPos,
        entity_id: EntityId,
        key: String,
    },
    UnsupportedWorldComponent {
        id: String,
        version: u32,
    },
}

impl std::fmt::Display for WorldCodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => write!(f, "invalid durable game state: {message}"),
            Self::UnknownEntityType {
                world,
                column,
                entity_id,
                entity_type,
            } => write!(
                f,
                "world {world} column ({},{}) entity {entity_id} requires unavailable semantic entity type {entity_type}",
                column.x, column.z
            ),
            Self::UnsupportedEntitySchema {
                world,
                column,
                entity_id,
                version,
            } => write!(
                f,
                "world {world} column ({},{}) item entity {entity_id} uses unsupported schema v{version}",
                column.x, column.z
            ),
            Self::MissingItem {
                world,
                column,
                entity_id,
                key,
            } => write!(
                f,
                "world {world} column ({},{}) entity {entity_id} requires missing semantic item {key}",
                column.x, column.z
            ),
            Self::UnsupportedWorldComponent { id, version } => {
                write!(f, "world component {id} uses unsupported schema v{version}")
            }
        }
    }
}
impl std::error::Error for WorldCodecError {}

pub fn encode_item_entity(
    entity: &ItemEntity,
    registry: &BlockRegistry,
) -> Result<SpatialRecord, WorldCodecError> {
    let definition = registry
        .item(entity.stack.item)
        .ok_or(WorldCodecError::Invalid("unresolved runtime item handle"))?;
    validate_stack(
        entity.stack,
        definition.max_stack,
        definition.tool.map(|tool| tool.durability),
    )?;
    let key = definition.name.as_bytes();
    if key.is_empty() || key.len() > MAX_ITEM_KEY_BYTES {
        return Err(WorldCodecError::Invalid("semantic item key length"));
    }
    let numbers = [
        entity.position.x,
        entity.position.y,
        entity.position.z,
        entity.velocity.x,
        entity.velocity.y,
        entity.velocity.z,
        entity.age,
        entity.pickup_delay,
    ];
    if numbers.iter().any(|number| !number.is_finite())
        || entity.age < 0.0
        || entity.pickup_delay < 0.0
    {
        return Err(WorldCodecError::Invalid("invalid item transform/lifetime"));
    }
    let mut payload = Vec::with_capacity(2 + key.len() + 4 + numbers.len() * 4);
    payload.extend_from_slice(&(key.len() as u16).to_le_bytes());
    payload.extend_from_slice(key);
    payload.extend_from_slice(&entity.stack.count.to_le_bytes());
    payload.extend_from_slice(&entity.stack.damage.to_le_bytes());
    for number in numbers {
        payload.extend_from_slice(&number.to_le_bytes());
    }
    Ok(SpatialRecord {
        entity_id: entity.id,
        entity_revision: entity.persistence_revision.max(1),
        entity_type: ITEM_ENTITY_TYPE.into(),
        schema_version: ITEM_ENTITY_SCHEMA,
        payload,
    })
}

pub fn decode_item_entity(
    record: &SpatialRecord,
    registry: &BlockRegistry,
    world: &str,
    column: ChunkPos,
) -> Result<ItemEntity, WorldCodecError> {
    if record.entity_type != ITEM_ENTITY_TYPE {
        return Err(WorldCodecError::UnknownEntityType {
            world: world.into(),
            column,
            entity_id: record.entity_id,
            entity_type: record.entity_type.clone(),
        });
    }
    if record.schema_version != ITEM_ENTITY_SCHEMA {
        return Err(WorldCodecError::UnsupportedEntitySchema {
            world: world.into(),
            column,
            entity_id: record.entity_id,
            version: record.schema_version,
        });
    }
    if record.entity_id == EntityId::NIL || record.entity_revision == 0 {
        return Err(WorldCodecError::Invalid("zero entity identity/revision"));
    }
    let mut reader = Reader::new(&record.payload);
    let key_len = usize::from(reader.u16()?);
    if key_len == 0 || key_len > MAX_ITEM_KEY_BYTES {
        return Err(WorldCodecError::Invalid("semantic item key length"));
    }
    let key = std::str::from_utf8(reader.take(key_len)?)
        .map_err(|_| WorldCodecError::Invalid("semantic item key encoding"))?;
    let Some(definition) = registry.items().iter().find(|item| item.name == key) else {
        return Err(WorldCodecError::MissingItem {
            world: world.into(),
            column,
            entity_id: record.entity_id,
            key: key.into(),
        });
    };
    let stack = ItemStack {
        item: definition.id,
        count: reader.u16()?,
        damage: reader.u16()?,
    };
    validate_stack(
        stack,
        definition.max_stack,
        definition.tool.map(|tool| tool.durability),
    )?;
    let position = Vec3::new(reader.f32()?, reader.f32()?, reader.f32()?);
    let velocity = Vec3::new(reader.f32()?, reader.f32()?, reader.f32()?);
    let age = reader.f32()?;
    let pickup_delay = reader.f32()?;
    reader.finish()?;
    if [
        position.x,
        position.y,
        position.z,
        velocity.x,
        velocity.y,
        velocity.z,
        age,
        pickup_delay,
    ]
    .iter()
    .any(|number| !number.is_finite())
        || age < 0.0
        || pickup_delay < 0.0
    {
        return Err(WorldCodecError::Invalid("invalid item transform/lifetime"));
    }
    let entity = ItemEntity {
        id: record.entity_id,
        persistence_revision: record.entity_revision,
        stack,
        position,
        velocity,
        age,
        pickup_delay,
    };
    if entity.column() != column {
        return Err(WorldCodecError::Invalid(
            "item position does not match spatial owner",
        ));
    }
    Ok(entity)
}

pub fn encode_world_state(
    world_time: u64,
    revision: u64,
    preserved_unknown: &[WorldStateComponent],
) -> WorldStateRecord {
    let mut components = vec![WorldStateComponent {
        id: WORLD_CLOCK_COMPONENT.into(),
        schema_version: WORLD_CLOCK_SCHEMA,
        payload: world_time.to_le_bytes().to_vec(),
    }];
    components.extend(
        preserved_unknown
            .iter()
            .filter(|component| component.id != WORLD_CLOCK_COMPONENT)
            .cloned(),
    );
    components.sort_by(|a, b| a.id.cmp(&b.id));
    WorldStateRecord {
        revision: revision.max(1),
        components,
        recovered_from_checkpoint: false,
    }
}

pub fn decode_world_state(
    record: &WorldStateRecord,
) -> Result<(u64, Vec<WorldStateComponent>), WorldCodecError> {
    let mut world_time = 0;
    let mut unknown = Vec::new();
    for component in &record.components {
        if component.id == WORLD_CLOCK_COMPONENT {
            if component.schema_version != WORLD_CLOCK_SCHEMA {
                return Err(WorldCodecError::UnsupportedWorldComponent {
                    id: component.id.clone(),
                    version: component.schema_version,
                });
            }
            if component.payload.len() != 8 {
                return Err(WorldCodecError::Invalid("world clock payload length"));
            }
            world_time = u64::from_le_bytes(component.payload.as_slice().try_into().unwrap());
        } else {
            // Unknown generic components are retained byte-for-byte on the next checkpoint.
            unknown.push(component.clone());
        }
    }
    Ok((world_time, unknown))
}

fn validate_stack(
    stack: ItemStack,
    max_stack: u16,
    durability: Option<u16>,
) -> Result<(), WorldCodecError> {
    if stack.count == 0 || stack.count > max_stack {
        return Err(WorldCodecError::Invalid("stack count out of range"));
    }
    if match durability {
        Some(max) => stack.damage >= max,
        None => stack.damage != 0,
    } {
        return Err(WorldCodecError::Invalid(
            "item durability encoding out of range",
        ));
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], WorldCodecError> {
        let end = self
            .at
            .checked_add(length)
            .ok_or(WorldCodecError::Invalid("payload length overflow"))?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or(WorldCodecError::Invalid("truncated payload"))?;
        self.at = end;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16, WorldCodecError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, WorldCodecError> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn finish(self) -> Result<(), WorldCodecError> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(WorldCodecError::Invalid("trailing payload bytes"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_engine_core::{ItemId, World};
    use rustcraft_mod_api::GameplayModule;
    use rustcraft_runtime::Simulation;

    fn registry() -> BlockRegistry {
        let mut registry = BlockRegistry::default();
        crate::blocks::BlocksModule.register(&mut registry).unwrap();
        registry
    }

    #[test]
    fn dropped_item_roundtrips_every_authoritative_field_semantically() {
        let registry = registry();
        let entity = ItemEntity {
            id: EntityId::from_parts(9, 17),
            persistence_revision: 42,
            stack: ItemStack {
                item: ItemId(1),
                count: 7,
                damage: 0,
            },
            position: Vec3::new(-15.25, 70.5, 31.75),
            velocity: Vec3::new(0.125, -0.25, 0.5),
            age: 299.0,
            pickup_delay: 0.75,
        };
        let column = entity.column();
        let record = encode_item_entity(&entity, &registry).unwrap();
        let restored = decode_item_entity(&record, &registry, "test-world", column).unwrap();
        assert_eq!(restored, entity);
        assert_eq!(record.entity_type, ITEM_ENTITY_TYPE);
        assert!(
            record
                .payload
                .windows(b"minecraft_b173:stone".len())
                .any(|bytes| bytes == b"minecraft_b173:stone")
        );
    }

    #[test]
    fn item_decoder_rejects_corruption_and_missing_content_contextually() {
        let registry = registry();
        let mut sim = Simulation::new_with_entity_namespace(
            World::new(crate::blocks::AIR.id),
            registry.clone(),
            Vec3::ZERO,
            1,
        );
        sim.spawn_item(ItemId(1), 1, Vec3::new(0.5, 2.0, 0.5));
        let mut record = encode_item_entity(&sim.items[0], &registry).unwrap();
        record.payload.truncate(3);
        assert!(matches!(
            decode_item_entity(&record, &registry, "bad", ChunkPos { x: 0, z: 0 }),
            Err(WorldCodecError::Invalid("truncated payload"))
        ));
        let empty = BlockRegistry::default();
        let record = encode_item_entity(&sim.items[0], &registry).unwrap();
        let error = decode_item_entity(&record, &empty, "missing", ChunkPos { x: 0, z: 0 })
            .unwrap_err()
            .to_string();
        assert!(error.contains("missing semantic item minecraft_b173:stone"));
        assert!(error.contains("entity 00000000000000010000000000000001"));

        let mut invalid_count = record.clone();
        let key_len = u16::from_le_bytes(invalid_count.payload[..2].try_into().unwrap()) as usize;
        invalid_count.payload[2 + key_len..4 + key_len].copy_from_slice(&0u16.to_le_bytes());
        assert!(matches!(
            decode_item_entity(&invalid_count, &registry, "bad", ChunkPos { x: 0, z: 0 }),
            Err(WorldCodecError::Invalid("stack count out of range"))
        ));
        let mut invalid_number = record.clone();
        let position_offset = 2 + key_len + 4;
        invalid_number.payload[position_offset..position_offset + 4]
            .copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(matches!(
            decode_item_entity(&invalid_number, &registry, "bad", ChunkPos { x: 0, z: 0 }),
            Err(WorldCodecError::Invalid("invalid item transform/lifetime"))
        ));
        let mut unsupported = record.clone();
        unsupported.schema_version = 99;
        assert!(matches!(
            decode_item_entity(&unsupported, &registry, "bad", ChunkPos { x: 0, z: 0 }),
            Err(WorldCodecError::UnsupportedEntitySchema { version: 99, .. })
        ));
        let mut unknown = record;
        unknown.entity_type = "example:entity/unknown".into();
        assert!(matches!(
            decode_item_entity(&unknown, &registry, "bad", ChunkPos { x: 0, z: 0 }),
            Err(WorldCodecError::UnknownEntityType { .. })
        ));
    }

    #[test]
    fn clock_roundtrips_and_unknown_components_are_preserved() {
        let future = WorldStateComponent {
            id: "example:world/future".into(),
            schema_version: 7,
            payload: vec![1, 2, 3],
        };
        let record = encode_world_state(98_765, 12, std::slice::from_ref(&future));
        let (time, unknown) = decode_world_state(&record).unwrap();
        assert_eq!(time, 98_765);
        assert_eq!(unknown, vec![future]);

        let mut unsupported = record;
        unsupported
            .components
            .iter_mut()
            .find(|component| component.id == WORLD_CLOCK_COMPONENT)
            .unwrap()
            .schema_version = 2;
        assert!(matches!(
            decode_world_state(&unsupported),
            Err(WorldCodecError::UnsupportedWorldComponent { version: 2, .. })
        ));
    }
}
