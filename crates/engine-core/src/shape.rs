//! Bounded static cell geometry, independent of content policy and renderer resources.
use crate::orientation::ModelRotation;

pub type Point = [f32; 3];
pub const MAX_MODEL_TRIANGLES: usize = 128;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalBox {
    pub min: Point,
    pub max: Point,
}
impl LocalBox {
    pub const UNIT: Self = Self {
        min: [0.; 3],
        max: [1.; 3],
    };
    pub fn valid(self) -> bool {
        (0..3).all(|a| {
            self.min[a].is_finite()
                && self.max[a].is_finite()
                && self.min[a] >= 0.
                && self.max[a] <= 1.
                && self.min[a] < self.max[a]
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Primitive {
    Box(LocalBox),
    /// Triangular prism: within bounds, local height rises linearly from -Z to +Z.
    Wedge(LocalBox),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle {
    pub positions: [Point; 3],
    pub uv: [[f32; 2]; 3],
    /// Material role in +Z, -Z, +X, -X, +Y, -Y order.
    pub face: u8,
}
impl Triangle {
    pub fn normal(self) -> Point {
        let n = cross(
            sub(self.positions[1], self.positions[0]),
            sub(self.positions[2], self.positions[0]),
        );
        let length = dot(n, n).sqrt();
        n.map(|v| v / length)
    }
    pub fn valid(self) -> bool {
        self.face < 6
            && self
                .positions
                .iter()
                .flatten()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            && self
                .uv
                .iter()
                .flatten()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            && {
                let n = cross(
                    sub(self.positions[1], self.positions[0]),
                    sub(self.positions[2], self.positions[0]),
                );
                dot(n, n) > 1e-12
            }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshVertex {
    pub position: Point,
    pub uv: [f32; 2],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshTriangle {
    pub indices: [u16; 3],
    pub face: u8,
}
#[derive(Debug, Clone, PartialEq)]
pub struct BoundedStaticMesh {
    pub vertices: Vec<MeshVertex>,
    pub triangles: Vec<MeshTriangle>,
}
impl BoundedStaticMesh {
    pub fn from_triangles(triangles: &[Triangle]) -> Option<Self> {
        if !valid_mesh(triangles) {
            return None;
        }
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for t in triangles {
            let base = vertices.len() as u16;
            vertices.extend(
                t.positions
                    .into_iter()
                    .zip(t.uv)
                    .map(|(position, uv)| MeshVertex { position, uv }),
            );
            indices.push(MeshTriangle {
                indices: [base, base + 1, base + 2],
                face: t.face,
            });
        }
        Some(Self {
            vertices,
            triangles: indices,
        })
    }
    pub fn valid(&self) -> bool {
        !self.vertices.is_empty()
            && self.vertices.len() <= MAX_MODEL_TRIANGLES * 3
            && !self.triangles.is_empty()
            && self.triangles.len() <= MAX_MODEL_TRIANGLES
            && self.vertices.iter().all(|v| {
                v.position
                    .into_iter()
                    .all(|p| p.is_finite() && (0.0..=1.0).contains(&p))
                    && v.uv
                        .into_iter()
                        .all(|p| p.is_finite() && (0.0..=1.0).contains(&p))
            })
            && self.triangles.iter().all(|t| {
                t.indices
                    .iter()
                    .all(|i| usize::from(*i) < self.vertices.len())
                    && self.expand(*t).valid()
            })
    }
    fn expand(&self, t: MeshTriangle) -> Triangle {
        Triangle {
            positions: t.indices.map(|i| self.vertices[usize::from(i)].position),
            uv: t.indices.map(|i| self.vertices[usize::from(i)].uv),
            face: t.face,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum RenderGeometry {
    FullCube,
    Primitives(Vec<Primitive>),
    StaticMesh(BoundedStaticMesh),
}
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Empty,
    FullCube,
    Compound(Vec<Primitive>),
    /// Surface selection only; collision uses closed primitive volumes.
    Surface(Vec<Triangle>),
}
impl Shape {
    pub fn valid(&self, collision: bool) -> bool {
        match self {
            Self::Empty | Self::FullCube => true,
            Self::Compound(parts) => {
                !parts.is_empty() && parts.len() <= 32 && parts.iter().all(|p| p.bounds().valid())
            }
            Self::Surface(tris) => !collision && valid_mesh(tris),
        }
    }
    /// Query coordinates are model-local, before orthogonal transform.
    pub fn overlaps(&self, bounds: LocalBox, rotation: ModelRotation) -> bool {
        match self {
            Self::Empty | Self::Surface(_) => false,
            Self::FullCube => box_overlap(bounds, LocalBox::UNIT),
            Self::Compound(parts) => parts.iter().any(|p| p.overlaps(bounds, rotation)),
        }
    }
    pub fn ray(
        &self,
        origin: Point,
        direction: Point,
        rotation: ModelRotation,
        reach: f32,
    ) -> Option<ShapeHit> {
        let inverse = rotation.inverse();
        let o = inverse.point(origin);
        let d = inverse.transform(direction);
        let hit = match self {
            Self::Empty => None,
            Self::FullCube => Primitive::Box(LocalBox::UNIT).ray(o, d, reach),
            Self::Compound(parts) => parts
                .iter()
                .filter_map(|p| p.ray(o, d, reach))
                .min_by(|a, b| a.distance.total_cmp(&b.distance)),
            Self::Surface(tris) => tris
                .iter()
                .filter_map(|t| triangle_ray(*t, o, d, reach))
                .min_by(|a, b| a.distance.total_cmp(&b.distance)),
        }?;
        Some(ShapeHit {
            distance: hit.distance,
            normal: rotation.transform(hit.normal),
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeHit {
    pub distance: f32,
    pub normal: Point,
}

pub fn valid_mesh(tris: &[Triangle]) -> bool {
    !tris.is_empty() && tris.len() <= MAX_MODEL_TRIANGLES && tris.iter().all(|t| t.valid())
}
impl RenderGeometry {
    pub fn valid(&self) -> bool {
        match self {
            Self::FullCube => true,
            Self::Primitives(p) => {
                !p.is_empty() && p.len() <= 32 && p.iter().all(|p| p.bounds().valid())
            }
            Self::StaticMesh(t) => t.valid(),
        }
    }
    /// Compilation only. Ordinary cube meshing never calls this.
    pub fn triangles(&self) -> Vec<Triangle> {
        match self {
            Self::FullCube => Primitive::Box(LocalBox::UNIT).triangles(),
            Self::Primitives(parts) => compound_triangles(parts),
            Self::StaticMesh(t) => t.triangles.iter().map(|ix| t.expand(*ix)).collect(),
        }
    }
}
impl Primitive {
    pub fn bounds(self) -> LocalBox {
        match self {
            Self::Box(b) | Self::Wedge(b) => b,
        }
    }
    pub fn triangles(self) -> Vec<Triangle> {
        let b = self.bounds();
        let p = |x: f32, y: f32, z: f32| {
            std::array::from_fn(|a| b.min[a] + [x, y, z][a] * (b.max[a] - b.min[a]))
        };
        let mut out = Vec::new();
        let mut quad = |face, corners: [Point; 4]| {
            let uv = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]];
            for ix in [[0, 1, 2], [0, 2, 3]] {
                out.push(Triangle {
                    positions: ix.map(|i| corners[i]),
                    uv: ix.map(|i| uv[i]),
                    face,
                });
            }
        };
        match self {
            Self::Box(_) => {
                quad(
                    0,
                    [p(0., 0., 1.), p(1., 0., 1.), p(1., 1., 1.), p(0., 1., 1.)],
                );
                quad(
                    1,
                    [p(1., 0., 0.), p(0., 0., 0.), p(0., 1., 0.), p(1., 1., 0.)],
                );
                quad(
                    2,
                    [p(1., 0., 1.), p(1., 0., 0.), p(1., 1., 0.), p(1., 1., 1.)],
                );
                quad(
                    3,
                    [p(0., 0., 0.), p(0., 0., 1.), p(0., 1., 1.), p(0., 1., 0.)],
                );
                quad(
                    4,
                    [p(0., 1., 1.), p(1., 1., 1.), p(1., 1., 0.), p(0., 1., 0.)],
                );
            }
            Self::Wedge(_) => {
                quad(
                    0,
                    [p(0., 0., 1.), p(1., 0., 1.), p(1., 1., 1.), p(0., 1., 1.)],
                );
                quad(
                    4,
                    [p(0., 1., 1.), p(1., 1., 1.), p(1., 0., 0.), p(0., 0., 0.)],
                );
            }
        }
        quad(
            5,
            [p(0., 0., 0.), p(1., 0., 0.), p(1., 0., 1.), p(0., 0., 1.)],
        );
        if matches!(self, Self::Wedge(_)) {
            out.push(Triangle {
                positions: [p(1., 0., 1.), p(1., 0., 0.), p(1., 1., 1.)],
                uv: [[0., 1.], [1., 1.], [0., 0.]],
                face: 2,
            });
            out.push(Triangle {
                positions: [p(0., 0., 0.), p(0., 0., 1.), p(0., 1., 1.)],
                uv: [[0., 1.], [1., 1.], [1., 0.]],
                face: 3,
            });
        }
        out
    }
    fn planes(self) -> [(Point, f32); 7] {
        let b = self.bounds();
        let mut out = [([0.; 3], 0.); 7];
        for a in 0..3 {
            let mut n = [0.; 3];
            n[a] = 1.;
            out[a * 2] = (n, b.max[a]);
            n[a] = -1.;
            out[a * 2 + 1] = (n, -b.min[a]);
        }
        if matches!(self, Self::Wedge(_)) {
            let height = b.max[1] - b.min[1];
            let depth = b.max[2] - b.min[2];
            let scale = height.max(depth);
            let normal = [0., depth / scale, -height / scale];
            out[6] = (normal, normal[1] * b.min[1] + normal[2] * b.min[2]);
        }
        out
    }
    fn ray(self, o: Point, d: Point, reach: f32) -> Option<ShapeHit> {
        let mut enter: f32 = 0.;
        let mut exit = reach;
        let mut normal = [0.; 3];
        for (n, offset) in self.planes() {
            let denominator = dot(n, d);
            let distance = offset - dot(n, o);
            if denominator.abs() < 1e-8 {
                if distance < 0. {
                    return None;
                }
                continue;
            }
            let t = distance / denominator;
            if denominator < 0. {
                if t > enter || (t == enter && normal == [0.; 3]) {
                    enter = t;
                    let length = dot(n, n).sqrt();
                    normal = n.map(|v| v / length);
                }
            } else {
                exit = exit.min(t);
            }
            if enter > exit {
                return None;
            }
        }
        Some(ShapeHit {
            distance: enter,
            normal,
        })
    }
    fn overlaps(self, query: LocalBox, rotation: ModelRotation) -> bool {
        // Orthogonal transform preserves AABBs. SAT additionally needs wedge plane and edge axes.
        let inverse = rotation.inverse();
        let a = inverse.point(query.min);
        let b = inverse.point(query.max);
        let query = LocalBox {
            min: std::array::from_fn(|i| a[i].min(b[i])),
            max: std::array::from_fn(|i| a[i].max(b[i])),
        };
        if !box_overlap(query, self.bounds()) {
            return false;
        }
        if matches!(self, Self::Wedge(_)) {
            let (n, offset) = self.planes()[6];
            // Wedge edges parallel to X; remaining cross axes coincide with box axes or slope normal.
            let closest: Point = std::array::from_fn(|i| {
                if n[i] >= 0. {
                    query.min[i]
                } else {
                    query.max[i]
                }
            });
            return dot(n, closest) < offset;
        }
        true
    }
}
// Split box faces at authored box boundaries during compilation, removing shared interior
// surfaces without neighbor polygon clipping or per-cell geometry construction.
fn compound_triangles(parts: &[Primitive]) -> Vec<Triangle> {
    let mut out = Vec::new();
    for (index, primitive) in parts.iter().enumerate() {
        if !matches!(primitive, Primitive::Box(_)) {
            out.extend(primitive.triangles());
            continue;
        }
        let faces = primitive.triangles();
        for pair in faces.as_chunks::<2>().0 {
            let p0 = pair[0].positions[0];
            let ds = sub(pair[0].positions[1], p0);
            let dt = sub(pair[1].positions[2], p0);
            let normal = pair[0].normal();
            let sa = (0..3).find(|a| ds[*a] != 0.).unwrap();
            let ta = (0..3).find(|a| dt[*a] != 0.).unwrap();
            let mut ss = vec![0., 1.];
            let mut ts = vec![0., 1.];
            for part in parts {
                let b = part.bounds();
                for v in [b.min[sa], b.max[sa]] {
                    let s = (v - p0[sa]) / ds[sa];
                    if s > 0. && s < 1. {
                        ss.push(s);
                    }
                }
                for v in [b.min[ta], b.max[ta]] {
                    let t = (v - p0[ta]) / dt[ta];
                    if t > 0. && t < 1. {
                        ts.push(t);
                    }
                }
            }
            ss.sort_by(f32::total_cmp);
            ss.dedup();
            ts.sort_by(f32::total_cmp);
            ts.dedup();
            let point = |s: f32, t: f32| std::array::from_fn(|a| p0[a] + s * ds[a] + t * dt[a]);
            for s in ss.windows(2) {
                for t in ts.windows(2) {
                    let mid = point((s[0] + s[1]) * 0.5, (t[0] + t[1]) * 0.5);
                    let positions = [
                        point(s[0], t[0]),
                        point(s[1], t[0]),
                        point(s[1], t[1]),
                        point(s[0], t[1]),
                    ];
                    let hidden = parts.iter().enumerate().any(|(other, p)| {
                        if other == index {
                            return false;
                        }
                        let contains = |q: Point| p.planes().iter().all(|(n, d)| dot(*n, q) <= *d);
                        positions.iter().all(|position| {
                            contains(std::array::from_fn(|a| position[a] + normal[a] * 1e-5))
                        }) || (other < index
                            && positions.iter().all(|position| contains(*position))
                            && p.triangles().iter().any(|triangle| {
                                triangle.normal() == normal
                                    && dot(normal, sub(triangle.positions[0], mid)).abs() < 1e-6
                            }))
                    });
                    if hidden {
                        continue;
                    }
                    let uv = [
                        [s[0], 1. - t[0]],
                        [s[1], 1. - t[0]],
                        [s[1], 1. - t[1]],
                        [s[0], 1. - t[1]],
                    ];
                    for ix in [[0, 1, 2], [0, 2, 3]] {
                        out.push(Triangle {
                            positions: ix.map(|i| positions[i]),
                            uv: ix.map(|i| uv[i]),
                            face: pair[0].face,
                        });
                    }
                }
            }
        }
    }
    out
}
pub fn box_overlap(a: LocalBox, b: LocalBox) -> bool {
    (0..3).all(|i| a.min[i] < b.max[i] && a.max[i] > b.min[i])
}
fn triangle_ray(t: Triangle, o: Point, d: Point, reach: f32) -> Option<ShapeHit> {
    let e1 = sub(t.positions[1], t.positions[0]);
    let e2 = sub(t.positions[2], t.positions[0]);
    let h = cross(d, e2);
    let det = dot(e1, h);
    if det.abs() < 1e-8 {
        return None;
    }
    let s = sub(o, t.positions[0]);
    let u = dot(s, h) / det;
    let q = cross(s, e1);
    let v = dot(d, q) / det;
    let distance = dot(e2, q) / det;
    if u < 0. || v < 0. || u + v > 1. || distance < 0. || distance > reach {
        return None;
    }
    Some(ShapeHit {
        distance,
        normal: t.normal(),
    })
}
pub fn dot(a: Point, b: Point) -> f32 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
pub fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}
pub fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_primitives_winding_and_slope_queries() {
        let wedge = Primitive::Wedge(LocalBox::UNIT);
        let triangles = wedge.triangles();
        assert_eq!(triangles.len(), 8);
        assert!(triangles.iter().all(|t| t.valid()));
        let center = [0.5, 0.2, 0.7];
        for t in triangles {
            assert!(dot(t.normal(), sub(t.positions[0], center)) > 0.);
        }
        let shape = Shape::Compound(vec![wedge]);
        let hit = shape
            .ray([0.5, 2., 0.5], [0., -1., 0.], ModelRotation::IDENTITY, 3.)
            .unwrap();
        assert_eq!(hit.distance, 1.5);
        assert!((hit.normal[1] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        assert!(!shape.overlaps(
            LocalBox {
                min: [0.2, 0.6, 0.2],
                max: [0.4, 0.8, 0.4]
            },
            ModelRotation::IDENTITY
        ));
        assert!(shape.overlaps(
            LocalBox {
                min: [0.2, 0.1, 0.6],
                max: [0.4, 0.3, 0.8]
            },
            ModelRotation::IDENTITY
        ));
    }
    #[test]
    fn invalid_static_data_and_rotated_queries() {
        assert!(
            !LocalBox {
                min: [0.; 3],
                max: [f32::NAN, 1., 1.]
            }
            .valid()
        );
        assert!(!Shape::Compound(vec![]).valid(true));
        let shape = Shape::Compound(vec![Primitive::Wedge(LocalBox::UNIT)]);
        for facing in [
            crate::orientation::Facing::North,
            crate::orientation::Facing::East,
            crate::orientation::Facing::Up,
        ] {
            let rotation = ModelRotation::facing(facing);
            let hit = shape
                .ray(
                    rotation.point([0.5, 2., 0.5]),
                    rotation.transform([0., -1., 0.]),
                    rotation,
                    3.,
                )
                .unwrap();
            assert_eq!(hit.distance, 1.5);
        }
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    #[test]
    fn static_mesh_rejects_indices_bounds_uv_degeneracy_and_complexity() {
        let triangles = Primitive::Box(LocalBox::UNIT).triangles();
        let mesh = BoundedStaticMesh::from_triangles(&triangles).unwrap();
        assert!(mesh.valid());
        let mut invalid = mesh.clone();
        invalid.triangles[0].indices[0] = u16::MAX;
        assert!(!invalid.valid());
        let mut invalid = mesh.clone();
        invalid.vertices[0].position[0] = f32::INFINITY;
        assert!(!invalid.valid());
        let mut invalid = mesh.clone();
        invalid.vertices[0].uv[0] = -0.1;
        assert!(!invalid.valid());
        let mut invalid = mesh.clone();
        invalid.triangles[0].indices = [0; 3];
        assert!(!invalid.valid());
        let mut invalid = mesh;
        invalid.triangles[0].face = 6;
        assert!(!invalid.valid());
        assert!(
            BoundedStaticMesh::from_triangles(&vec![triangles[0]; MAX_MODEL_TRIANGLES + 1])
                .is_none()
        );
    }
    #[test]
    fn mixed_wedge_and_box_preserve_partly_exposed_box_face() {
        let model = RenderGeometry::Primitives(vec![
            Primitive::Wedge(LocalBox::UNIT),
            Primitive::Box(LocalBox {
                min: [0.; 3],
                max: [1., 0.5, 1.],
            }),
        ]);
        let triangles = model.triangles();
        assert!(valid_mesh(&triangles));
        let hit = Shape::Surface(triangles)
            .ray(
                [2., 0.25, 0.125],
                [-1., 0., 0.],
                ModelRotation::IDENTITY,
                2.,
            )
            .unwrap();
        assert_eq!(hit.distance, 1.);
        assert_eq!(hit.normal, [1., 0., 0.]);
    }
    #[test]
    fn compound_shared_faces_are_removed_before_runtime_and_transform_inverse_is_exact() {
        let model = RenderGeometry::Primitives(vec![
            Primitive::Box(LocalBox {
                min: [0.; 3],
                max: [1., 0.5, 1.],
            }),
            Primitive::Box(LocalBox {
                min: [0., 0.5, 0.],
                max: [1.; 3],
            }),
        ]);
        let triangles = model.triangles();
        assert!(valid_mesh(&triangles));
        assert!(
            triangles
                .iter()
                .all(|t| !(t.face == 4 || t.face == 5) || t.positions.iter().all(|p| p[1] != 0.5))
        );
        for facing in [
            crate::orientation::Facing::North,
            crate::orientation::Facing::East,
            crate::orientation::Facing::South,
            crate::orientation::Facing::West,
            crate::orientation::Facing::Up,
            crate::orientation::Facing::Down,
        ] {
            let rotation = ModelRotation::facing(facing);
            assert_eq!(
                rotation.compose(rotation.inverse()),
                ModelRotation::IDENTITY
            );
            assert_eq!(
                rotation.inverse().point(rotation.point([0.25, 0.5, 0.75])),
                [0.25, 0.5, 0.75]
            );
        }
    }
}
