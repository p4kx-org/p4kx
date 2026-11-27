// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttribute, SourceElement};

use crate::soc_entities;
use crate::soc_source_node::{SocSourceNode, SocSourceValue};

/// One original entity declaration in a room connector document.
#[derive(Debug, Clone, Copy)]
pub struct SocRoomConnector<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocRoomConnector<'document> {
    /// Enumerates the Entity declarations in an `EntityComponentRoomConnector` root.
    pub fn declarations(root: SourceElement<'document>) -> impl Iterator<Item = Self> {
        SocSourceNode::children(root, "Entity").map(|node| Self { node })
    }

    /// Returns the original slot among the root's direct elements.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.node.ordinal()
    }

    /// Returns the original positioned XML element.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Returns the authored owner attributes, including repeats.
    pub fn guid_attributes(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        self.node.attributes("CryGUID")
    }

    /// Reads the position of the pivot relative to its declared owner.
    #[must_use]
    pub fn pivot_relative_position(self) -> SocSourceValue<[f64; 3]> {
        self.vector("PivotRelativePos")
    }

    /// Reads the pivot quaternion in its stored order.
    #[must_use]
    pub fn pivot_relative_rotation(self) -> SocSourceValue<[f64; 4]> {
        self.vector("PivotRelativeRot")
    }

    /// Reads the authored cross-section position.
    #[must_use]
    pub fn cross_section_position(self) -> SocSourceValue<[f64; 3]> {
        self.vector("CrossSectionPos")
    }

    /// Reads the authored cross-section quaternion in its stored order.
    #[must_use]
    pub fn cross_section_rotation(self) -> SocSourceValue<[f64; 4]> {
        self.vector("CrossSectionRot")
    }

    fn vector<const N: usize>(self, name: &str) -> SocSourceValue<[f64; N]> {
        self.node
            .read(name, |text| soc_entities::numbers(Some(text)))
    }
}
