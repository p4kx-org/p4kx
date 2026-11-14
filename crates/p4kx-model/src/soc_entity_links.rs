// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttribute, SourceElement};

use crate::soc_source_node::{SocSourceNode, SocSourceValue};

/// An authored `EntityLinks` group in an original SOC Entity.
#[derive(Debug, Clone, Copy)]
pub struct SocEntityLinks<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocEntityLinks<'document> {
    pub(crate) fn read(entity: SourceElement<'document>) -> impl Iterator<Item = Self> {
        SocSourceNode::children(entity, "EntityLinks").map(|node| Self { node })
    }

    /// Returns the group slot within the owning Entity.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.node.ordinal()
    }

    /// Returns the original group and all its authored children.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Visits original link occurrences in sibling order.
    pub fn links(self) -> impl Iterator<Item = SocEntityLink<'document>> {
        SocSourceNode::children(self.node.element(), "Link").map(|node| SocEntityLink { node })
    }
}

/// One authored link to a SOC Entity.
#[derive(Debug, Clone, Copy)]
pub struct SocEntityLink<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocEntityLink<'document> {
    /// Returns the link's slot within the owning group.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.node.ordinal()
    }

    /// Returns the original link and all of its attributes.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Returns the authored `Name` attribute, including an explicit empty string.
    #[must_use]
    pub fn name(self) -> SocSourceValue<&'document str> {
        self.node.read("Name", Some)
    }

    /// Returns original `TargetCryGUID` attributes in source order.
    pub fn target_attributes(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        self.node.attributes("TargetCryGUID")
    }
}
