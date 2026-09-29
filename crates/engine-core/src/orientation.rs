//! Compact semantic orientation, independent of historical metadata and rendering APIs.
use crate::BlockState;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u16)]
pub enum Facing {
    #[default]
    North,
    East,
    South,
    West,
    Up,
    Down,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u16)]
pub enum Axis {
    X,
    #[default]
    Y,
    Z,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u16)]
pub enum HorizontalRotation {
    #[default]
    R0,
    R90,
    R180,
    R270,
}
/// A definition declares which orientation it uses. Bits are not a Beta metadata byte.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OrientationProperty {
    #[default]
    None,
    Facing,
    Axis,
    HorizontalRotation,
}
/// Proper orthogonal signed basis; multiplication uses column vectors, no reflection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelRotation([[i8; 3]; 3]);
impl Default for ModelRotation {
    fn default() -> Self {
        Self::IDENTITY
    }
}
impl ModelRotation {
    pub const IDENTITY: Self = Self([[1, 0, 0], [0, 1, 0], [0, 0, 1]]);
    pub fn horizontal(r: HorizontalRotation) -> Self {
        let quarter = Self([[0, 0, -1], [0, 1, 0], [1, 0, 0]]);
        let mut result = Self::IDENTITY;
        for _ in 0..r as u16 {
            result = quarter.compose(result);
        }
        result
    }
    /// Canonical front is -Z, up is +Y. North=-Z, East=+X.
    pub fn facing(f: Facing) -> Self {
        match f {
            Facing::North => Self::IDENTITY,
            Facing::East => Self::horizontal(HorizontalRotation::R90),
            Facing::South => Self::horizontal(HorizontalRotation::R180),
            Facing::West => Self::horizontal(HorizontalRotation::R270),
            Facing::Up => Self([[1, 0, 0], [0, 0, -1], [0, 1, 0]]),
            Facing::Down => Self([[1, 0, 0], [0, 0, 1], [0, -1, 0]]),
        }
    }
    pub fn axis(a: Axis) -> Self {
        match a {
            Axis::Y => Self::IDENTITY,
            Axis::X => Self([[0, 1, 0], [-1, 0, 0], [0, 0, 1]]),
            Axis::Z => Self([[1, 0, 0], [0, 0, -1], [0, 1, 0]]),
        }
    }
    /// self * local: applies local first, then self. Geometry carries UVs unchanged.
    pub fn compose(self, local: Self) -> Self {
        Self(std::array::from_fn(|r| {
            std::array::from_fn(|c| (0..3).map(|i| self.0[r][i] * local.0[i][c]).sum())
        }))
    }
    pub fn transform(self, v: [f32; 3]) -> [f32; 3] {
        self.0
            .map(|row| row.into_iter().zip(v).map(|(a, b)| f32::from(a) * b).sum())
    }
    pub fn point(self, p: [f32; 3]) -> [f32; 3] {
        self.transform(p.map(|v| v - 0.5)).map(|v| v + 0.5)
    }
}
impl BlockState {
    pub const fn new(block: crate::BlockId) -> Self {
        Self { block, variant: 0 }
    }
    pub fn with_facing(mut self, f: Facing) -> Self {
        self.variant = (self.variant & !7) | f as u16;
        self
    }
    pub fn facing(self) -> Facing {
        match self.variant & 7 {
            1 => Facing::East,
            2 => Facing::South,
            3 => Facing::West,
            4 => Facing::Up,
            5 => Facing::Down,
            _ => Facing::North,
        }
    }
    pub fn with_axis(mut self, a: Axis) -> Self {
        let bits = match a {
            Axis::Y => 0,
            Axis::X => 1,
            Axis::Z => 2,
        };
        self.variant = (self.variant & !(3 << 3)) | (bits << 3);
        self
    }
    pub fn axis(self) -> Axis {
        match (self.variant >> 3) & 3 {
            1 => Axis::X,
            2 => Axis::Z,
            _ => Axis::Y,
        }
    }
    pub fn with_rotation(mut self, r: HorizontalRotation) -> Self {
        self.variant = (self.variant & !(3 << 5)) | ((r as u16) << 5);
        self
    }
    pub fn rotation(self) -> HorizontalRotation {
        match (self.variant >> 5) & 3 {
            1 => HorizontalRotation::R90,
            2 => HorizontalRotation::R180,
            3 => HorizontalRotation::R270,
            _ => HorizontalRotation::R0,
        }
    }
    pub fn model_rotation(self, property: OrientationProperty) -> ModelRotation {
        match property {
            OrientationProperty::None => ModelRotation::IDENTITY,
            OrientationProperty::Facing => ModelRotation::facing(self.facing()),
            OrientationProperty::Axis => ModelRotation::axis(self.axis()),
            OrientationProperty::HorizontalRotation => ModelRotation::horizontal(self.rotation()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockId, BlockPos, World};
    #[test]
    fn compact_semantic_state_survives_world_storage() {
        assert_eq!(std::mem::size_of::<BlockState>(), 8);
        let state = BlockState::new(BlockId(4));
        assert_eq!(state.facing(), Facing::North);
        assert_eq!(state.axis(), Axis::Y);
        assert_eq!(state.rotation(), HorizontalRotation::R0);
        let state = state
            .with_facing(Facing::West)
            .with_axis(Axis::X)
            .with_rotation(HorizontalRotation::R270);
        assert_eq!(state, state.with_facing(Facing::West));
        let mut world = World::new(BlockId(0));
        let p = BlockPos {
            x: -17,
            y: 32,
            z: 16,
        };
        world.set_state(p, state);
        assert_eq!(world.state(p), state);
    }
    #[test]
    fn six_facings_and_horizontal_rotations_are_proper_and_composable() {
        for (f, expected) in [
            (Facing::North, [0., 0., -1.]),
            (Facing::East, [1., 0., 0.]),
            (Facing::South, [0., 0., 1.]),
            (Facing::West, [-1., 0., 0.]),
            (Facing::Up, [0., 1., 0.]),
            (Facing::Down, [0., -1., 0.]),
        ] {
            let r = ModelRotation::facing(f);
            assert_eq!(r.transform([0., 0., -1.]), expected);
            let x = r.transform([1., 0., 0.]);
            let y = r.transform([0., 1., 0.]);
            let z = r.transform([0., 0., 1.]);
            assert_eq!(
                [
                    x[1] * y[2] - x[2] * y[1],
                    x[2] * y[0] - x[0] * y[2],
                    x[0] * y[1] - x[1] * y[0]
                ],
                z
            );
            assert_eq!(r.point([0.5; 3]), [0.5; 3]);
        }
        let a = ModelRotation::facing(Facing::Up);
        let b = ModelRotation::horizontal(HorizontalRotation::R90);
        let v = [0.2, 0.4, 0.6];
        assert_eq!(a.compose(b).transform(v), a.transform(b.transform(v)));
        assert_eq!(b.compose(b).compose(b).compose(b), ModelRotation::IDENTITY);
    }
}
