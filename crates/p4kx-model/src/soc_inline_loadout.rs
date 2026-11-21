// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttribute, SourceElement};

use crate::soc_source_node::{SocSourceNode, SocSourceValue};

/// An authored SOC default or nested loadout declaration.
#[derive(Debug, Clone, Copy)]
pub struct SocInlineLoadout<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocInlineLoadout<'document> {
    pub(crate) fn read(owner: SourceElement<'document>) -> impl Iterator<Item = Self> {
        SocSourceNode::children(owner, "loadout").map(|node| Self { node })
    }

    /// Returns the declaration's slot within its component or item entry.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.node.ordinal()
    }

    /// Returns the original declaration and its unclassified content.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Reads the original loadout type without assigning it a default.
    #[must_use]
    pub fn kind(self) -> SocSourceValue<SocInlineLoadoutKind<'document>> {
        self.node.read("__type", |name| match name {
            "" => None,
            "SItemPortLoadoutManualParams" => Some(SocInlineLoadoutKind::Manual),
            "<NullPtr>" => Some(SocInlineLoadoutKind::Null),
            other => Some(SocInlineLoadoutKind::Other(other)),
        })
    }

    /// Visits original entry groups of an authored manual loadout, including empty groups.
    pub fn entry_groups(self) -> impl Iterator<Item = SocInlineLoadoutEntries<'document>> {
        (self.kind() == SocSourceValue::Present(SocInlineLoadoutKind::Manual))
            .then_some(self.node.element())
            .into_iter()
            .flat_map(|owner| {
                SocSourceNode::children(owner, "entries")
                    .map(|node| SocInlineLoadoutEntries { node })
            })
    }
}

/// The declared type of a SOC loadout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SocInlineLoadoutKind<'document> {
    /// An explicit manual loadout.
    Manual,
    /// An explicit null pointer.
    Null,
    /// An unclassified original type name.
    Other(&'document str),
}

/// An original `entries` group of an inline manual loadout.
#[derive(Debug, Clone, Copy)]
pub struct SocInlineLoadoutEntries<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocInlineLoadoutEntries<'document> {
    /// Returns the group slot within its loadout declaration.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.node.ordinal()
    }

    /// Returns the original group including any unclassified children.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Visits the original manual loadout entries in sibling order.
    pub fn entries(self) -> impl Iterator<Item = SocInlineLoadoutEntry<'document>> {
        SocSourceNode::children(self.node.element(), "SItemPortLoadoutEntryParams")
            .map(|node| SocInlineLoadoutEntry { node })
    }
}

/// One original manual loadout entry.
#[derive(Debug, Clone, Copy)]
pub struct SocInlineLoadoutEntry<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocInlineLoadoutEntry<'document> {
    /// Returns the entry's slot within its original group.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.node.ordinal()
    }

    /// Returns the original entry and all authored attributes and children.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Returns the original item-port name, including an explicit empty string.
    #[must_use]
    pub fn port_name(self) -> SocSourceValue<&'document str> {
        self.node.read("itemPortName", Some)
    }

    /// Returns the original class name, including an explicit empty string.
    #[must_use]
    pub fn class_name(self) -> SocSourceValue<&'document str> {
        self.node.read("entityClassName", Some)
    }

    /// Returns the original class-reference attributes in source order.
    pub fn class_reference_attributes(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        self.node.attributes("entityClassReference")
    }

    /// Visits original nested loadout declarations owned by this entry.
    pub fn loadouts(self) -> impl Iterator<Item = SocInlineLoadout<'document>> {
        SocInlineLoadout::read(self.node.element())
    }
}
