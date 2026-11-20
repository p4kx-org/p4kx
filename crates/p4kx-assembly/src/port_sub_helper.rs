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
    pub(crate) tags: AuthoredHelperTags,
    transform: Option<PortTransform>,
    pub(crate) children: HelperChildren,
    pub(crate) helper_unreadable: bool,
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
        self.tags.text()
    }

    /// Returns this node's transform when it declares a helper.
    #[must_use]
    pub const fn transform(&self) -> Option<&PortTransform> {
        self.transform.as_ref()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum AuthoredHelperTags {
    #[default]
    Absent,
    Text(Box<str>),
    Unreadable,
}

impl AuthoredHelperTags {
    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Absent | Self::Unreadable => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum HelperChildren {
    #[default]
    Read,
    Unreadable,
    UnreadableSlot(usize),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PortSubHelpers {
    pub(crate) root_tags: AuthoredHelperTags,
    pub(crate) root_unreadable: bool,
    pub(crate) root_children: HelperChildren,
    pub(crate) nodes: Box<[PortSubHelper]>,
}

fn tags(node: &StructValue<'_>) -> AuthoredHelperTags {
    match node.field("Tags") {
        None => AuthoredHelperTags::Absent,
        Some(Value::String(tags)) => AuthoredHelperTags::Text((*tags).into()),
        _ => AuthoredHelperTags::Unreadable,
    }
}

pub(crate) fn read(attachment: Option<&Value<'_>>, kind: &str) -> PortSubHelpers {
    let root = match attachment {
        None | Some(Value::Null) => return PortSubHelpers::default(),
        Some(Value::Struct(attachment)) => attachment.field("Helper"),
        _ => return unreadable_tree(),
    };
    let root = match root {
        None | Some(Value::Null) => return PortSubHelpers::default(),
        Some(Value::Struct(root)) => root,
        _ => return unreadable_tree(),
    };
    let mut helpers = Vec::new();
    let mut pending = Vec::new();
    let root_children = push_children(root, None, &mut pending);
    while let Some(child) = pending.pop() {
        let children = push_children(child.node, Some(helpers.len()), &mut pending);
        helpers.push(PortSubHelper {
            parent: child.parent,
            ordinal: child.ordinal,
            tags: tags(child.node),
            transform: PortTransform::read(child.node, kind),
            children,
            helper_unreadable: matches!(
                child.node.field("Helper"),
                Some(value) if !matches!(value, Value::Null | Value::Struct(_))
            ),
        });
    }
    PortSubHelpers {
        root_tags: tags(root),
        root_unreadable: false,
        root_children,
        nodes: helpers.into_boxed_slice(),
    }
}

fn unreadable_tree() -> PortSubHelpers {
    PortSubHelpers {
        root_unreadable: true,
        ..PortSubHelpers::default()
    }
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
) -> HelperChildren {
    let children = match node.field("SubHelpers") {
        None | Some(Value::Null) => return HelperChildren::Read,
        Some(Value::Array(children)) => children,
        _ => return HelperChildren::Unreadable,
    };
    let mut outcome = HelperChildren::Read;
    for (ordinal, child) in children.iter().enumerate().rev() {
        if let Some(node) = read::as_struct(child) {
            pending.push(Pending {
                parent,
                ordinal,
                node,
            });
        } else if !matches!(child, Value::Null) {
            outcome = HelperChildren::UnreadableSlot(ordinal);
        }
    }
    outcome
}

#[cfg(test)]
#[path = "port_sub_helper_tests.rs"]
mod tests;
