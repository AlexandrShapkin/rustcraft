//! Amanatides/Woo traversal; normalized distance, deterministic X/Y/Z tie order.
use crate::{
    BlockId, BlockPos, BlockState, Vec3, World,
    orientation::ModelRotation,
    shape::{Shape, ShapeHit},
};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub block: BlockPos,
    pub adjacent: BlockPos,
    pub normal: [i32; 3],
    pub distance: f32,
    pub point: [f32; 3],
    pub local_point: [f32; 3],
    pub surface_normal: [f32; 3],
}
pub fn cast(
    world: &World,
    eye: Vec3,
    direction: Vec3,
    reach: f32,
    hit: impl Fn(BlockId) -> bool,
) -> Option<RayHit> {
    cast_shapes(world, eye, direction, reach, |state, o, d, reach| {
        if hit(state.block) {
            Shape::FullCube.ray(o, d, ModelRotation::IDENTITY, reach)
        } else {
            None
        }
    })
}
/// DDA remains the broad phase; the callback intersects one cell's selection shape.
pub fn cast_shapes(
    world: &World,
    eye: Vec3,
    direction: Vec3,
    reach: f32,
    intersect: impl Fn(BlockState, [f32; 3], [f32; 3], f32) -> Option<ShapeHit>,
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
    let mut entered = 0.;
    loop {
        let end = next.into_iter().fold(reach, f32::min);
        let local = std::array::from_fn(|i| o[i] - cell[i] as f32);
        if let Some(hit) = intersect(world.state(pos(cell)), local, d, reach)
            && hit.distance + 1e-5 >= entered
            && hit.distance <= end + 1e-5
        {
            let normal = if hit.normal == [0.; 3] {
                [0; 3]
            } else {
                let a = (0..3)
                    .max_by(|a, b| {
                        hit.normal[*a]
                            .abs()
                            .total_cmp(&hit.normal[*b].abs())
                            .then_with(|| b.cmp(a))
                    })
                    .unwrap();
                let mut n = [0; 3];
                n[a] = if hit.normal[a] > 0. { 1 } else { -1 };
                n
            };
            return Some(RayHit {
                block: pos(cell),
                adjacent: pos(std::array::from_fn(|i| cell[i] + normal[i])),
                normal,
                distance: hit.distance,
                point: std::array::from_fn(|i| o[i] + d[i] * hit.distance),
                local_point: std::array::from_fn(|i| local[i] + d[i] * hit.distance),
                surface_normal: hit.normal,
            });
        }
        let a = if next[0] <= next[1] && next[0] <= next[2] {
            0
        } else if next[1] <= next[2] {
            1
        } else {
            2
        };
        entered = next[a];
        if entered > reach {
            return None;
        }
        cell[a] += step[a];
        next[a] += delta[a];
        if !available(cell) {
            return None;
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

#[cfg(test)]
mod oblique_tests {
    use super::*;
    #[test]
    fn oblique_cell_entry_rounding_never_skips_the_nearest_surface() {
        let mut world = World::new(BlockId(0));
        world.fill_box(
            BlockPos {
                x: -2,
                y: -1,
                z: -2,
            },
            BlockPos { x: 2, y: 0, z: 8 },
            BlockId(1),
        );
        let eye = Vec3::new(0.5, 2.62, 0.5);
        let direction = Vec3::new(0., -0.9_f32.sin(), 0.9_f32.cos());
        for _ in 0..2 {
            let hit = cast(&world, eye, direction, 8., |id| id.0 != 0).unwrap();
            assert_eq!(world.get(hit.adjacent), BlockId(0));
            world.set(hit.block, BlockId(0));
        }
    }
}
