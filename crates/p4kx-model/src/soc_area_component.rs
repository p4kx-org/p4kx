// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttribute, SourceElement};

use crate::soc_room_segment::SocRoomSegment;
use crate::soc_source_node::SocSourceNode;

/// One authored `AreaComponent` occurrence in a SOC Entity.
#[derive(Debug, Clone, Copy)]
pub struct SocAreaComponent<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocAreaComponent<'document> {
    pub(crate) fn read(entity: SourceElement<'document>) -> impl Iterator<Item = Self> {
        SocSourceNode::children(entity, "AreaComponent").map(|node| Self { node })
    }

    /// Returns the zero-based slot among the Entity's direct elements.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.node.ordinal()
    }

    /// Returns the original component including unclassified children.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Visits target declarations in original group and child order.
    pub fn links(self) -> impl Iterator<Item = SocAreaLink<'document>> {
        SocSourceNode::children(self.node.element(), "Entities").flat_map(|group| {
            SocSourceNode::children(group.element(), "Entity").map(move |node| SocAreaLink {
                group_ordinal: group.ordinal(),
                node,
            })
        })
    }

    /// Visits all original segments, including unsupported or malformed geometry.
    pub fn segments(self) -> impl Iterator<Item = SocRoomSegment<'document>> {
        SocSourceNode::children(self.node.element(), "Segment").map(SocRoomSegment::new)
    }
}

/// One original target of an `AreaComponent`'s ordered Entities lists.
#[derive(Debug, Clone, Copy)]
pub struct SocAreaLink<'document> {
    group_ordinal: usize,
    node: SocSourceNode<'document>,
}

impl<'document> SocAreaLink<'document> {
    /// Returns the group slot within the component and target slot within that group.
    #[must_use]
    pub const fn ordinal(self) -> (usize, usize) {
        (self.group_ordinal, self.node.ordinal())
    }

    /// Returns the original target declaration.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Returns the original target `EntityCryGUID` declarations in order.
    pub fn guid_attributes(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        self.node.attributes("EntityCryGUID")
    }
}
