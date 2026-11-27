// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::VecDeque;

use p4kx_dataforge::{RecordTree, StructValue, Value};
use p4kx_extraction::SourceLocation;

use super::GeometryComponent;
use super::origins::Position;
use crate::read::{indexed_struct_items, struct_field};
use crate::spec::GEOMETRY_RESOURCE;

/// One authored geometry node in its original Entity component and nested array position.
#[derive(Debug)]
pub struct GeometrySource<'tree, 'forge> {
    position: Position,
    node: &'tree StructValue<'forge>,
}

impl<'tree, 'forge> GeometrySource<'tree, 'forge> {
    /// The original Tags field of this node, including explicit empty or absent states.
    #[must_use]
    pub fn tags(&self) -> Option<&'tree Value<'forge>> {
        self.node.field("Tags")
    }

    /// The original field address of this node's Tags.
    #[must_use]
    pub fn tag_location(&self) -> SourceLocation {
        self.position.at(".Tags")
    }

    /// The original authored model resource path.
    #[must_use]
    pub fn model(&self) -> Option<&'tree Value<'forge>> {
        self.node
            .field("Geometry")
            .and_then(crate::read::as_struct)
            .and_then(|data| struct_field(data, "Geometry"))
            .and_then(|resource| resource.field("path"))
    }

    /// The original field address of this node's model path.
    #[must_use]
    pub fn model_location(&self) -> SourceLocation {
        self.position.at(".Geometry.Geometry.path")
    }

    /// The containing Geometry component position in the Entity record.
    #[must_use]
    pub const fn component(&self) -> usize {
        self.position.owner.ordinal
    }

    /// The stored original nested `SubGeometry` array positions.
    #[must_use]
    pub fn sub_geometry(&self) -> &[usize] {
        &self.position.slots
    }
}

/// Visits geometry nodes by original component and breadth-first nested position.
///
/// # Errors
/// Returns the caller's exact interruption or projection error.
pub fn visit_geometry_sources<'tree, 'forge, E>(
    tree: &'tree RecordTree<'forge>,
    mut visit: impl FnMut(GeometrySource<'tree, 'forge>) -> Result<(), E>,
) -> Result<(), E> {
    for (ordinal, component) in indexed_struct_items(tree.root(), "Components") {
        if component.type_name() == GEOMETRY_RESOURCE
            && let Some(node) = struct_field(component, "Geometry")
        {
            let owner = GeometryComponent {
                record: tree.record_index(),
                ordinal,
            };
            visit_nodes(owner, node, &mut visit)?;
        }
    }
    Ok(())
}

fn visit_nodes<'tree, 'forge, E>(
    owner: GeometryComponent,
    node: &'tree StructValue<'forge>,
    visit: &mut impl FnMut(GeometrySource<'tree, 'forge>) -> Result<(), E>,
) -> Result<(), E> {
    let mut pending = VecDeque::from([(node, Vec::<usize>::new())]);
    while let Some((node, slots)) = pending.pop_front() {
        for (slot, child) in indexed_struct_items(node, "SubGeometry") {
            let mut child_slots = slots.clone();
            child_slots.push(slot);
            pending.push_back((child, child_slots));
        }
        visit(GeometrySource {
            position: Position {
                owner,
                slots: slots.into(),
            },
            node,
        })?;
    }
    Ok(())
}
