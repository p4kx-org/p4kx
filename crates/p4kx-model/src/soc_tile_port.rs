// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttribute, SourceElement};

use crate::soc_entities::numbers;
use crate::soc_source_node::{SocSourceNode, SocSourceValue, named};

/// One original tile attachment declaration on an SOC Entity.
#[derive(Debug, Clone, Copy)]
pub struct SocTileAttachment<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocTileAttachment<'document> {
    pub(crate) fn read(entity: SourceElement<'document>) -> impl Iterator<Item = Self> {
        SocSourceNode::children(entity, "TileAttachmentParams").map(|node| Self { node })
    }

    /// Returns the original attachment element and its position.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Returns all original item port selector attributes.
    pub fn item_port_attributes(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        self.node.attributes("ItemPortName")
    }

    /// Reads the unique authored item port name.
    #[must_use]
    pub fn item_port(self) -> SocSourceValue<&'document str> {
        self.node.read("ItemPortName", nonempty)
    }
}

/// One original tile item port in an SOC container descriptor.
#[derive(Debug, Clone, Copy)]
pub struct SocTileItemPort<'document> {
    section: SocSourceNode<'document>,
    node: SocSourceNode<'document>,
}

impl<'document> SocTileItemPort<'document> {
    pub(crate) fn read(root: SourceElement<'document>) -> impl Iterator<Item = Self> {
        SocSourceNode::direct_children(root)
            .flat_map(|owner| {
                let direct = named(owner.element(), "TileItemPortEntries").then_some(owner);
                let tile = named(owner.element(), "TileXmlEntry").then_some(owner);
                direct.into_iter().chain(tile.into_iter().flat_map(|tile| {
                    SocSourceNode::children(tile.element(), "TileItemPortEntries")
                }))
            })
            .flat_map(|section| {
                SocSourceNode::children(section.element(), "ItemPort")
                    .map(move |node| Self { section, node })
            })
    }

    /// Returns the original port-table section.
    #[must_use]
    pub const fn section(self) -> SourceElement<'document> {
        self.section.element()
    }

    /// Returns the original item port and its position.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Returns all original item port name attributes.
    pub fn name_attributes(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        self.node.attributes("name")
    }

    /// Returns all original interaction offset attributes.
    pub fn offset_attributes(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        self.node.attributes("interactionOffset")
    }

    /// Reads the unique authored item port name.
    #[must_use]
    pub fn name(self) -> SocSourceValue<&'document str> {
        self.node.read("name", nonempty)
    }

    /// Reads the authored translation in the container owner's frame, in metres.
    #[must_use]
    pub fn interaction_offset(self) -> SocSourceValue<[f64; 3]> {
        self.node
            .read("interactionOffset", |value| numbers(Some(value)))
    }
}

fn nonempty(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}
