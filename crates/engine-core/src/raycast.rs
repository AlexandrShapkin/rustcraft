//! Amanatides/Woo traversal; normalized distance, deterministic X/Y/Z tie order.
use crate::{BlockId, BlockPos, Vec3, World};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub block: BlockPos,
    pub adjacent: BlockPos,
    pub normal: [i32; 3],
    pub distance: f32,
}
pub fn cast(
    world: &World,
    eye: Vec3,
    direction: Vec3,
    reach: f32,
    hit: impl Fn(BlockId) -> bool,
) -> Option<RayHit> {
    let o = [eye.x, eye.y, eye.z];
    let mut d = [direction.x, direction.y, direction.z];
    let length = d.iter().map(|v| v * v).sum::<f32>().sqrt();
    if !length.is_finite()
        || length < 1e-8
        || !reach.is_finite()
        || reach < 0.0
        || !o.iter().all(|v| v.is_finite())
    {
        return None;
    }
    for v in &mut d {
        *v /= length;
    }
    let mut cell = o.map(|v| v.floor() as i32);
    let step = d.map(|v| {
        if v > 0.0 {
            1
        } else if v < 0.0 {
            -1
        } else {
            0
        }
    });
    let delta = d.map(|v| {
        if v == 0.0 {
            f32::INFINITY
        } else {
            1.0 / v.abs()
        }
    });
    let mut next: [f32; 3] = std::array::from_fn(|a| {
        if step[a] == 0 {
            f32::INFINITY
        } else {
            ((cell[a] + i32::from(step[a] > 0)) as f32 - o[a]) / d[a]
        }
    });
    let pos = |c: [i32; 3]| BlockPos {
        x: c[0],
        y: c[1],
        z: c[2],
    };
    let available = |c: [i32; 3]| {
        world.column_available(crate::ChunkPos {
            x: c[0].div_euclid(crate::CHUNK_SIZE),
            z: c[2].div_euclid(crate::CHUNK_SIZE),
        })
    };
    if !available(cell) {
        return None;
    }
    // Starting inside a target has no unambiguous placement face.
    if hit(world.get(pos(cell))) {
        return Some(RayHit {
            block: pos(cell),
            adjacent: pos(cell),
            normal: [0; 3],
            distance: 0.0,
        });
    }
    loop {
        let a = if next[0] <= next[1] && next[0] <= next[2] {
            0
        } else if next[1] <= next[2] {
            1
        } else {
            2
        };
        let distance = next[a];
        if distance > reach {
            return None;
        }
        let adjacent = pos(cell);
        cell[a] += step[a];
        next[a] += delta[a];
        if !available(cell) {
            return None;
        }
        if hit(world.get(pos(cell))) {
            let mut normal = [0; 3];
            normal[a] = -step[a];
            return Some(RayHit {
                block: pos(cell),
                adjacent,
                normal,
                distance,
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_faces_and_reach() {
        for x in [-17, -16, -1, 0, 15, 16] {
            let mut w = World::new(BlockId(0));
            let p = BlockPos { x, y: 16, z: -1 };
            w.set(p, BlockId(1));
            let eye = Vec3::new(x as f32 - 2.0, 16.5, -0.5);
            assert!(cast(&w, eye, Vec3::new(1., 0., 0.), 1.99, |b| b.0 != 0).is_none());
            let h = cast(&w, eye, Vec3::new(1., 0., 0.), 2., |b| b.0 != 0).unwrap();
            assert_eq!(h.block, p);
            assert_eq!(h.normal, [-1, 0, 0]);
            assert_eq!(h.adjacent.x, x - 1);
            assert!(cast(&w, eye, Vec3::new(-1., 0., 0.), 5., |b| b.0 != 0).is_none());
        }
    }
}

#[cfg(test)]
mod axis_tests {
    use super::*;
    #[test]
    fn all_axes_signed_sections_and_boundary_origins() {
        for axis in 0..3 {
            for sign in [-1, 1] {
                for boundary in [-16, 0, 16] {
                    let mut w = World::new(BlockId(0));
                    let mut target = [2, 2, 2];
                    target[axis] = boundary;
                    let p = BlockPos {
                        x: target[0],
                        y: target[1],
                        z: target[2],
                    };
                    w.set(p, BlockId(1));
                    let mut origin = target.map(|v| v as f32 + 0.5);
                    origin[axis] = boundary as f32 + if sign > 0 { -1. } else { 2. };
                    let eye = Vec3::new(origin[0], origin[1], origin[2]);
                    let mut d = [0.; 3];
                    d[axis] = sign as f32 * 3.;
                    let direction = Vec3::new(d[0], d[1], d[2]);
                    let hit = cast(&w, eye, direction, 1., |b| b.0 != 0).unwrap();
                    assert_eq!(hit.block, p);
                    assert_eq!(hit.normal[axis], -sign);
                    assert_eq!(hit.distance, 1.);
                    assert!(cast(&w, eye, direction, 0.999, |b| b.0 != 0).is_none());
                }
            }
        }
    }
    #[test]
    fn invalid_directions_inside_and_exact_negative_face() {
        let mut w = World::new(BlockId(0));
        w.set(BlockPos { x: -1, y: 0, z: 0 }, BlockId(1));
        let eye = Vec3::new(0., 0.5, 0.5);
        let h = cast(&w, eye, Vec3::new(-1., 0., 0.), 0., |b| b.0 == 1).unwrap();
        assert_eq!(h.normal, [1, 0, 0]);
        assert_eq!(h.distance, 0.);
        assert!(cast(&w, eye, Vec3::ZERO, 5., |_| true).is_none());
        assert!(cast(&w, eye, Vec3::new(f32::NAN, 1., 0.), 5., |_| true).is_none());
        let inside = cast(
            &w,
            Vec3::new(-0.5, 0.5, 0.5),
            Vec3::new(1., 0., 0.),
            5.,
            |b| b.0 == 1,
        )
        .unwrap();
        assert_eq!(inside.normal, [0; 3]);
    }
}
