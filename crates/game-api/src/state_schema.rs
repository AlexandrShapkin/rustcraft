//! Definition-local state. Semantic data is confined to compilation/serialization boundaries.
use crate::{ContentId, RegistrationError};
use rustcraft_engine_core::orientation::{Axis, Facing, ModelRotation, OrientationProperty};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    Lower,
    Upper,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateValue {
    Facing(Facing),
    Axis(Axis),
    Half(Half),
    Shape(ContentId),
    Powered(bool),
    Connections(u8),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateDomain {
    Facing,
    Axis,
    Half,
    Shape(Vec<ContentId>),
    Powered,
    Connections { mask: u8 },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateField {
    pub key: ContentId,
    pub domain: StateDomain,
    pub default: StateValue,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateAssignment {
    pub key: ContentId,
    pub value: StateValue,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSchema {
    pub version: u32,
    pub fields: Vec<StateField>,
    /// Full semantic combinations that this definition prohibits.
    pub forbidden: Vec<Vec<StateAssignment>>,
}
impl Default for StateSchema {
    fn default() -> Self {
        Self {
            version: 1,
            fields: vec![],
            forbidden: vec![],
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct CompiledField {
    key: ContentId,
    values: Vec<StateValue>,
    stride: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledStateSchema {
    pub version: u32,
    fields: Vec<CompiledField>,
    count: u32,
    forbidden: Vec<u16>,
    legacy: Option<OrientationProperty>,
    facing: Option<usize>,
    axis: Option<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    InvalidVariant,
    UnknownField,
    DuplicateField,
    MissingField,
    InvalidValue,
    WrongSchema,
}

impl StateDomain {
    fn values(&self) -> Result<Vec<StateValue>, RegistrationError> {
        use StateValue as V;
        Ok(match self {
            Self::Facing => vec![
                V::Facing(Facing::North),
                V::Facing(Facing::East),
                V::Facing(Facing::South),
                V::Facing(Facing::West),
                V::Facing(Facing::Up),
                V::Facing(Facing::Down),
            ],
            Self::Axis => vec![V::Axis(Axis::Y), V::Axis(Axis::X), V::Axis(Axis::Z)],
            Self::Half => vec![V::Half(Half::Lower), V::Half(Half::Upper)],
            Self::Powered => vec![V::Powered(false), V::Powered(true)],
            Self::Connections { mask } => {
                if *mask > 63 {
                    return Err(RegistrationError::InvalidDefinition);
                }
                (0..=*mask)
                    .filter(|v| v & !mask == 0)
                    .map(V::Connections)
                    .collect()
            }
            Self::Shape(keys) => {
                let mut keys = keys.clone();
                keys.sort();
                if keys.is_empty() || keys.windows(2).any(|v| v[0] == v[1]) {
                    return Err(RegistrationError::InvalidDefinition);
                }
                keys.into_iter().map(V::Shape).collect()
            }
        })
    }
}
impl StateSchema {
    pub fn compile(&self) -> Result<CompiledStateSchema, RegistrationError> {
        if self.version == 0 {
            return Err(RegistrationError::InvalidDefinition);
        }
        let mut authored = self.fields.clone();
        authored.sort_by(|a, b| a.key.cmp(&b.key));
        if authored.len() > 16 || authored.windows(2).any(|v| v[0].key == v[1].key) {
            return Err(RegistrationError::InvalidDefinition);
        }
        let mut out = CompiledStateSchema {
            version: self.version,
            fields: vec![],
            count: 1,
            forbidden: vec![],
            legacy: None,
            facing: None,
            axis: None,
        };
        for f in authored {
            let mut values = f.domain.values()?;
            let default = values
                .iter()
                .position(|v| v == &f.default)
                .ok_or(RegistrationError::InvalidDefinition)?;
            let value = values.remove(default);
            values.insert(0, value);
            let slot = out.fields.len();
            let orientation_slot = match f.domain {
                StateDomain::Facing => Some(&mut out.facing),
                StateDomain::Axis => Some(&mut out.axis),
                _ => None,
            };
            if let Some(index) = orientation_slot
                && index.replace(slot).is_some()
            {
                return Err(RegistrationError::InvalidDefinition);
            }
            let stride = out.count;
            out.count = out
                .count
                .checked_mul(values.len() as u32)
                .filter(|v| *v <= 65536)
                .ok_or(RegistrationError::InvalidDefinition)?;
            out.fields.push(CompiledField {
                key: f.key,
                values,
                stride,
            });
        }
        let mut forbidden = Vec::new();
        for state in &self.forbidden {
            let variant = out
                .encode(state)
                .map_err(|_| RegistrationError::InvalidDefinition)?;
            if variant == 0 {
                return Err(RegistrationError::InvalidDefinition);
            }
            forbidden.push(variant);
        }
        forbidden.sort_unstable();
        forbidden.dedup();
        out.forbidden = forbidden;
        Ok(out)
    }
}
impl CompiledStateSchema {
    /// Explicit compatibility for existing key+variant saves; no reinterpretation of old bits.
    pub fn legacy(orientation: OrientationProperty) -> Self {
        Self {
            version: 0,
            fields: vec![],
            count: 65536,
            forbidden: vec![],
            legacy: Some(orientation),
            facing: None,
            axis: None,
        }
    }
    pub(crate) fn hash_contract(&self, hasher: &mut blake3::Hasher) {
        hasher.update(b"definition-state-schema");
        hasher.update(&self.version.to_le_bytes());
        hasher.update(&(self.fields.len() as u32).to_le_bytes());
        for f in &self.fields {
            hasher.update(&(f.key.as_str().len() as u32).to_le_bytes());
            hasher.update(f.key.as_str().as_bytes());
            hasher.update(&(f.values.len() as u32).to_le_bytes());
            for value in &f.values {
                let name = value_name(value);
                hasher.update(&(name.len() as u32).to_le_bytes());
                hasher.update(name.as_bytes());
            }
        }
        hasher.update(&(self.forbidden.len() as u32).to_le_bytes());
        for variant in &self.forbidden {
            hasher.update(&variant.to_le_bytes());
        }
    }
    pub fn is_legacy(&self) -> bool {
        self.legacy.is_some()
    }
    pub fn validate(&self, variant: u16) -> Result<(), StateError> {
        if u32::from(variant) >= self.count || self.forbidden.binary_search(&variant).is_ok() {
            Err(StateError::InvalidVariant)
        } else {
            Ok(())
        }
    }
    pub fn field_slot(&self, key: &ContentId) -> Option<usize> {
        self.fields.binary_search_by(|f| f.key.cmp(key)).ok()
    }
    /// Indexed value access: no parsing, allocation or property-map lookup.
    pub fn value(&self, slot: usize, variant: u16) -> Result<&StateValue, StateError> {
        self.validate(variant)?;
        let f = self.fields.get(slot).ok_or(StateError::UnknownField)?;
        Ok(&f.values[((u32::from(variant) / f.stride) % f.values.len() as u32) as usize])
    }
    pub fn encode(&self, values: &[StateAssignment]) -> Result<u16, StateError> {
        if self.is_legacy() {
            return Err(StateError::WrongSchema);
        }
        let mut seen = 0u32;
        let mut encoded = 0u32;
        for a in values {
            let slot = self.field_slot(&a.key).ok_or(StateError::UnknownField)?;
            if seen & (1 << slot) != 0 {
                return Err(StateError::DuplicateField);
            }
            seen |= 1 << slot;
            let f = &self.fields[slot];
            let digit = f
                .values
                .iter()
                .position(|v| v == &a.value)
                .ok_or(StateError::InvalidValue)?;
            encoded += digit as u32 * f.stride;
        }
        if seen != (1 << self.fields.len()) - 1 {
            return Err(StateError::MissingField);
        }
        let variant = u16::try_from(encoded).map_err(|_| StateError::InvalidVariant)?;
        self.validate(variant)?;
        Ok(variant)
    }
    pub fn decode(&self, variant: u16) -> Result<Vec<StateAssignment>, StateError> {
        if self.is_legacy() {
            return Err(StateError::WrongSchema);
        }
        self.validate(variant)?;
        self.fields
            .iter()
            .enumerate()
            .map(|(slot, f)| {
                Ok(StateAssignment {
                    key: f.key.clone(),
                    value: self.value(slot, variant)?.clone(),
                })
            })
            .collect()
    }
    pub fn rotation(&self, variant: u16) -> Result<ModelRotation, StateError> {
        self.validate(variant)?;
        if let Some(property) = self.legacy {
            return Ok(rustcraft_engine_core::BlockState {
                block: rustcraft_engine_core::BlockId(0),
                variant,
            }
            .model_rotation(property));
        }
        let mut rotation = ModelRotation::IDENTITY;
        if let Some(slot) = self.facing
            && let StateValue::Facing(v) = self.value(slot, variant)?
        {
            rotation = rotation.compose(ModelRotation::facing(*v));
        }
        if let Some(slot) = self.axis
            && let StateValue::Axis(v) = self.value(slot, variant)?
        {
            rotation = rotation.compose(ModelRotation::axis(*v));
        }
        Ok(rotation)
    }
    pub fn export(&self, variant: u16) -> Result<StateRecord, StateError> {
        self.validate(variant)?;
        Ok(StateRecord {
            schema_version: self.version,
            legacy_variant: self.legacy.map(|_| variant),
            values: if self.is_legacy() {
                vec![]
            } else {
                self.decode(variant)?
                    .into_iter()
                    .map(|v| SemanticStateValue {
                        key: v.key.as_str().into(),
                        value: value_name(&v.value),
                    })
                    .collect()
            },
        })
    }
    pub fn import(&self, record: &StateRecord) -> Result<u16, StateError> {
        if record.schema_version != self.version {
            return Err(StateError::WrongSchema);
        }
        if self.is_legacy() {
            if !record.values.is_empty() {
                return Err(StateError::InvalidValue);
            }
            let v = record.legacy_variant.ok_or(StateError::MissingField)?;
            self.validate(v)?;
            return Ok(v);
        }
        if record.legacy_variant.is_some() {
            return Err(StateError::WrongSchema);
        }
        if record.values.len() != self.fields.len() {
            return Err(StateError::MissingField);
        }
        if record
            .values
            .iter()
            .zip(&self.fields)
            .any(|(value, field)| value.key != field.key.as_str())
        {
            return Err(StateError::InvalidValue);
        }
        let values = record
            .values
            .iter()
            .map(|a| {
                let key = ContentId::parse(&a.key).map_err(|_| StateError::UnknownField)?;
                let slot = self.field_slot(&key).ok_or(StateError::UnknownField)?;
                let value = self.fields[slot]
                    .values
                    .iter()
                    .find(|v| value_name(v) == a.value)
                    .ok_or(StateError::InvalidValue)?
                    .clone();
                Ok(StateAssignment { key, value })
            })
            .collect::<Result<Vec<_>, StateError>>()?;
        self.encode(&values)
    }
}
fn value_name(v: &StateValue) -> String {
    match v {
        StateValue::Facing(v) => format!("{v:?}").to_lowercase(),
        StateValue::Axis(v) => format!("{v:?}").to_lowercase(),
        StateValue::Half(v) => format!("{v:?}").to_lowercase(),
        StateValue::Shape(v) => v.as_str().into(),
        StateValue::Powered(v) => v.to_string(),
        StateValue::Connections(v) => v.to_string(),
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRecord {
    pub schema_version: u32,
    pub legacy_variant: Option<u16>,
    pub values: Vec<SemanticStateValue>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticStateValue {
    pub key: String,
    pub value: String,
}

/// Serializable semantic boundary record; not a network protocol or a changed chunk envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticVoxelState {
    pub block: String,
    pub state: StateRecord,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(s: &str) -> ContentId {
        ContentId::parse(format!("test:state/{s}")).unwrap()
    }
    fn field(name: &str, domain: StateDomain, default: StateValue) -> StateField {
        StateField {
            key: key(name),
            domain,
            default,
        }
    }
    #[test]
    fn definition_local_domains_are_compact_canonical_and_reorder_independent() {
        let schemas = [
            vec![
                field(
                    "facing",
                    StateDomain::Facing,
                    StateValue::Facing(Facing::North),
                ),
                field("powered", StateDomain::Powered, StateValue::Powered(false)),
                field(
                    "connections",
                    StateDomain::Connections { mask: 63 },
                    StateValue::Connections(0),
                ),
            ],
            vec![field("axis", StateDomain::Axis, StateValue::Axis(Axis::Y))],
            vec![
                field("half", StateDomain::Half, StateValue::Half(Half::Lower)),
                field(
                    "shape",
                    StateDomain::Shape(vec![key("straight"), key("inner"), key("outer")]),
                    StateValue::Shape(key("straight")),
                ),
            ],
        ];
        assert_eq!(std::mem::size_of::<rustcraft_engine_core::BlockState>(), 8);
        for fields in schemas {
            let schema = StateSchema {
                fields: fields.clone(),
                ..Default::default()
            }
            .compile()
            .unwrap();
            let mut reversed = fields;
            reversed.reverse();
            let other = StateSchema {
                fields: reversed,
                ..Default::default()
            }
            .compile()
            .unwrap();
            assert_eq!(schema, other);
            if schema.fields.len() > 1 {
                let mut noncanonical = schema.export(0).unwrap();
                noncanonical.values.reverse();
                assert_eq!(schema.import(&noncanonical), Err(StateError::InvalidValue));
            }
            for variant in 0..schema.count {
                let decoded = schema.decode(variant as u16).unwrap();
                assert_eq!(schema.encode(&decoded), Ok(variant as u16));
                let record = schema.export(variant as u16).unwrap();
                assert_eq!(other.import(&record), Ok(variant as u16));
            }
            assert_eq!(
                schema.validate(schema.count as u16),
                Err(StateError::InvalidVariant)
            );
        }
    }
    #[test]
    fn schema_rejects_invalid_defaults_limits_combinations_and_noncanonical_input() {
        let mut schema = StateSchema {
            fields: vec![field(
                "powered",
                StateDomain::Powered,
                StateValue::Powered(false),
            )],
            ..Default::default()
        };
        schema.forbidden = vec![vec![StateAssignment {
            key: key("powered"),
            value: StateValue::Powered(true),
        }]];
        let compiled = schema.compile().unwrap();
        assert_eq!(compiled.validate(1), Err(StateError::InvalidVariant));
        assert_eq!(compiled.encode(&[]), Err(StateError::MissingField));
        let assignment = StateAssignment {
            key: key("powered"),
            value: StateValue::Powered(false),
        };
        assert_eq!(
            compiled.encode(&[assignment.clone(), assignment]),
            Err(StateError::DuplicateField)
        );
        let mut record = compiled.export(0).unwrap();
        record.values[0].value = "not a bool".into();
        assert_eq!(compiled.import(&record), Err(StateError::InvalidValue));
        record.schema_version += 1;
        assert_eq!(compiled.import(&record), Err(StateError::WrongSchema));
        schema.fields[0].default = StateValue::Axis(Axis::X);
        assert!(schema.compile().is_err());
        schema.fields = vec![
            field(
                "a",
                StateDomain::Connections { mask: 63 },
                StateValue::Connections(0),
            ),
            field(
                "b",
                StateDomain::Connections { mask: 63 },
                StateValue::Connections(0),
            ),
            field(
                "c",
                StateDomain::Connections { mask: 63 },
                StateValue::Connections(0),
            ),
        ];
        assert!(schema.compile().is_err());
        schema.fields[0].domain = StateDomain::Connections { mask: 255 };
        assert!(schema.compile().is_err());
    }
    #[test]
    fn legacy_variants_remain_byte_compatible_without_reinterpretation() {
        for orientation in [
            OrientationProperty::None,
            OrientationProperty::Facing,
            OrientationProperty::Axis,
            OrientationProperty::HorizontalRotation,
        ] {
            let schema = CompiledStateSchema::legacy(orientation);
            for variant in [0, 1, 12, 64, u16::MAX] {
                let record = schema.export(variant).unwrap();
                assert_eq!(schema.import(&record), Ok(variant));
                assert_eq!(
                    schema.rotation(variant).unwrap(),
                    rustcraft_engine_core::BlockState {
                        block: rustcraft_engine_core::BlockId(93),
                        variant
                    }
                    .model_rotation(orientation)
                );
            }
        }
    }
    #[test]
    fn forbidden_combination_order_and_duplicates_compile_identically() {
        let state = |value| {
            vec![StateAssignment {
                key: key("connections"),
                value: StateValue::Connections(value),
            }]
        };
        let schema = StateSchema {
            fields: vec![field(
                "connections",
                StateDomain::Connections { mask: 3 },
                StateValue::Connections(0),
            )],
            forbidden: vec![state(3), state(1), state(2), state(3)],
            ..Default::default()
        };
        let compiled = schema.compile().unwrap();
        let mut reversed = schema;
        reversed.forbidden.reverse();
        assert_eq!(compiled, reversed.compile().unwrap());
        assert_eq!(compiled.validate(0), Ok(()));
        for v in 1..=3 {
            assert_eq!(compiled.validate(v), Err(StateError::InvalidVariant));
        }
    }
}
