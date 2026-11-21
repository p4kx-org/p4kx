// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceDocument, SourceElement};

/// A source-ordered item or Items group in a Loadout XML document.
#[derive(Debug, Clone, Copy)]
pub struct PositionedLoadoutNode<'document> {
    parent: Option<usize>,
    element: SourceElement<'document>,
}

impl<'document> PositionedLoadoutNode<'document> {
    /// Returns the source element with its original coordinates and attributes.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.element
    }

    /// Returns the preceding node that physically contains this node.
    #[must_use]
    pub const fn parent(self) -> Option<usize> {
        self.parent
    }

    /// Returns whether this node is an Items group, including an empty group.
    #[must_use]
    pub fn is_group(self) -> bool {
        self.element.name() == "Items"
    }
}

/// A borrowed source view of the original Loadout and its Items hierarchy.
pub struct PositionedLoadout<'document> {
    root: SourceElement<'document>,
}

impl<'document> PositionedLoadout<'document> {
    /// Recognizes an unqualified Loadout document.
    #[must_use]
    pub fn read(document: &'document SourceDocument<'_>) -> Option<Self> {
        let root = document.root();
        named(root, "Loadout").then_some(Self { root })
    }

    /// Returns the original root and any uninterpreted top-level source content.
    #[must_use]
    pub const fn root(&self) -> SourceElement<'document> {
        self.root
    }

    /// Visits every direct Items group and Item in source preorder.
    pub fn nodes(&self) -> impl Iterator<Item = PositionedLoadoutNode<'document>> {
        let mut pending = Vec::new();
        append(&mut pending, self.root, None, "Items");
        let mut next = 0;
        std::iter::from_fn(move || {
            let node = pending.pop()?;
            let index = next;
            next += 1;
            let child = if node.is_group() { "Item" } else { "Items" };
            append(&mut pending, node.element, Some(index), child);
            Some(node)
        })
    }
}

fn append<'document>(
    pending: &mut Vec<PositionedLoadoutNode<'document>>,
    owner: SourceElement<'document>,
    parent: Option<usize>,
    name: &str,
) {
    let start = pending.len();
    pending.extend(
        owner
            .elements()
            .filter(|element| named(*element, name))
            .map(|element| PositionedLoadoutNode { parent, element }),
    );
    if let Some(children) = pending.get_mut(start..) {
        children.reverse();
    }
}

fn named(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name() == name
}
