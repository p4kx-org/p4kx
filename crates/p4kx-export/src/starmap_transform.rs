// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// The largest coordinate of a position that is kept, in metres: a light year is 9.5e15 m, far
/// beyond any star system, so a larger value is data that cannot be a place.
pub(crate) const MAX_COORDINATE: f64 = 1.0e15;

/// A rotation as a unit quaternion in the order `w, x, y, z`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rotation {
    w: f64,
    x: f64,
    y: f64,
    z: f64,
}

fn dot4(a: [f64; 4], b: [f64; 4]) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2].mul_add(b[2], a[3] * b[3])))
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1].mul_add(b[2], -(a[2] * b[1])),
        a[2].mul_add(b[0], -(a[0] * b[2])),
        a[0].mul_add(b[1], -(a[1] * b[0])),
    ]
}

impl Rotation {
    pub(crate) const IDENTITY: Self = Self {
        w: 1.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Returns the rotation of the quaternion `w, x, y, z` scaled to unit length, `None` for a
    /// quaternion with a non-finite part or without length.
    pub(crate) fn from_parts(parts: [f64; 4]) -> Option<Self> {
        let length = dot4(parts, parts).sqrt();
        let [w, x, y, z] = parts.map(|part| part / length);
        (length.is_finite() && length > f64::EPSILON).then_some(Self { w, x, y, z })
    }

    pub(crate) const fn parts(self) -> [f64; 4] {
        [self.w, self.x, self.y, self.z]
    }

    /// Returns the rotation that applies `inner` first and `self` after it.
    pub(crate) fn then_after(self, inner: Self) -> Self {
        let outer = self.parts();
        let [iw, ix, iy, iz] = inner.parts();
        Self {
            w: dot4(outer, [iw, -ix, -iy, -iz]),
            x: dot4(outer, [ix, iw, iz, -iy]),
            y: dot4(outer, [iy, -iz, iw, ix]),
            z: dot4(outer, [iz, iy, -ix, iw]),
        }
    }

    /// Returns `vector` rotated.
    pub(crate) fn rotate(self, vector: [f64; 3]) -> [f64; 3] {
        let axis = [self.x, self.y, self.z];
        let twice = cross(axis, vector).map(|part| 2.0 * part);
        let [cx, cy, cz] = cross(axis, twice);
        let [tx, ty, tz] = twice;
        let [vx, vy, vz] = vector;
        [
            self.w.mul_add(tx, vx) + cx,
            self.w.mul_add(ty, vy) + cy,
            self.w.mul_add(tz, vz) + cz,
        ]
    }
}

/// A position and a rotation in the frame of a container.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Transform {
    pub(crate) position: [f64; 3],
    pub(crate) rotation: Rotation,
}

impl Transform {
    pub(crate) const IDENTITY: Self = Self {
        position: [0.0; 3],
        rotation: Rotation::IDENTITY,
    };

    /// Returns whether every coordinate is a finite number of at most [`MAX_COORDINATE`] metres.
    pub(crate) fn is_sane(self) -> bool {
        self.position
            .iter()
            .all(|coordinate| coordinate.abs() <= MAX_COORDINATE)
    }

    /// Returns the transform of a child placed at `position` with `rotation` in the frame of
    /// `self`: its position is rotated by the rotation of `self` before it is added, and the
    /// rotations multiply.
    pub(crate) fn place(self, position: [f64; 3], rotation: Rotation) -> Self {
        let [ox, oy, oz] = self.rotation.rotate(position);
        let [px, py, pz] = self.position;
        Self {
            position: [px + ox, py + oy, pz + oz],
            rotation: self.rotation.then_after(rotation),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_1_SQRT_2;

    use super::{MAX_COORDINATE, Rotation, Transform};

    const QUARTER_TURN_Z: [f64; 4] = [FRAC_1_SQRT_2, 0.0, 0.0, FRAC_1_SQRT_2];

    fn near(have: [f64; 3], want: [f64; 3]) {
        for (have, want) in have.into_iter().zip(want) {
            assert!((have - want).abs() < 1e-9, "{have:?} != {want:?}");
        }
    }

    #[test]
    fn a_quarter_turn_about_z_takes_x_to_y() {
        let turn = Rotation::from_parts(QUARTER_TURN_Z).expect("a quaternion");
        near(turn.rotate([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
        near(turn.rotate([0.0, 0.0, 5.0]), [0.0, 0.0, 5.0]);
    }

    #[test]
    fn a_child_position_is_rotated_by_its_container() {
        let turn = Rotation::from_parts(QUARTER_TURN_Z).expect("a quaternion");
        let container = Transform::IDENTITY.place([10.0, 20.0, 30.0], turn);
        let child = container.place([1.0, 2.0, 3.0], Rotation::IDENTITY);
        near(child.position, [8.0, 21.0, 33.0]);
    }

    #[test]
    fn rotations_compose_from_the_outside_in() {
        let turn = Rotation::from_parts(QUARTER_TURN_Z).expect("a quaternion");
        let twice = Transform::IDENTITY
            .place([0.0; 3], turn)
            .place([0.0; 3], turn)
            .place([1.0, 0.0, 0.0], Rotation::IDENTITY);
        near(twice.position, [-1.0, 0.0, 0.0]);
    }

    #[test]
    fn rotations_do_not_commute() {
        let about_x =
            Rotation::from_parts([FRAC_1_SQRT_2, FRAC_1_SQRT_2, 0.0, 0.0]).expect("a quaternion");
        let about_z = Rotation::from_parts(QUARTER_TURN_Z).expect("a quaternion");
        let x_then_z = about_z.then_after(about_x).rotate([0.0, 1.0, 0.0]);
        let z_then_x = about_x.then_after(about_z).rotate([0.0, 1.0, 0.0]);
        near(x_then_z, [0.0, 0.0, 1.0]);
        near(z_then_x, [-1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_position_beyond_the_limit_or_not_a_number_is_not_sane() {
        let at = |x: f64| Transform::IDENTITY.place([x, 0.0, 0.0], Rotation::IDENTITY);
        assert!(at(MAX_COORDINATE).is_sane(), "exactly the limit");
        assert!(!at(MAX_COORDINATE * 2.0).is_sane(), "beyond the limit");
        assert!(
            !at(1.0e308)
                .place([1.0e308; 3], Rotation::IDENTITY)
                .is_sane(),
            "overflow to infinity"
        );
        let turned = Rotation::from_parts(QUARTER_TURN_Z).expect("a quaternion");
        let huge = Transform::IDENTITY
            .place([0.0; 3], turned)
            .place([f64::MAX; 3], turned);
        assert!(
            !huge.is_sane(),
            "a huge vector gives infinity or not a number"
        );
    }

    #[test]
    fn a_quaternion_without_length_or_with_a_non_finite_part_is_refused() {
        assert!(Rotation::from_parts([0.0; 4]).is_none(), "no length");
        assert!(
            Rotation::from_parts([f64::NAN, 0.0, 0.0, 1.0]).is_none(),
            "not a number"
        );
        assert!(
            Rotation::from_parts([f64::INFINITY, 0.0, 0.0, 0.0]).is_none(),
            "infinite"
        );
    }

    #[test]
    fn a_quaternion_is_scaled_to_unit_length() {
        let doubled = Rotation::from_parts([2.0 * FRAC_1_SQRT_2, 0.0, 0.0, 2.0 * FRAC_1_SQRT_2])
            .expect("a quaternion");
        near(doubled.rotate([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
    }
}
