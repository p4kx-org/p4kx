// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{Bone, Model, Transform, chained_world_poses};

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
    let mut at = model
        .nodes
        .iter()
        .position(|node| node.name.eq_ignore_ascii_case(name))?;
    let mut transform = Transform::IDENTITY;
    for _ in 0..=model.nodes.len() {
        let node = model.nodes.get(at)?;
        transform = node.transform.then(&transform);
        match node.parent {
            Some(parent) => at = parent,
            None => return Some(transform),
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
    let bones = model
        .lods
        .first()
        .map_or(model.bones.as_slice(), |lod| model.bones_of(lod));
    bone_pose(bones, name)
}

/// Returns the bind pose of the bone of `bones` named `name`, ignoring ASCII case, in the space of
/// the skeleton, or `None` when there is no such bone or its pose is not finite.
pub(crate) fn bone_pose(bones: &[Bone], name: &str) -> Option<Transform> {
    let at = bones
        .iter()
        .position(|bone| bone.name.eq_ignore_ascii_case(name))?;
    let pose = *chained_world_poses(bones).get(at)?;
    let [x, y, z, w] = pose.rotation.map(f64::from);
    Transform::from_quaternion([w, x, y, z], pose.translation.map(f64::from))
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

    #[test]
    fn rotations_apply_x_before_y_before_z() {
        let transform = offset([0.0; 3], [90.0, 0.0, 90.0]);
        let up = transform.rows.map(|row| row[2]);
        for (actual, expected) in up.iter().zip([1.0, 0.0, 0.0]) {
            assert!((actual - expected).abs() < 1e-5, "{up:?}");
        }
    }
}
