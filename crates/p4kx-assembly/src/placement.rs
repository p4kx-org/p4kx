// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{Bone, Model, ModelFile, Transform, chained_world_poses};

/// Returns the transform of a port's offset from its helper: the rotation `rotation` in degrees,
/// about X first, then Y, then Z, and the translation `position` in metres.
pub(crate) fn offset(position: [f32; 3], rotation: [f32; 3]) -> Transform {
    let [x, y, z] = rotation.map(|degrees| f64::from(degrees).to_radians());
    let (sx, cx) = x.sin_cos();
    let (sy, cy) = y.sin_cos();
    let (sz, cz) = z.sin_cos();
    let rows = [
        [
            cy * cz,
            (sx * sy).mul_add(cz, -(cx * sz)),
            (cx * sy).mul_add(cz, sx * sz),
        ],
        [
            cy * sz,
            (sx * sy).mul_add(sz, cx * cz),
            (cx * sy).mul_add(sz, -(sx * cz)),
        ],
        [-sy, sx * cy, cx * cy],
    ];
    let [px, py, pz] = position;
    Transform {
        rows: [row(rows[0], px), row(rows[1], py), row(rows[2], pz)],
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "transforms are stored at single precision"
)]
const fn row([a, b, c]: [f64; 3], shift: f32) -> [f32; 4] {
    [a as f32, b as f32, c as f32, shift]
}

/// Returns the transform from the space of the node of `model` named `name`, ignoring ASCII
/// case, to the model's space, or `None` when the model has no such node.
pub(crate) fn node_transform(model: &Model, name: &str) -> Option<Transform> {
    node_placement(model, name).map(|placement| placement.transform)
}

#[derive(Clone, Copy)]
pub(crate) struct NodePlacement {
    pub(crate) transform: Transform,
    pub(crate) node: usize,
}

pub(crate) fn node_placement(model: &Model, name: &str) -> Option<NodePlacement> {
    let node = model
        .nodes
        .iter()
        .position(|node| node.name.eq_ignore_ascii_case(name))?;
    let mut transform = Transform::IDENTITY;
    for (_, ancestor) in model.node_ancestors(node) {
        transform = ancestor.transform.then(&transform);
        if ancestor.parent.is_none() {
            return Some(NodePlacement { transform, node });
        }
    }
    None
}

/// Returns whether the model has a level of detail near `level` and none of its meshes is skinned;
/// a level without meshes counts as unskinned.
pub(crate) fn is_unskinned(model: &Model, level: u32) -> bool {
    model
        .nearest_lod(level)
        .is_some_and(|lod| lod.meshes.iter().all(|mesh| mesh.influences.is_none()))
}

/// The item type prefix of the items a character wears.
const CHARACTER_PREFIX: &str = "Char_";

/// Returns whether `item_type` is the item type of a character item: a body, head, clothing,
/// armour or accessory.
pub(crate) fn is_character_item(item_type: &str) -> bool {
    item_type
        .get(..CHARACTER_PREFIX.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(CHARACTER_PREFIX))
}

/// Returns the transform from the bind-pose space of the bone of `model` named `name`, ignoring
/// ASCII case, to the model's space, or `None` when the model has no such bone or its pose is
/// not finite.
///
/// Only the bones of the first level of detail count, and of several bones with the same name the
/// first wins.
pub(crate) fn bone_transform(model: &Model, name: &str) -> Option<Transform> {
    bone_placement(model, name).map(|placement| placement.transform)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BoneSelection {
    pub(crate) file: ModelFile,
    pub(crate) ordinal: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct BonePlacement {
    pub(crate) transform: Transform,
    pub(crate) selection: BoneSelection,
}

pub(crate) fn bone_placement(model: &Model, name: &str) -> Option<BonePlacement> {
    let (file, bones) = model
        .lods
        .first()
        .and_then(|lod| Some((ModelFile::LodModel(lod.level), lod.bones.as_deref()?)))
        .unwrap_or((ModelFile::Model, model.bones.as_slice()));
    let (ordinal, transform) = named_bone_pose(bones, name)?;
    Some(BonePlacement {
        transform,
        selection: BoneSelection { file, ordinal },
    })
}

/// Returns the bind pose of the bone of `bones` named `name`, ignoring ASCII case, in the space of
/// the skeleton, or `None` when there is no such bone or its pose is not finite.
pub(crate) fn bone_pose(bones: &[Bone], name: &str) -> Option<Transform> {
    named_bone_pose(bones, name).map(|(_, transform)| transform)
}

fn named_bone_pose(bones: &[Bone], name: &str) -> Option<(usize, Transform)> {
    let at = bones
        .iter()
        .position(|bone| bone.name.eq_ignore_ascii_case(name))?;
    let pose = *chained_world_poses(bones).get(at)?;
    let [x, y, z, w] = pose.rotation.map(f64::from);
    let transform = Transform::from_quaternion([w, x, y, z], pose.translation.map(f64::from))?;
    Some((at, transform))
}

#[cfg(test)]
mod tests {
    use super::offset;

    fn close(actual: [f32; 4], expected: [f32; 4]) {
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-5, "{actual:?} against {expected:?}");
        }
    }

    #[test]
    fn a_quarter_turn_about_z_maps_x_to_y() {
        let transform = offset([1.0, 2.0, 3.0], [0.0, 0.0, 90.0]);
        close(transform.rows[0], [0.0, -1.0, 0.0, 1.0]);
        close(transform.rows[1], [1.0, 0.0, 0.0, 2.0]);
        close(transform.rows[2], [0.0, 0.0, 1.0, 3.0]);
    }

    /// The local axes of a part placed with `rotation`, as the columns of its rotation matrix.
    fn axes(rotation: [f32; 3]) -> [[f32; 3]; 3] {
        let rows = offset([0.0; 3], rotation).rows;
        [0, 1, 2].map(|column| rows.map(|row| row[column]))
    }

    fn assert_axis(actual: [f32; 3], expected: [f32; 3]) {
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-5, "{actual:?} against {expected:?}");
        }
    }

    #[test]
    fn a_multi_axis_rack_points_its_z_axis_forward_and_mirrors_its_twin() {
        let left = axes([90.0, 90.0, 180.0]);
        let right = axes([90.0, -90.0, 180.0]);
        assert_axis(left[2], [0.0, 1.0, 0.0]);
        assert_axis(right[2], [0.0, 1.0, 0.0]);
        assert_axis(left[1], [-1.0, 0.0, 0.0]);
        assert_axis(right[1], [1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_pitch_and_a_yaw_leave_the_right_axis_level() {
        let tilted = axes([30.0, 0.0, 90.0]);
        assert!(tilted[0][2].abs() < 1e-5, "{tilted:?}");
    }

    #[test]
    fn rotations_apply_x_before_y_before_z() {
        let transform = offset([0.0; 3], [90.0, 0.0, 90.0]);
        let up = transform.rows.map(|row| row[2]);
        for (actual, expected) in up.iter().zip([1.0, 0.0, 0.0]) {
            assert!((actual - expected).abs() < 1e-5, "{up:?}");
        }
    }
}
