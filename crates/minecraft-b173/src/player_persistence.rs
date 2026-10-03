//! Minecraft-owned durable player schema. Runtime item handles never enter the payload.
use rustcraft_engine_core::{ChunkPos, EntityId, Vec3};
use rustcraft_mod_api::BlockRegistry;
use rustcraft_runtime::{
    PickupReceipt, Simulation,
    inventory::{HOTBAR_SLOTS, Inventory, ItemStack},
    survival::GameMode,
};
use rustcraft_world::{PlayerComponent, PlayerRecord, WorldError};

pub const LOCAL_PLAYER_ID: &str = "local-player";
pub const PLAYER_SCHEMA_VERSION: u32 = 1;
pub const TRANSFORM_COMPONENT: &str = "minecraft_b173:player/transform";
pub const INVENTORY_COMPONENT: &str = "minecraft_b173:player/inventory";
pub const GAME_MODE_COMPONENT: &str = "minecraft_b173:player/game_mode";
pub const PICKUP_RECEIPTS_COMPONENT: &str = "minecraft_b173:player/pickup_receipts";
pub const PICKUP_RECEIPTS_SCHEMA_VERSION: u32 = 1;
pub const LEGACY_COMPONENT: &str = "rustcraft:legacy-player-payload";
const SLOT_COUNT: usize = 36;
const MAX_KEY_BYTES: usize = 256;
const MAX_PICKUP_RECEIPTS: usize = 512;

#[derive(Debug)]
pub enum PlayerCodecError {
    World(WorldError),
    Invalid(&'static str),
    MissingItem {
        player: String,
        location: String,
        key: String,
    },
}
impl std::fmt::Display for PlayerCodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::World(e) => write!(f, "{e}"),
            Self::Invalid(s) => write!(f, "invalid player record: {s}"),
            Self::MissingItem {
                player,
                location,
                key,
            } => write!(
                f,
                "player {player} {location} requires missing semantic item {key}"
            ),
        }
    }
}
impl std::error::Error for PlayerCodecError {}
impl From<WorldError> for PlayerCodecError {
    fn from(e: WorldError) -> Self {
        Self::World(e)
    }
}

#[derive(Debug, Clone)]
pub struct RestoredPlayer {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub mode: GameMode,
    pub inventory: Inventory,
    pub cursor: Option<ItemStack>,
    pub crafting: [Option<ItemStack>; 4],
    pub pickup_receipts: Vec<PickupReceipt>,
}

pub fn encode(sim: &Simulation) -> Result<PlayerRecord, PlayerCodecError> {
    encode_revision(sim, 1, &[])
}

pub fn encode_revision(
    sim: &Simulation,
    revision: u64,
    preserved_unknown: &[PlayerComponent],
) -> Result<PlayerRecord, PlayerCodecError> {
    let payload = encode_legacy_payload(sim)?;
    let mut components = vec![
        PlayerComponent {
            id: TRANSFORM_COMPONENT.into(),
            schema_version: 1,
            payload: payload[..20].to_vec(),
        },
        PlayerComponent {
            id: GAME_MODE_COMPONENT.into(),
            schema_version: 1,
            payload: payload[20..21].to_vec(),
        },
        PlayerComponent {
            id: INVENTORY_COMPONENT.into(),
            schema_version: 1,
            payload: payload[21..].to_vec(),
        },
        PlayerComponent {
            id: PICKUP_RECEIPTS_COMPONENT.into(),
            schema_version: PICKUP_RECEIPTS_SCHEMA_VERSION,
            payload: encode_pickup_receipts(sim)?,
        },
    ];
    components.extend(
        preserved_unknown
            .iter()
            .filter(|component| {
                !matches!(
                    component.id.as_str(),
                    TRANSFORM_COMPONENT
                        | INVENTORY_COMPONENT
                        | GAME_MODE_COMPONENT
                        | PICKUP_RECEIPTS_COMPONENT
                        | LEGACY_COMPONENT
                )
            })
            .cloned(),
    );
    components.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(PlayerRecord {
        player_id: LOCAL_PLAYER_ID.into(),
        revision,
        components,
        recovered_from_checkpoint: false,
    })
}

fn encode_legacy_payload(sim: &Simulation) -> Result<Vec<u8>, PlayerCodecError> {
    let mut out = Vec::with_capacity(2048);
    for n in [
        sim.player.position.x,
        sim.player.position.y,
        sim.player.position.z,
        sim.player.yaw,
        sim.player.pitch,
    ] {
        if !n.is_finite() {
            return Err(PlayerCodecError::Invalid("non-finite position/orientation"));
        }
        out.extend_from_slice(&n.to_le_bytes());
    }
    if sim.player.pitch.abs() > 1.5709
        || !supported_coordinate(sim.player.position.x)
        || !supported_coordinate(sim.player.position.y)
        || !supported_coordinate(sim.player.position.z)
    {
        return Err(PlayerCodecError::Invalid(
            "position/orientation outside supported range",
        ));
    }
    out.push(match sim.mode {
        GameMode::Development => 0,
        GameMode::Survival => 1,
    });
    out.push(sim.inventory.selected() as u8);
    out.push(SLOT_COUNT as u8);
    for (i, stack) in sim.inventory.slots().iter().copied().enumerate() {
        encode_stack(&mut out, stack, &sim.registry, &format!("slot {i}"))?;
    }
    encode_stack(&mut out, sim.inventory_cursor, &sim.registry, "cursor")?;
    out.push(4);
    for (i, stack) in sim.crafting_grid.iter().copied().enumerate() {
        encode_stack(
            &mut out,
            stack,
            &sim.registry,
            &format!("crafting slot {i}"),
        )?;
    }
    if out.len() > 32 * 1024 {
        return Err(PlayerCodecError::Invalid("payload limit"));
    }
    Ok(out)
}

pub fn decode(
    record: &PlayerRecord,
    registry: &BlockRegistry,
) -> Result<RestoredPlayer, PlayerCodecError> {
    decode_with_unknown(record, registry).map(|(state, _)| state)
}

pub fn decode_with_unknown(
    record: &PlayerRecord,
    registry: &BlockRegistry,
) -> Result<(RestoredPlayer, Vec<PlayerComponent>), PlayerCodecError> {
    if let Some(legacy) = record.components.iter().find(|c| c.id == LEGACY_COMPONENT) {
        if legacy.schema_version != PLAYER_SCHEMA_VERSION {
            return Err(PlayerCodecError::Invalid(
                "unsupported legacy player schema",
            ));
        }
        let state = decode_payload(&legacy.payload, &record.player_id, registry, Vec::new())?;
        return Ok((state, Vec::new()));
    }
    let get = |id: &str| {
        record
            .components
            .iter()
            .find(|component| component.id == id)
            .ok_or(PlayerCodecError::Invalid(
                "required player component missing",
            ))
    };
    let transform = get(TRANSFORM_COMPONENT)?;
    let inventory = get(INVENTORY_COMPONENT)?;
    let mode = get(GAME_MODE_COMPONENT)?;
    for component in [transform, inventory, mode] {
        if component.schema_version != 1 {
            return Err(PlayerCodecError::Invalid(
                "unsupported player component schema",
            ));
        }
    }
    if transform.payload.len() != 20 || mode.payload.len() != 1 {
        return Err(PlayerCodecError::Invalid(
            "invalid transform/game-mode component length",
        ));
    }
    let mut payload = Vec::with_capacity(21 + inventory.payload.len());
    payload.extend_from_slice(&transform.payload);
    payload.extend_from_slice(&mode.payload);
    payload.extend_from_slice(&inventory.payload);
    let receipts = record
        .components
        .iter()
        .find(|component| component.id == PICKUP_RECEIPTS_COMPONENT)
        .map(decode_pickup_receipts)
        .transpose()?
        .unwrap_or_default();
    let state = decode_payload(&payload, &record.player_id, registry, receipts)?;
    let unknown = record
        .components
        .iter()
        .filter(|component| {
            !matches!(
                component.id.as_str(),
                TRANSFORM_COMPONENT
                    | INVENTORY_COMPONENT
                    | GAME_MODE_COMPONENT
                    | PICKUP_RECEIPTS_COMPONENT
            )
        })
        .cloned()
        .collect();
    Ok((state, unknown))
}

fn decode_payload(
    payload: &[u8],
    player_id: &str,
    registry: &BlockRegistry,
    pickup_receipts: Vec<PickupReceipt>,
) -> Result<RestoredPlayer, PlayerCodecError> {
    if payload.len() > 32 * 1024 {
        return Err(PlayerCodecError::Invalid("payload limit"));
    }
    let mut r = Read::new(payload);
    let position = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
    let yaw = r.f32()?;
    let pitch = r.f32()?;
    if ![position.x, position.y, position.z, yaw, pitch]
        .iter()
        .all(|n| n.is_finite())
        || pitch.abs() > 1.5709
    {
        return Err(PlayerCodecError::Invalid("invalid position/orientation"));
    }
    if !supported_coordinate(position.x)
        || !supported_coordinate(position.y)
        || !supported_coordinate(position.z)
    {
        return Err(PlayerCodecError::Invalid(
            "position outside supported world coordinates",
        ));
    }
    let mode = match r.u8()? {
        0 => GameMode::Development,
        1 => GameMode::Survival,
        _ => return Err(PlayerCodecError::Invalid("invalid game mode")),
    };
    let selected = r.u8()? as usize;
    if selected >= HOTBAR_SLOTS {
        return Err(PlayerCodecError::Invalid(
            "selected hotbar slot out of range",
        ));
    }
    if r.u8()? as usize != SLOT_COUNT {
        return Err(PlayerCodecError::Invalid("inventory slot count mismatch"));
    }
    let mut slots = [None; SLOT_COUNT];
    for (i, slot) in slots.iter_mut().enumerate() {
        *slot = decode_stack(&mut r, registry, player_id, &format!("slot {i}"))?;
    }
    let cursor = decode_stack(&mut r, registry, player_id, "cursor")?;
    if r.u8()? != 4 {
        return Err(PlayerCodecError::Invalid("crafting slot count mismatch"));
    }
    let mut crafting = [None; 4];
    for (i, slot) in crafting.iter_mut().enumerate() {
        *slot = decode_stack(&mut r, registry, player_id, &format!("crafting slot {i}"))?;
    }
    r.finish()?;
    let inventory = Inventory::from_slots(slots, selected).ok_or(PlayerCodecError::Invalid(
        "selected hotbar slot out of range",
    ))?;
    Ok(RestoredPlayer {
        position,
        yaw,
        pitch,
        mode,
        inventory,
        cursor,
        crafting,
        pickup_receipts,
    })
}

fn supported_coordinate(value: f32) -> bool {
    let value = f64::from(value.floor());
    value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX)
}

pub fn apply(sim: &mut Simulation, state: RestoredPlayer) {
    sim.player.position = state.position;
    sim.player.velocity = Vec3::ZERO;
    sim.player.yaw = state.yaw;
    sim.player.pitch = state.pitch;
    sim.player.on_ground = false;
    sim.mode = state.mode;
    sim.inventory = state.inventory;
    sim.inventory_cursor = state.cursor;
    sim.crafting_grid = state.crafting;
    sim.restore_pickup_receipts(state.pickup_receipts);
}

fn encode_pickup_receipts(sim: &Simulation) -> Result<Vec<u8>, PlayerCodecError> {
    let receipts = sim.pickup_receipts();
    if receipts.len() > MAX_PICKUP_RECEIPTS {
        return Err(PlayerCodecError::Invalid("pickup receipt count limit"));
    }
    let mut out = Vec::with_capacity(2 + receipts.len() * 24);
    out.extend_from_slice(&(receipts.len() as u16).to_le_bytes());
    for receipt in receipts {
        if receipt.entity_id == EntityId::NIL {
            return Err(PlayerCodecError::Invalid("zero pickup receipt entity id"));
        }
        out.extend_from_slice(&receipt.entity_id.0.to_le_bytes());
        out.extend_from_slice(&receipt.source.x.to_le_bytes());
        out.extend_from_slice(&receipt.source.z.to_le_bytes());
    }
    Ok(out)
}

fn decode_pickup_receipts(
    component: &PlayerComponent,
) -> Result<Vec<PickupReceipt>, PlayerCodecError> {
    if component.schema_version != PICKUP_RECEIPTS_SCHEMA_VERSION {
        return Err(PlayerCodecError::Invalid(
            "unsupported pickup receipt component schema",
        ));
    }
    let mut reader = Read::new(&component.payload);
    let count = usize::from(reader.u16()?);
    if count > MAX_PICKUP_RECEIPTS {
        return Err(PlayerCodecError::Invalid("pickup receipt count limit"));
    }
    let mut receipts = Vec::with_capacity(count);
    for _ in 0..count {
        let entity_id = EntityId(u128::from_le_bytes(reader.take(16)?.try_into().unwrap()));
        if entity_id == EntityId::NIL {
            return Err(PlayerCodecError::Invalid("zero pickup receipt entity id"));
        }
        let source = ChunkPos {
            x: i32::from_le_bytes(reader.take(4)?.try_into().unwrap()),
            z: i32::from_le_bytes(reader.take(4)?.try_into().unwrap()),
        };
        receipts.push(PickupReceipt { entity_id, source });
    }
    reader.finish()?;
    receipts.sort_by_key(|receipt| receipt.entity_id);
    if receipts
        .windows(2)
        .any(|pair| pair[0].entity_id == pair[1].entity_id)
    {
        return Err(PlayerCodecError::Invalid("duplicate pickup receipt"));
    }
    Ok(receipts)
}

fn encode_stack(
    out: &mut Vec<u8>,
    stack: Option<ItemStack>,
    registry: &BlockRegistry,
    location: &str,
) -> Result<(), PlayerCodecError> {
    let Some(stack) = stack else {
        out.push(0);
        return Ok(());
    };
    let Some(def) = registry.item(stack.item) else {
        return Err(PlayerCodecError::Invalid(
            "runtime item handle is unresolved",
        ));
    };
    validate_stack(
        stack,
        def.max_stack,
        def.tool.map(|t| t.durability),
        location,
    )?;
    let key = def.name.as_bytes();
    if key.is_empty() || key.len() > MAX_KEY_BYTES {
        return Err(PlayerCodecError::Invalid("semantic item key length"));
    }
    out.push(1);
    out.extend_from_slice(&(key.len() as u16).to_le_bytes());
    out.extend_from_slice(key);
    out.extend_from_slice(&stack.count.to_le_bytes());
    out.extend_from_slice(&stack.damage.to_le_bytes());
    Ok(())
}

fn decode_stack(
    r: &mut Read<'_>,
    registry: &BlockRegistry,
    player: &str,
    location: &str,
) -> Result<Option<ItemStack>, PlayerCodecError> {
    match r.u8()? {
        0 => Ok(None),
        1 => {
            let n = r.u16()? as usize;
            if n == 0 || n > MAX_KEY_BYTES {
                return Err(PlayerCodecError::Invalid("semantic item key length"));
            }
            let key = std::str::from_utf8(r.take(n)?)
                .map_err(|_| PlayerCodecError::Invalid("semantic item key encoding"))?;
            let Some(def) = registry.items().iter().find(|item| item.name == key) else {
                return Err(PlayerCodecError::MissingItem {
                    player: player.into(),
                    location: location.into(),
                    key: key.into(),
                });
            };
            let stack = ItemStack {
                item: def.id,
                count: r.u16()?,
                damage: r.u16()?,
            };
            validate_stack(
                stack,
                def.max_stack,
                def.tool.map(|t| t.durability),
                location,
            )?;
            Ok(Some(stack))
        }
        _ => Err(PlayerCodecError::Invalid("invalid stack tag")),
    }
}

fn validate_stack(
    stack: ItemStack,
    max_stack: u16,
    durability: Option<u16>,
    location: &str,
) -> Result<(), PlayerCodecError> {
    if stack.count == 0 || stack.count > max_stack {
        return Err(PlayerCodecError::Invalid("stack count out of range"));
    }
    if match durability {
        Some(max) => stack.damage >= max,
        None => stack.damage != 0,
    } {
        return Err(PlayerCodecError::Invalid(
            "item durability encoding out of range",
        ));
    }
    let _ = location;
    Ok(())
}

struct Read<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Read<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], PlayerCodecError> {
        let end = self
            .at
            .checked_add(n)
            .ok_or(PlayerCodecError::Invalid("length overflow"))?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or(PlayerCodecError::Invalid("truncated payload"))?;
        self.at = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, PlayerCodecError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, PlayerCodecError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, PlayerCodecError> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn finish(self) -> Result<(), PlayerCodecError> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(PlayerCodecError::Invalid("trailing payload bytes"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_engine_core::ItemId;
    use rustcraft_mod_api::GameplayModule;
    #[test]
    fn semantic_item_and_all_player_state_roundtrip() {
        let mut registry = BlockRegistry::default();
        crate::blocks::BlocksModule.register(&mut registry).unwrap();
        let mut sim = Simulation::new(
            rustcraft_engine_core::World::new(crate::blocks::AIR.id),
            registry.clone(),
            Vec3::new(4.5, 70.0, -2.5),
        );
        sim.player.yaw = 0.75;
        sim.player.pitch = -0.2;
        sim.mode = GameMode::Survival;
        sim.inventory.insert(ItemId(1), 5, &registry);
        sim.inventory.insert(ItemId(5), 13, &registry);
        let pick = crate::blocks::STONE_PICKAXE.id;
        sim.inventory.insert(pick, 1, &registry);
        let mut damaged = sim.inventory.slot(2).unwrap();
        damaged.damage = 7;
        sim.inventory.swap_slots(2, 4);
        sim.inventory.insert(pick, 1, &registry);
        sim.inventory.select(4);
        sim.inventory_cursor = Some(damaged);
        sim.crafting_grid[2] = Some(damaged);
        let record = encode(&sim).unwrap();
        let restored = decode(&record, &registry).unwrap();
        assert_eq!(restored.position, sim.player.position);
        assert_eq!(restored.yaw, sim.player.yaw);
        assert_eq!(restored.pitch, sim.player.pitch);
        assert_eq!(restored.mode, sim.mode);
        assert_eq!(restored.inventory.slots(), sim.inventory.slots());
        assert_eq!(restored.inventory.selected(), 4);
        assert_eq!(restored.cursor, sim.inventory_cursor);
        assert_eq!(restored.crafting, sim.crafting_grid);
    }

    #[test]
    fn semantic_item_key_resolves_after_runtime_item_handle_changes() {
        use rustcraft_mod_api::GameplayModule;
        let mut original = BlockRegistry::default();
        crate::blocks::BlocksModule.register(&mut original).unwrap();
        let mut sim = Simulation::new(
            rustcraft_engine_core::World::new(crate::blocks::AIR.id),
            original.clone(),
            Vec3::new(0.5, 65.0, 0.5),
        );
        sim.inventory
            .insert(crate::blocks::STONE_PICKAXE.id, 1, &original);
        let record = encode(&sim).unwrap();
        let saved_key = original.item(crate::blocks::STONE_PICKAXE.id).unwrap().name;

        let mut remapped = BlockRegistry::default();
        let mut definition = original
            .item(crate::blocks::STONE_PICKAXE.id)
            .unwrap()
            .clone();
        definition.id = ItemId(777);
        remapped.register_item(definition).unwrap();
        let state = decode(&record, &remapped).unwrap();
        assert_eq!(
            remapped
                .item(state.inventory.slot(0).unwrap().item)
                .unwrap()
                .name,
            saved_key
        );
        assert_eq!(state.inventory.slot(0).unwrap().item, ItemId(777));
    }

    #[test]
    fn components_are_sorted_versioned_and_unknown_payloads_survive_reencoding() {
        let mut registry = BlockRegistry::default();
        crate::blocks::BlocksModule.register(&mut registry).unwrap();
        let sim = Simulation::new(
            rustcraft_engine_core::World::new(crate::blocks::AIR.id),
            registry.clone(),
            Vec3::new(0.5, 65.0, 0.5),
        );
        let future = PlayerComponent {
            id: "some_mod:player/achievement_state".into(),
            schema_version: 4,
            payload: vec![9, 8, 7, 6],
        };
        let record = encode_revision(&sim, 12, std::slice::from_ref(&future)).unwrap();
        assert!(
            record
                .components
                .windows(2)
                .all(|pair| pair[0].id < pair[1].id)
        );
        assert_eq!(record.revision, 12);
        let (_, unknown) = decode_with_unknown(&record, &registry).unwrap();
        assert_eq!(unknown.as_slice(), std::slice::from_ref(&future));
        let saved_again = encode_revision(&sim, 13, &unknown).unwrap();
        assert!(saved_again.components.contains(&future));
    }

    #[test]
    fn legacy_monolithic_payload_migrates_and_required_schema_errors_are_explicit() {
        let mut registry = BlockRegistry::default();
        crate::blocks::BlocksModule.register(&mut registry).unwrap();
        let mut sim = Simulation::new(
            rustcraft_engine_core::World::new(crate::blocks::AIR.id),
            registry.clone(),
            Vec3::new(-12.5, 72.0, 18.5),
        );
        sim.mode = GameMode::Survival;
        sim.player.yaw = 1.0;
        let legacy = PlayerRecord {
            player_id: LOCAL_PLAYER_ID.into(),
            revision: 0,
            components: vec![PlayerComponent {
                id: LEGACY_COMPONENT.into(),
                schema_version: PLAYER_SCHEMA_VERSION,
                payload: encode_legacy_payload(&sim).unwrap(),
            }],
            recovered_from_checkpoint: false,
        };
        let (restored, unknown) = decode_with_unknown(&legacy, &registry).unwrap();
        assert!(unknown.is_empty());
        assert_eq!(restored.position, sim.player.position);
        assert_eq!(restored.mode, GameMode::Survival);
        let migrated = encode_revision(&sim, 1, &unknown).unwrap();
        assert!(
            migrated
                .components
                .iter()
                .any(|c| c.id == TRANSFORM_COMPONENT)
        );

        let mut invalid = migrated.clone();
        invalid
            .components
            .retain(|component| component.id != INVENTORY_COMPONENT);
        assert!(
            decode(&invalid, &registry)
                .unwrap_err()
                .to_string()
                .contains("required player component missing")
        );
        let mut invalid = migrated;
        invalid
            .components
            .iter_mut()
            .find(|component| component.id == INVENTORY_COMPONENT)
            .unwrap()
            .schema_version = 99;
        assert!(
            decode(&invalid, &registry)
                .unwrap_err()
                .to_string()
                .contains("unsupported player component schema")
        );
    }

    #[test]
    fn missing_semantic_item_is_not_silently_cleared() {
        use rustcraft_mod_api::GameplayModule;
        let mut registry = BlockRegistry::default();
        crate::blocks::BlocksModule.register(&mut registry).unwrap();
        let mut sim = Simulation::new(
            rustcraft_engine_core::World::new(crate::blocks::AIR.id),
            registry.clone(),
            Vec3::new(0.5, 65.0, 0.5),
        );
        sim.inventory
            .insert(crate::blocks::STONE_PICKAXE.id, 1, &registry);
        let record = encode(&sim).unwrap();
        let error = decode(&record, &BlockRegistry::default())
            .unwrap_err()
            .to_string();
        assert!(error.contains("slot 0"));
        assert!(error.contains("minecraft_b173:stone_pickaxe"));
    }

    #[test]
    fn malformed_player_payloads_fail_boundedly() {
        use rustcraft_mod_api::GameplayModule;
        let mut registry = BlockRegistry::default();
        crate::blocks::BlocksModule.register(&mut registry).unwrap();
        let mut sim = Simulation::new(
            rustcraft_engine_core::World::new(crate::blocks::AIR.id),
            registry.clone(),
            Vec3::new(0.5, 65.0, 0.5),
        );
        sim.inventory
            .insert(crate::blocks::STONE_PICKAXE.id, 1, &registry);
        let good = encode(&sim).unwrap();
        let mut bad = good.clone();
        let inventory = bad
            .components
            .iter_mut()
            .find(|component| component.id == INVENTORY_COMPONENT)
            .unwrap();
        inventory.payload.truncate(2);
        assert!(decode(&bad, &registry).is_err());
        let mut bad = good.clone();
        bad.components
            .iter_mut()
            .find(|component| component.id == INVENTORY_COMPONENT)
            .unwrap()
            .payload[0] = 9;
        assert!(
            decode(&bad, &registry)
                .unwrap_err()
                .to_string()
                .contains("selected hotbar")
        );
        let mut bad = good;
        bad.components
            .iter_mut()
            .find(|component| component.id == INVENTORY_COMPONENT)
            .unwrap()
            .payload[2] = 9;
        assert!(decode(&bad, &registry).is_err());
    }

    #[test]
    fn pickup_receipt_recovery_is_idempotent_and_prunes_boundedly() {
        let mut registry = BlockRegistry::default();
        crate::blocks::BlocksModule.register(&mut registry).unwrap();
        let mut picked = Simulation::new_with_entity_namespace(
            rustcraft_engine_core::World::new(crate::blocks::AIR.id),
            registry.clone(),
            Vec3::new(0.5, 3.0, 0.5),
            7,
        );
        picked.mode = GameMode::Survival;
        picked.spawn_item(ItemId(1), 3, Vec3::new(0.5, 3.0, 0.5));
        picked.items[0].pickup_delay = 0.0;
        let stale_source = picked.items[0];
        picked.step(Default::default(), 0.0);
        assert!(picked.items.is_empty());
        let receipt_record = encode_revision(&picked, 4, &[]).unwrap();

        // Repeating recovery from inventory+receipt plus the same stale source never inserts the
        // stack again and never reactivates the entity.
        for _ in 0..3 {
            let state = decode(&receipt_record, &registry).unwrap();
            let mut recovered = Simulation::new_with_entity_namespace(
                rustcraft_engine_core::World::new(crate::blocks::AIR.id),
                registry.clone(),
                Vec3::ZERO,
                8,
            );
            apply(&mut recovered, state);
            recovered
                .activate_entity_column(stale_source.column(), vec![stale_source], &[])
                .unwrap();
            assert!(recovered.items.is_empty());
            assert_eq!(recovered.inventory.slot(0).unwrap().count, 3);
        }

        let ids = picked
            .pickup_receipts()
            .into_iter()
            .map(|receipt| receipt.entity_id)
            .collect::<Vec<_>>();
        picked.commit_pickup_receipts(&ids);
        let snapshot = picked.entity_column_snapshot(stale_source.column());
        assert!(picked.note_entity_column_persisted(&snapshot));
        assert!(picked.pickup_receipts().is_empty());
        let pruned = encode_revision(&picked, 5, &[]).unwrap();
        assert!(
            decode(&pruned, &registry)
                .unwrap()
                .pickup_receipts
                .is_empty()
        );

        let source = ChunkPos { x: -2, z: 3 };
        picked.restore_pickup_receipts((1..=MAX_PICKUP_RECEIPTS as u64).map(|counter| {
            PickupReceipt {
                entity_id: EntityId::from_parts(99, counter),
                source,
            }
        }));
        assert_eq!(picked.pickup_receipts().len(), MAX_PICKUP_RECEIPTS);
        let stress_record = encode_revision(&picked, 6, &[]).unwrap();
        assert_eq!(
            decode(&stress_record, &registry)
                .unwrap()
                .pickup_receipts
                .len(),
            MAX_PICKUP_RECEIPTS
        );
        let stress_ids = picked
            .pickup_receipts()
            .into_iter()
            .map(|receipt| receipt.entity_id)
            .collect::<Vec<_>>();
        picked.commit_pickup_receipts(&stress_ids);
        let stress_snapshot = picked.entity_column_snapshot(source);
        assert!(picked.note_entity_column_persisted(&stress_snapshot));
        assert!(picked.pickup_receipts().is_empty());
    }
}
