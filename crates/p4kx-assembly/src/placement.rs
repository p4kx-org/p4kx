// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{Bone, Lod, Model, ModelFile, Transform, chained_world_poses};

use crate::{IssueKind, ItemOffsetDiagnostic};

/// Converts an authored offset with translation in metres and rotation in X, Y, Z degrees.
#[must_use]
pub fn offset_transform(position: [f32; 3], rotation: [f32; 3]) -> Transform {
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

#[derive(Clone, Copy)]
pub(crate) struct NodePlacement {
    pub(crate) transform: Transform,
    pub(crate) node: usize,
}

pub(crate) fn node_placement(model: &Model, name: &str) -> Option<NodePlacement> {
    placement_at(model, named_node(model, name)?)
}

fn named_node(model: &Model, name: &str) -> Option<usize> {
    model
        .nodes
        .iter()
        .position(|node| node.name.eq_ignore_ascii_case(name))
}

fn placement_at(model: &Model, node: usize) -> Option<NodePlacement> {
    let mut transform = Transform::IDENTITY;
    for (_, ancestor) in model.node_ancestors(node) {
        transform = ancestor.transform.then(&transform);
        if ancestor.parent.is_none() {
            return Some(NodePlacement { transform, node });
        }
    }
    None
}

pub(crate) struct ItemOffsetPlacement {
    pub(crate) node: usize,
    pub(crate) inverse: Transform,
}

pub(crate) fn item_offset_placement(
    model: Option<&Model>,
    helper: &str,
) -> Result<Option<ItemOffsetPlacement>, IssueKind> {
    let Some(model) = model.filter(|_| !helper.is_empty()) else {
        return Ok(None);
    };
    let node = named_node(model, helper).ok_or_else(|| ItemOffsetDiagnostic::NotFound {
        helper: helper.to_owned(),
    })?;
    let inverse = placement_at(model, node)
        .ok_or_else(|| ItemOffsetDiagnostic::InvalidPlacement {
            helper: helper.to_owned(),
        })?
        .transform
        .inverse()
        .ok_or_else(|| ItemOffsetDiagnostic::NotInvertible {
            helper: helper.to_owned(),
        })?;
    Ok(Some(ItemOffsetPlacement { node, inverse }))
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
    PlacementSkeleton::of(model, model.lods.first()).find(name)
}

#[derive(Clone, Copy)]
pub(crate) struct PlacementSkeleton<'a> {
    file: ModelFile,
    bones: &'a [Bone],
}

impl<'a> PlacementSkeleton<'a> {
    pub(crate) fn of(model: &'a Model, lod: Option<&'a Lod>) -> Self {
        lod.and_then(|lod| {
            Some(Self {
                file: ModelFile::LodModel(lod.level),
                bones: lod.bones.as_deref()?,
            })
        })
        .unwrap_or(Self {
            file: ModelFile::Model,
            bones: model.bones.as_slice(),
        })
    }

    pub(crate) const fn bones(self) -> &'a [Bone] {
        self.bones
    }

    pub(crate) fn find(self, name: &str) -> Option<BonePlacement> {
        let (ordinal, transform) = named_bone_pose(self.bones, name)?;
        Some(BonePlacement {
            transform,
            selection: BoneSelection {
                file: self.file,
                ordinal,
            },
        })
    }
}

/// Returns the selected original bone ordinal and its pose in skeleton space.
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
    use std::error::Error;

    use p4kx_chunks::ChunkFile;
    use p4kx_model::{CompanionError, CompanionPath, Model};
    use p4kx_testkit::TestModel;

    use super::{item_offset_placement, offset_transform};
    use crate::{IssueKind, ItemOffsetDiagnostic};

    #[test]
    fn an_item_offset_helper_with_a_cyclic_parent_has_no_placement() -> Result<(), Box<dyn Error>> {
        let bytes = TestModel::cube().single_file().cgf();
        let file = ChunkFile::parse(bytes)?;
        let mut companions =
            |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> { Ok(None) };
        let mut model = Model::read("item.cgf", &file, &mut companions)?;
        let node = model.nodes.first_mut().ok_or("item node")?;
        node.parent = Some(0);
        let name = node.name.clone();
        assert!(!name.is_empty(), "the cyclic node has a selectable name");
        assert!(
            matches!(item_offset_placement(Some(&model), &name),
                Err(IssueKind::ItemOffsetHelper(ItemOffsetDiagnostic::InvalidPlacement { helper })) if helper == name),
            "a named cyclic node has no placement"
        );
        Ok(())
    }

    fn close(actual: [f32; 4], expected: [f32; 4]) {
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-5, "{actual:?} against {expected:?}");
        }
    }

    #[test]
    fn a_quarter_turn_about_z_maps_x_to_y() {
        let transform = offset_transform([1.0, 2.0, 3.0], [0.0, 0.0, 90.0]);
        close(transform.rows[0], [0.0, -1.0, 0.0, 1.0]);
        close(transform.rows[1], [1.0, 0.0, 0.0, 2.0]);
        close(transform.rows[2], [0.0, 0.0, 1.0, 3.0]);
    }

    /// The local axes of a part placed with `rotation`, as the columns of its rotation matrix.
    fn axes(rotation: [f32; 3]) -> [[f32; 3]; 3] {
        let rows = offset_transform([0.0; 3], rotation).rows;
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
        let transform = offset_transform([0.0; 3], [90.0, 0.0, 90.0]);
        let up = transform.rows.map(|row| row[2]);
        for (actual, expected) in up.iter().zip([1.0, 0.0, 0.0]) {
            assert!((actual - expected).abs() < 1e-5, "{up:?}");
        }
    }
}
