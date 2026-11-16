// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::rig::SLOT_VARIANT_DELTAS;
use crate::{DeltaSet, DnaMatrix, DnaRig, RigError, RigLogic};

const WHAT: &str = "head delta records";
const AXES: usize = 3;

/// One output channel of the rig: the blendshapes that move it and their summed coefficients.
type Row = Vec<(u16, f32)>;

/// What the rig makes of the values of its controls at one moment.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FacePose {
    /// The activation of every blendshape, the controls first, then the correctives.
    pub blendshapes: Vec<f32>,
    /// How far each joint of the rig has moved from its bind pose, in metres, relative to its
    /// parent joint, in the rig's axes.
    pub translations: Vec<[f32; 3]>,
    /// The rotation each joint is turned by from its bind pose, as a unit quaternion
    /// `x, y, z, w` from the Euler angles of the secondary delta records (`qx * qy * qz`). It is
    /// multiplied onto the bind rotation on the right, `bind * delta`, so it turns in the
    /// joint's own axes.
    pub rotations: Vec<[f32; 4]>,
}

/// The rig logic of a head for one Character: how the values of the controls of a face animation
/// become the activations of the blendshapes and the motion of the facial joints.
///
/// The delta records of the heads the Character's DNA blends, mixed by the DNA and the joint
/// weights of the rig, are summed to one coefficient per joint axis and blendshape; what the heads
/// leave of 1 in a region belongs to the base head, as for the expression blendshapes. Built by
/// [`FaceRig::new`], evaluated by [`FaceRig::pose`].
#[derive(Debug, Clone)]
pub struct FaceRig {
    logic: RigLogic,
    joints: usize,
    translations: Vec<Row>,
    rotations: Vec<Row>,
}

impl FaceRig {
    /// Prepares the rig logic of the Character `matrix` stands for.
    ///
    /// `logic` is the `RigLogic` chunk of the head skin; its blendshapes must be the ones the
    /// delta records of `rig` count.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::MatrixShape`] for a matrix of another rig, the errors of
    /// [`DnaRig::variant_deltas`] and [`DnaRig::joint_weights`], and [`RigError::Malformed`] when
    /// a record moves a joint the rig lacks or names a blendshape beyond `logic`'s.
    pub fn new(rig: &DnaRig<'_>, matrix: &DnaMatrix, logic: &RigLogic) -> Result<Self, RigError> {
        matrix.check(rig)?;
        let deltas = rig.variant_deltas()?;
        let weights = rig.joint_weights()?;
        let joints = weights.joints();
        let blendshapes = logic.blendshapes();
        let heads = head_weights(matrix);
        let mut rows = [
            vec![Vec::<(u16, f64)>::new(); joints * AXES],
            vec![Vec::<(u16, f64)>::new(); joints * AXES],
        ];
        for (variant, region_weights) in &heads {
            let folds: Vec<f32> = (0..joints)
                .map(|joint| {
                    region_weights
                        .iter()
                        .enumerate()
                        .map(|(region, &weight)| {
                            weights.weight(joint, region).unwrap_or_default() * weight
                        })
                        .sum()
                })
                .collect();
            let header = deltas
                .header(*variant)
                .ok_or_else(|| malformed("the head is outside the table"))?;
            for (set, set_rows) in [DeltaSet::Main, DeltaSet::Secondary]
                .into_iter()
                .zip(&mut rows)
            {
                let step = header.step(set);
                let records = deltas.records(*variant, set)?;
                add_records(records, (step, &folds), set_rows, blendshapes)?;
            }
        }
        Ok(Self::assembled(logic, joints, rows))
    }

    /// Prepares the rig logic of the head a `RigLogic` rig carries its own delta records for, as
    /// a story character's `.RigLogic` file does; there is no DNA to mix, so the records apply
    /// as they are and the joints keep their bind poses from the head's skin.
    ///
    /// # Errors
    ///
    /// Returns [`RigError::LogicWithoutDeltas`] when the rig carries no delta records.
    pub fn from_logic(logic: &RigLogic) -> Result<Self, RigError> {
        let joints = logic.joint_hashes().len();
        let folds = vec![1.0_f32; joints];
        let blendshapes = logic.blendshapes();
        let mut rows = [
            vec![Vec::<(u16, f64)>::new(); joints * AXES],
            vec![Vec::<(u16, f64)>::new(); joints * AXES],
        ];
        for (set, set_rows) in [DeltaSet::Main, DeltaSet::Secondary]
            .into_iter()
            .zip(&mut rows)
        {
            let (Some(records), Some(step)) = (logic.joint_records(set), logic.joint_step(set))
            else {
                return Err(RigError::LogicWithoutDeltas);
            };
            add_records(records, (step, &folds), set_rows, blendshapes)?;
        }
        Ok(Self::assembled(logic, joints, rows))
    }

    fn assembled(logic: &RigLogic, joints: usize, rows: [Vec<Vec<(u16, f64)>>; 2]) -> Self {
        let [translations, rotations] = rows.map(|set| set.into_iter().map(merged).collect());
        Self {
            logic: logic.clone(),
            joints,
            translations,
            rotations,
        }
    }

    /// Returns the rig logic chunk the rig was made with.
    #[must_use]
    pub const fn logic(&self) -> &RigLogic {
        &self.logic
    }

    /// Returns the number of joints the rig moves.
    #[must_use]
    pub const fn joints(&self) -> usize {
        self.joints
    }

    /// Returns the number of non-zero coefficients, which is the work of one evaluation.
    #[must_use]
    pub fn coefficients(&self) -> usize {
        self.translations
            .iter()
            .chain(&self.rotations)
            .map(Vec::len)
            .sum()
    }

    /// Evaluates the rig for the values of the controls, see [`RigLogic::activations`].
    #[must_use]
    pub fn pose(&self, controls: &[f32]) -> FacePose {
        let mut pose = FacePose::default();
        self.pose_into(controls, &mut pose);
        pose
    }

    /// Evaluates the rig into `pose`, reusing its buffers.
    pub fn pose_into(&self, controls: &[f32], pose: &mut FacePose) {
        self.fill_pose(controls, pose, |_| {});
    }

    /// Evaluates one pose and visits its parent-local Euler radians in joint order.
    #[cfg(feature = "verification")]
    #[must_use]
    pub fn pose_observing_angles(
        &self,
        controls: &[f32],
        observe: impl FnMut([f32; 3]),
    ) -> FacePose {
        let mut pose = FacePose::default();
        self.fill_pose(controls, &mut pose, observe);
        pose
    }

    fn fill_pose(&self, controls: &[f32], pose: &mut FacePose, mut observe: impl FnMut([f32; 3])) {
        pose.blendshapes.resize(self.logic.blendshapes(), 0.0);
        self.logic.activations_into(controls, &mut pose.blendshapes);
        pose.translations.clear();
        pose.rotations.clear();
        for joint in 0..self.joints {
            let slice = joint * AXES..(joint + 1) * AXES;
            let moved = sum(&self.translations, slice.clone(), &pose.blendshapes);
            let turned = sum(&self.rotations, slice, &pose.blendshapes);
            pose.translations.push(moved);
            observe(turned);
            pose.rotations.push(euler_quaternion(turned));
        }
    }
}

const fn malformed(reason: &'static str) -> RigError {
    RigError::Malformed {
        what: WHAT,
        slot: SLOT_VARIANT_DELTAS,
        reason,
    }
}

/// Adds the entries of `records`, scaled by `step` and the fold of their joint, to `rows`.
fn add_records(
    records: crate::DeltaRecords<'_>,
    (step, folds): (f64, &[f32]),
    rows: &mut [Vec<(u16, f64)>],
    blendshapes: usize,
) -> Result<(), RigError> {
    for record in records {
        let record = record?;
        let output = usize::from(record.output);
        let fold = folds.get(output / AXES).copied().unwrap_or_default();
        let row = rows
            .get_mut(output)
            .ok_or_else(|| malformed("a record moves a joint the rig lacks"))?;
        if fold == 0.0 {
            continue;
        }
        for entry in record.entries() {
            if usize::from(entry.blendshape) >= blendshapes {
                return Err(malformed("a record names a blendshape the rig lacks"));
            }
            row.push((
                entry.blendshape,
                f64::from(entry.coefficient) * step * f64::from(fold),
            ));
        }
    }
    Ok(())
}

fn sum(rows: &[Row], axes: std::ops::Range<usize>, activations: &[f32]) -> [f32; 3] {
    let mut total = [0.0_f32; AXES];
    for (value, row) in total.iter_mut().zip(rows.get(axes).unwrap_or_default()) {
        *value = row
            .iter()
            .map(|&(blendshape, coefficient)| {
                coefficient
                    * activations
                        .get(usize::from(blendshape))
                        .copied()
                        .unwrap_or_default()
            })
            .sum();
    }
    total
}

/// Returns the heads that move the joints with their weight in each region: the Character's heads
/// and the base head with what they leave of 1.
fn head_weights(matrix: &DnaMatrix) -> Vec<(usize, Vec<f32>)> {
    let regions = matrix.regions();
    let mut base = vec![1.0_f32; regions];
    let mut heads = Vec::new();
    for variant in 1..matrix.variants() {
        let weights: Vec<f32> = (0..regions)
            .map(|region| matrix.cell(variant, region))
            .collect();
        if weights.iter().all(|&weight| weight <= 0.0) {
            continue;
        }
        for (rest, weight) in base.iter_mut().zip(&weights) {
            *rest -= weight;
        }
        heads.push((variant, weights));
    }
    for rest in &mut base {
        *rest = rest.max(0.0);
    }
    heads.insert(0, (0, base));
    heads
        .into_iter()
        .filter(|(_, weights)| weights.iter().any(|&weight| weight > 0.0))
        .collect()
}

/// Sorts a row by blendshape and adds the coefficients of equal blendshapes.
fn merged(mut row: Vec<(u16, f64)>) -> Row {
    row.sort_unstable_by_key(|&(blendshape, _)| blendshape);
    let mut out: Row = Vec::with_capacity(row.len());
    let mut last = None;
    let mut total = 0.0_f64;
    for (blendshape, coefficient) in row {
        if last.is_some_and(|last| last != blendshape) {
            push(&mut out, last, total);
            total = 0.0;
        }
        last = Some(blendshape);
        total += coefficient;
    }
    push(&mut out, last, total);
    out
}

fn push(out: &mut Row, blendshape: Option<u16>, total: f64) {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the coefficients are stored as f32 and are far from its range"
    )]
    let coefficient = total as f32;
    if let Some(blendshape) = blendshape
        && coefficient != 0.0
        && coefficient.is_finite()
    {
        out.push((blendshape, coefficient));
    }
}

/// Returns the quaternion `x, y, z, w` of the Euler angles `x, y, z` in radians as the Hamilton
/// product `qx * qy * qz` of the half-angle quaternions about the axes, `qx` leftmost.
///
/// The order and the signs are those `StarBreaker` documents for the engine (`euler_xyz_to_quat`);
/// they are not verified against the game.
fn euler_quaternion([x, y, z]: [f32; 3]) -> [f32; 4] {
    let (sx, cx) = (x * 0.5).sin_cos();
    let (sy, cy) = (y * 0.5).sin_cos();
    let (sz, cz) = (z * 0.5).sin_cos();
    [
        (sx * cy).mul_add(cz, cx * sy * sz),
        (cx * sy).mul_add(cz, -(sx * cy * sz)),
        (cx * cy).mul_add(sz, sx * sy * cz),
        (cx * cy).mul_add(cz, -(sx * sy * sz)),
    ]
}

#[cfg(test)]
mod tests {
    use super::euler_quaternion;

    #[test]
    fn two_axes_are_the_product_of_x_then_y_with_x_leftmost() {
        let (x, y) = (0.6_f32, 0.8_f32);
        let [qx, qy, qz, qw] = euler_quaternion([x, y, 0.0]);
        let (sx, cx) = (x * 0.5).sin_cos();
        let (sy, cy) = (y * 0.5).sin_cos();
        let expected = [sx * cy, cx * sy, sx * sy, cx * cy];
        for (actual, expected) in [qx, qy, qz, qw].into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 1e-6,
                "{actual} against {expected}"
            );
        }
    }

    #[test]
    fn a_single_axis_turns_about_it_and_the_result_is_a_unit() {
        let [x, y, z, w] = euler_quaternion([0.0, 0.0, 1.0]);
        assert!(x.abs() < 1e-7 && y.abs() < 1e-7);
        assert!((z - 0.5_f32.sin()).abs() < 1e-6 && (w - 0.5_f32.cos()).abs() < 1e-6);
        let [x, y, z, w] = euler_quaternion([0.3, -0.4, 0.5]);
        let length: f32 = [x, y, z, w].iter().map(|value| value * value).sum();
        assert!((length - 1.0).abs() < 1e-6);
    }
}
