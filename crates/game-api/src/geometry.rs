//! Semantic static geometry registration and definition-local state compilation.
use crate::{
    ModelKey, RegistrationError, ShapeKey,
    state_schema::{CompiledStateSchema, StateAssignment, StateError},
};
use rustcraft_engine_core::{
    orientation::ModelRotation,
    shape::{RenderGeometry, Shape, Triangle},
};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelHandle(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeHandle(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightCoverage {
    Full,
    Partial,
    None,
    Transmitting,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ModelDefinition {
    pub key: ModelKey,
    pub geometry: RenderGeometry,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeDefinition {
    pub key: ShapeKey,
    pub shape: Shape,
}
#[derive(Debug, Clone, PartialEq)]
pub struct GeometryRoles {
    pub model: ModelKey,
    pub collision: ShapeKey,
    pub selection: ShapeKey,
    pub transform: ModelRotation,
    pub light: LightCoverage,
}
#[derive(Debug, Clone, PartialEq)]
pub struct GeometryRule {
    pub when: Vec<StateAssignment>,
    pub roles: GeometryRoles,
}
#[derive(Debug, Clone, PartialEq)]
pub struct GeometryBinding {
    pub default: GeometryRoles,
    pub rules: Vec<GeometryRule>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedGeometry {
    pub model: ModelHandle,
    pub collision: ShapeHandle,
    pub selection: ShapeHandle,
    pub transform: ModelRotation,
    pub light: LightCoverage,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledModel {
    pub key: ModelKey,
    /// None is the specialized full-cube path, never a triangle iterator.
    pub triangles: Option<Arc<[Triangle]>>,
    /// Conservative full boundary coverage in +Z,-Z,+X,-X,+Y,-Y order.
    pub coverage: [Coverage; 6],
    pub boundary_masks: [u16; 6],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    None,
    Partial,
    Full,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledShape {
    pub key: ShapeKey,
    pub shape: Shape,
    pub edges: Option<Arc<[[[f32; 3]; 2]]>>,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GeometryCatalog {
    pub models: Vec<CompiledModel>,
    pub shapes: Vec<CompiledShape>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledGeometryBinding {
    entries: Arc<[Option<ResolvedGeometry>]>,
}
impl CompiledGeometryBinding {
    pub fn resolve(&self, variant: u16) -> Result<ResolvedGeometry, StateError> {
        self.entries
            .get(usize::from(variant))
            .copied()
            .flatten()
            .ok_or(StateError::InvalidVariant)
    }
}
impl GeometryCatalog {
    pub(crate) fn compile(
        models: &[ModelDefinition],
        shapes: &[ShapeDefinition],
    ) -> Result<Self, RegistrationError> {
        let mut models = models.to_vec();
        models.sort_by(|a, b| a.key.cmp(&b.key));
        let mut shapes = shapes.to_vec();
        shapes.sort_by(|a, b| a.key.cmp(&b.key));
        if models.len() > 4096
            || shapes.len() > 4096
            || models.windows(2).any(|p| p[0].key == p[1].key)
            || shapes.windows(2).any(|p| p[0].key == p[1].key)
        {
            return Err(RegistrationError::InvalidDefinition);
        }
        Ok(Self {
            models: models
                .into_iter()
                .map(|m| {
                    if !m.geometry.valid() {
                        return Err(RegistrationError::InvalidDefinition);
                    }
                    let (coverage, boundary_masks) = coverage(&m.geometry);
                    let triangles = if matches!(m.geometry, RenderGeometry::FullCube) {
                        None
                    } else {
                        let triangles = m.geometry.triangles();
                        if !rustcraft_engine_core::shape::valid_mesh(&triangles) {
                            return Err(RegistrationError::InvalidDefinition);
                        }
                        Some(Arc::from(triangles))
                    };
                    Ok(CompiledModel {
                        key: m.key,
                        triangles,
                        coverage,
                        boundary_masks,
                    })
                })
                .collect::<Result<_, _>>()?,
            shapes: shapes
                .into_iter()
                .map(|s| {
                    if !s.shape.valid(false) {
                        return Err(RegistrationError::InvalidDefinition);
                    }
                    let edges = selection_edges(&s.shape);
                    Ok(CompiledShape {
                        key: s.key,
                        shape: s.shape,
                        edges,
                    })
                })
                .collect::<Result<_, _>>()?,
        })
    }
    pub(crate) fn binding(
        &self,
        binding: &GeometryBinding,
        schema: &CompiledStateSchema,
    ) -> Result<CompiledGeometryBinding, RegistrationError> {
        if schema.is_legacy() || schema.cardinality() > 4096 || binding.rules.len() > 128 {
            return Err(RegistrationError::InvalidDefinition);
        }
        let models: HashMap<_, _> = self
            .models
            .iter()
            .enumerate()
            .map(|(i, m)| (&m.key, ModelHandle(i as u32)))
            .collect();
        let shapes: HashMap<_, _> = self
            .shapes
            .iter()
            .enumerate()
            .map(|(i, m)| (&m.key, ShapeHandle(i as u32)))
            .collect();
        let resolve = |r: &GeometryRoles| -> Result<ResolvedGeometry, RegistrationError> {
            let model = *models
                .get(&r.model)
                .ok_or(RegistrationError::InvalidDefinition)?;
            let collision = *shapes
                .get(&r.collision)
                .ok_or(RegistrationError::InvalidDefinition)?;
            let selection = *shapes
                .get(&r.selection)
                .ok_or(RegistrationError::InvalidDefinition)?;
            if !self.shapes[collision.0 as usize].shape.valid(true) {
                return Err(RegistrationError::InvalidDefinition);
            }
            Ok(ResolvedGeometry {
                model,
                collision,
                selection,
                transform: r.transform,
                light: r.light,
            })
        };
        let default = resolve(&binding.default)?;
        let mut rules = Vec::new();
        for rule in &binding.rules {
            if rule.when.is_empty() {
                return Err(RegistrationError::InvalidDefinition);
            }
            let mut tests = Vec::new();
            for a in &rule.when {
                let slot = schema
                    .field_slot(&a.key)
                    .ok_or(RegistrationError::InvalidDefinition)?;
                if tests.iter().any(|(s, _)| *s == slot) || !schema.accepts(slot, &a.value) {
                    return Err(RegistrationError::InvalidDefinition);
                }
                tests.push((slot, a.value.clone()));
            }
            rules.push((tests, resolve(&rule.roles)?));
        }
        let mut entries = Vec::new();
        for variant in 0..schema.cardinality() {
            let variant = variant as u16;
            if schema.validate(variant).is_err() {
                entries.push(None);
                continue;
            }
            let mut selected = default;
            let mut matched = false;
            for (tests, roles) in &rules {
                if tests
                    .iter()
                    .all(|(slot, value)| schema.value(*slot, variant) == Ok(value))
                {
                    if matched {
                        return Err(RegistrationError::InvalidDefinition);
                    }
                    selected = *roles;
                    matched = true;
                }
            }
            selected.transform = schema
                .rotation(variant)
                .map_err(|_| RegistrationError::InvalidDefinition)?
                .compose(selected.transform);
            entries.push(Some(selected));
        }
        Ok(CompiledGeometryBinding {
            entries: entries.into(),
        })
    }
}
fn coverage(geometry: &RenderGeometry) -> ([Coverage; 6], [u16; 6]) {
    use rustcraft_engine_core::shape::Primitive;
    if matches!(geometry, RenderGeometry::FullCube) {
        return ([Coverage::Full; 6], [u16::MAX; 6]);
    }
    let RenderGeometry::Primitives(parts) = geometry else {
        return ([Coverage::None; 6], [0; 6]);
    };
    let mut masks = [0; 6];
    let faces = std::array::from_fn(|face| {
        let axis = [2, 2, 0, 0, 1, 1][face];
        let side = if face % 2 == 0 { 1. } else { 0. };
        let uv: Vec<_> = (0..3).filter(|a| *a != axis).collect();
        let mut mask = 0u16;
        let mut touches = false;
        for p in parts {
            let b = p.bounds();
            if (if side == 1. { b.max[axis] } else { b.min[axis] }) != side {
                continue;
            }
            touches = true;
            if matches!(p, Primitive::Wedge(_)) && face != 0 && face != 5 {
                continue;
            }
            for v in 0..4 {
                for u in 0..4 {
                    if b.min[uv[0]] <= u as f32 / 4.
                        && b.max[uv[0]] >= (u + 1) as f32 / 4.
                        && b.min[uv[1]] <= v as f32 / 4.
                        && b.max[uv[1]] >= (v + 1) as f32 / 4.
                    {
                        mask |= 1 << (v * 4 + u);
                    }
                }
            }
        }
        masks[face] = mask;
        if mask == u16::MAX {
            Coverage::Full
        } else if touches {
            Coverage::Partial
        } else {
            Coverage::None
        }
    });
    (faces, masks)
}

impl GeometryCatalog {
    pub(crate) fn hash_binding(
        &self,
        binding: &CompiledGeometryBinding,
        hash: &mut blake3::Hasher,
    ) {
        fn key(hash: &mut blake3::Hasher, key: &str) {
            hash.update(&(key.len() as u32).to_le_bytes());
            hash.update(key.as_bytes());
        }
        fn floats(hash: &mut blake3::Hasher, values: impl IntoIterator<Item = f32>) {
            for value in values {
                hash.update(&value.to_bits().to_le_bytes());
            }
        }
        fn triangle(hash: &mut blake3::Hasher, t: &Triangle) {
            hash.update(&[t.face]);
            floats(hash, t.positions.into_iter().flatten());
            floats(hash, t.uv.into_iter().flatten());
        }
        fn shape(hash: &mut blake3::Hasher, s: &Shape) {
            use rustcraft_engine_core::shape::Primitive;
            match s {
                Shape::Empty => {
                    hash.update(&[0]);
                }
                Shape::FullCube => {
                    hash.update(&[1]);
                }
                Shape::Compound(p) => {
                    hash.update(&[2]);
                    hash.update(&(p.len() as u32).to_le_bytes());
                    for p in p {
                        hash.update(&[u8::from(matches!(p, Primitive::Wedge(_)))]);
                        floats(hash, p.bounds().min);
                        floats(hash, p.bounds().max);
                    }
                }
                Shape::Surface(t) => {
                    hash.update(&[3]);
                    hash.update(&(t.len() as u32).to_le_bytes());
                    for t in t {
                        triangle(hash, t);
                    }
                }
            }
        }
        hash.update(b"bg1.geometry.v1");
        hash.update(&(binding.entries.len() as u32).to_le_bytes());
        for entry in binding.entries.iter() {
            let Some(g) = entry else {
                hash.update(&[0]);
                continue;
            };
            hash.update(&[1]);
            let m = &self.models[g.model.0 as usize];
            key(hash, m.key.as_str());
            for mask in m.boundary_masks {
                hash.update(&mask.to_le_bytes());
            }
            if let Some(t) = &m.triangles {
                hash.update(&[1]);
                hash.update(&(t.len() as u32).to_le_bytes());
                for t in t.iter() {
                    triangle(hash, t);
                }
            } else {
                hash.update(&[0]);
            }
            for handle in [g.collision, g.selection] {
                let s = &self.shapes[handle.0 as usize];
                key(hash, s.key.as_str());
                shape(hash, &s.shape);
            }
            for axis in [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]] {
                floats(hash, g.transform.transform(axis));
            }
            hash.update(&[g.light as u8]);
        }
    }
}

fn selection_edges(shape: &Shape) -> Option<Arc<[[[f32; 3]; 2]]>> {
    let triangles = match shape {
        Shape::FullCube => return None,
        Shape::Empty => Vec::new(),
        Shape::Compound(p) => p.iter().flat_map(|p| p.triangles()).collect(),
        Shape::Surface(t) => t.clone(),
    };
    let mut edges: Vec<([[f32; 3]; 2], [f32; 3], bool)> = Vec::new();
    for triangle in triangles {
        for i in 0..3 {
            let mut e = [triangle.positions[i], triangle.positions[(i + 1) % 3]];
            if (0..3)
                .map(|a| e[0][a].total_cmp(&e[1][a]))
                .find(|c| !c.is_eq())
                .is_some_and(|c| c.is_gt())
            {
                e.swap(0, 1);
            }
            if let Some((_, normal, hide)) = edges.iter_mut().find(|(edge, _, _)| *edge == e) {
                *hide = *normal == triangle.normal();
            } else {
                edges.push((e, triangle.normal(), false));
            }
        }
    }
    Some(
        edges
            .into_iter()
            .filter(|(_, _, hide)| !*hide)
            .map(|(e, _, _)| e)
            .collect::<Vec<_>>()
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ContentId,
        state_schema::{Half, StateDomain, StateField, StateSchema, StateValue},
    };
    use rustcraft_engine_core::shape::{LocalBox, Primitive};
    fn id(s: &str) -> ContentId {
        ContentId::parse(s).unwrap()
    }
    #[test]
    fn semantic_compilation_state_consumers_and_invalid_rules() {
        let low = Primitive::Box(LocalBox {
            min: [0.; 3],
            max: [1., 0.5, 1.],
        });
        let high = Primitive::Box(LocalBox {
            min: [0., 0.5, 0.],
            max: [1.; 3],
        });
        let models = vec![
            ModelDefinition {
                key: ModelKey::parse("proof:lower").unwrap(),
                geometry: RenderGeometry::Primitives(vec![low]),
            },
            ModelDefinition {
                key: ModelKey::parse("proof:upper").unwrap(),
                geometry: RenderGeometry::Primitives(vec![high]),
            },
        ];
        let shapes = vec![
            ShapeDefinition {
                key: ShapeKey::parse("proof:lower").unwrap(),
                shape: Shape::Compound(vec![low]),
            },
            ShapeDefinition {
                key: ShapeKey::parse("proof:upper").unwrap(),
                shape: Shape::Compound(vec![high]),
            },
        ];
        let catalog = GeometryCatalog::compile(&models, &shapes).unwrap();
        let mut reversed = models.clone();
        reversed.reverse();
        assert_eq!(
            catalog,
            GeometryCatalog::compile(&reversed, &shapes).unwrap()
        );
        let schema = StateSchema {
            fields: vec![StateField {
                key: id("proof:half"),
                domain: StateDomain::Half,
                default: StateValue::Half(Half::Lower),
            }],
            ..Default::default()
        }
        .compile()
        .unwrap();
        let roles = |name: &str| GeometryRoles {
            model: ModelKey::parse(name).unwrap(),
            collision: ShapeKey::parse(name).unwrap(),
            selection: ShapeKey::parse(name).unwrap(),
            transform: ModelRotation::IDENTITY,
            light: LightCoverage::Partial,
        };
        let binding = GeometryBinding {
            default: roles("proof:lower"),
            rules: vec![GeometryRule {
                when: vec![StateAssignment {
                    key: id("proof:half"),
                    value: StateValue::Half(Half::Upper),
                }],
                roles: roles("proof:upper"),
            }],
        };
        let compiled = catalog.binding(&binding, &schema).unwrap();
        // Equal triangles with different guaranteed coverage are different content semantics.
        let static_models = models
            .iter()
            .map(|m| ModelDefinition {
                key: m.key.clone(),
                geometry: RenderGeometry::StaticMesh(
                    rustcraft_engine_core::shape::BoundedStaticMesh::from_triangles(
                        &m.geometry.triangles(),
                    )
                    .unwrap(),
                ),
            })
            .collect::<Vec<_>>();
        let static_catalog = GeometryCatalog::compile(&static_models, &shapes).unwrap();
        let static_binding = static_catalog.binding(&binding, &schema).unwrap();
        let mut primitive_hash = blake3::Hasher::new();
        let mut static_hash = blake3::Hasher::new();
        catalog.hash_binding(&compiled, &mut primitive_hash);
        static_catalog.hash_binding(&static_binding, &mut static_hash);
        assert_ne!(primitive_hash.finalize(), static_hash.finalize());
        let a = compiled.resolve(0).unwrap();
        let b = compiled.resolve(1).unwrap();
        assert_ne!(a.model, b.model);
        assert_ne!(a.collision, b.collision);
        assert_ne!(a.selection, b.selection);
        assert!(compiled.resolve(2).is_err());
        let query = LocalBox {
            min: [0.1, 0.1, 0.1],
            max: [0.2, 0.2, 0.2],
        };
        assert!(
            catalog.shapes[a.collision.0 as usize]
                .shape
                .overlaps(query, a.transform)
        );
        assert!(
            !catalog.shapes[b.collision.0 as usize]
                .shape
                .overlaps(query, b.transform)
        );
        let hit = |g: ResolvedGeometry| {
            catalog.shapes[g.selection.0 as usize]
                .shape
                .ray([0.5, 2., 0.5], [0., -1., 0.], g.transform, 3.)
                .unwrap()
        };
        assert_eq!(hit(a).distance, 1.5);
        assert_eq!(hit(b).distance, 1.);
        let mut invalid = binding;
        invalid.rules.push(invalid.rules[0].clone());
        assert!(catalog.binding(&invalid, &schema).is_err());
        assert_eq!(
            catalog.models[a.model.0 as usize].coverage[5],
            Coverage::Full
        );
        assert_eq!(
            catalog.models[a.model.0 as usize].coverage[0],
            Coverage::Partial
        );
    }
}
