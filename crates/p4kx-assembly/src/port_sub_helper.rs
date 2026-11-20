// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{StructValue, Value};

use crate::PortTransform;
use crate::read;

/// One authored node of an item port's `SubHelpers` tree.
#[derive(Debug, Clone, PartialEq)]
pub struct PortSubHelper {
    parent: Option<usize>,
    ordinal: usize,
    tags: Option<Box<str>>,
    transform: Option<PortTransform>,
}

impl PortSubHelper {
    /// Returns the parent's index in the port's subhelper slice, or `None` for the root helper.
    #[must_use]
    pub const fn parent(&self) -> Option<usize> {
        self.parent
    }

    /// Returns the original index in the parent's `SubHelpers` array.
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Returns the authored tag selector.
    #[must_use]
    pub fn tags(&self) -> Option<&str> {
        self.tags.as_deref()
    }

    /// Returns this node's transform when it declares a helper.
    #[must_use]
    pub const fn transform(&self) -> Option<&PortTransform> {
        self.transform.as_ref()
    }
}

pub(crate) fn tags(node: &StructValue<'_>) -> Option<Box<str>> {
    match node.field("Tags") {
        Some(Value::String(tags)) => Some((*tags).into()),
        _ => None,
    }
}

pub(crate) fn read(root: &StructValue<'_>, attachment: &str) -> Box<[PortSubHelper]> {
    let mut helpers = Vec::new();
    let mut pending = Vec::new();
    push_children(root, None, &mut pending);
    while let Some(child) = pending.pop() {
        push_children(child.node, Some(helpers.len()), &mut pending);
        helpers.push(PortSubHelper {
            parent: child.parent,
            ordinal: child.ordinal,
            tags: tags(child.node),
            transform: PortTransform::read(child.node, attachment),
        });
    }
    helpers.into_boxed_slice()
}

struct Pending<'tree, 'source> {
    parent: Option<usize>,
    ordinal: usize,
    node: &'tree StructValue<'source>,
}

fn push_children<'tree, 'source>(
    node: &'tree StructValue<'source>,
    parent: Option<usize>,
    pending: &mut Vec<Pending<'tree, 'source>>,
) {
    let Some(Value::Array(children)) = node.field("SubHelpers") else {
        return;
    };
    pending.extend(
        children
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(ordinal, child)| {
                read::as_struct(child).map(|node| Pending {
                    parent,
                    ordinal,
                    node,
                })
            }),
    );
}

#[cfg(test)]
#[path = "port_sub_helper_tests.rs"]
mod tests;
