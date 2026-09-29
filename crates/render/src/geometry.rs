//! Canonical full-cube geometry: [0,1]^3, outward CCW, top-left image UVs.
//! See reference/notes/features/m3-exact-item-ui-rendering.md (M3.8).
use crate::Face;
#[derive(Clone, Copy, Debug)]
pub struct FaceDefinition {
    pub direction: Face,
    pub positions: [[f32; 3]; 4],
    pub uv_corners: [[f32; 2]; 4],
    pub indices: [usize; 6],
    pub normal: [f32; 3],
}
const fn face(direction: Face, positions: [[f32; 3]; 4], normal: [f32; 3]) -> FaceDefinition {
    FaceDefinition {
        direction,
        positions,
        uv_corners: [[0., 1.], [1., 1.], [1., 0.], [0., 0.]],
        indices: [0, 1, 2, 0, 2, 3],
        normal,
    }
}
pub const FACES: [FaceDefinition; 6] = [
    face(
        Face::North,
        [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]],
        [0., 0., 1.],
    ),
    face(
        Face::South,
        [[1., 0., 0.], [0., 0., 0.], [0., 1., 0.], [1., 1., 0.]],
        [0., 0., -1.],
    ),
    face(
        Face::East,
        [[1., 0., 1.], [1., 0., 0.], [1., 1., 0.], [1., 1., 1.]],
        [1., 0., 0.],
    ),
    face(
        Face::West,
        [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]],
        [-1., 0., 0.],
    ),
    face(
        Face::Top,
        [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]],
        [0., 1., 0.],
    ),
    face(
        Face::Bottom,
        [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
        [0., -1., 0.],
    ),
];
pub fn definition(direction: Face) -> &'static FaceDefinition {
    &FACES[direction as usize]
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|i| a[i] - b[i])
    }
    fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }
    #[test]
    fn every_quad_is_planar_outward_and_uv_affine_not_bow_tied() {
        for f in FACES {
            let p = f.positions;
            let uv = f.uv_corners;
            for (axis, _) in p[0].iter().enumerate() {
                assert_eq!(
                    p[0][axis] + p[2][axis],
                    p[1][axis] + p[3][axis],
                    "{:?}",
                    f.direction
                );
            }
            for (axis, _) in uv[0].iter().enumerate() {
                assert_eq!(
                    uv[0][axis] + uv[2][axis],
                    uv[1][axis] + uv[3][axis],
                    "{:?} UV bow tie",
                    f.direction
                );
            }
            for tri in f.indices.as_chunks::<3>().0 {
                assert_eq!(
                    cross(sub(p[tri[1]], p[tri[0]]), sub(p[tri[2]], p[tri[0]])),
                    f.normal
                );
                let a = uv[tri[0]];
                let b = uv[tri[1]];
                let c = uv[tri[2]];
                assert_eq!(
                    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]),
                    -1.
                );
            }
            for q in p {
                assert_eq!(
                    sub(q, p[0])
                        .iter()
                        .zip(f.normal)
                        .map(|(a, b)| a * b)
                        .sum::<f32>(),
                    0.
                );
            }
            if f.normal[1] == 0. {
                for (p, uv) in p.into_iter().zip(uv) {
                    assert_eq!(uv[1], 1. - p[1]);
                }
            }
        }
    }
}
