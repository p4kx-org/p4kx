// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{Entry, PathIndex};

use super::INTERIOR_NODE;
use crate::{Assembly, AssemblyError, EntityNode, ObjectContainerRef};

/// The largest difference per axis, in metres, between two translations that still counts as
/// the same place.
const OFFSET_TOLERANCE: f32 = 0.05;

pub(crate) fn near(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| (x - y).abs() <= OFFSET_TOLERANCE)
}

pub(super) fn find<'a>(index: &PathIndex<'a>, file: &str) -> Option<&'a Entry> {
    let path = file.trim_start_matches(['/', '\\']);
    [format!("Data\\{path}"), path.to_owned()]
        .iter()
        .find_map(|candidate| index.find(candidate))
}

/// Returns the path of the `Interior` node of the part at the port path `owner_path`.
pub(super) fn node_path(owner_path: &str) -> String {
    if owner_path.is_empty() {
        INTERIOR_NODE.to_owned()
    } else {
        format!("{owner_path}/{INTERIOR_NODE}")
    }
}

/// Checks that `tree` is the Entity `assembly` was built from: the same root Record and the same
/// containers.
pub(super) fn check_tree(assembly: &Assembly, tree: &EntityNode) -> Result<(), AssemblyError> {
    match (assembly.parts.first(), assembly.containers.first()) {
        (Some(root), Some(containers))
            if root.guid() != tree.guid()
                || !(Arc::ptr_eq(containers, &tree.object_containers)
                    || same_containers(containers, tree.object_containers())) =>
        {
            Err(AssemblyError::TreeMismatch)
        }
        _ => Ok(()),
    }
}

fn same_containers(a: &[ObjectContainerRef], b: &[ObjectContainerRef]) -> bool {
    let bits = |values: [f32; 3]| values.map(f32::to_bits);
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.file == b.file
                && a.bone == b.bone
                && a.source == b.source
                && bits(a.position) == bits(b.position)
                && bits(a.rotation) == bits(b.rotation)
        })
}
