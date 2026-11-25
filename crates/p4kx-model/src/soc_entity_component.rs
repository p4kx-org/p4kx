// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::SourceElement;

use crate::soc_inline_loadout::SocInlineLoadout;
use crate::soc_source_node::{SocSourceNode, SocSourceValue, named};

/// A directly authored SOC Entity component.
#[derive(Debug, Clone, Copy)]
pub struct SocEntityComponent<'document> {
    properties: Option<SocSourceNode<'document>>,
    node: SocSourceNode<'document>,
    kind: SocEntityComponentKind,
}

impl<'document> SocEntityComponent<'document> {
    pub(crate) fn read(entity: SourceElement<'document>) -> impl Iterator<Item = Self> {
        SocSourceNode::direct_children(entity).flat_map(Self::from_entity_child)
    }

    fn from_entity_child(node: SocSourceNode<'document>) -> impl Iterator<Item = Self> {
        let direct = named(node.element(), "InteractionLinkController").then_some(Self {
            properties: None,
            node,
            kind: SocEntityComponentKind::InteractionLinkController,
        });
        let properties = named(node.element(), "PropertiesDataCore").then_some(node);
        direct
            .into_iter()
            .chain(properties.into_iter().flat_map(|properties| {
                SocSourceNode::direct_children(properties.element()).filter_map(move |node| {
                    let kind = SocEntityComponentKind::from_properties(node)?;
                    Some(Self {
                        properties: Some(properties),
                        node,
                        kind,
                    })
                })
            }))
    }

    /// Returns the `PropertiesDataCore` slot, if any, and the original component slot.
    #[must_use]
    pub const fn ordinal(self) -> (Option<usize>, usize) {
        let properties = match self.properties {
            Some(node) => Some(node.ordinal()),
            None => None,
        };
        (properties, self.node.ordinal())
    }

    /// Returns the owning `PropertiesDataCore` element, when present.
    #[must_use]
    pub fn properties_element(self) -> Option<SourceElement<'document>> {
        self.properties.map(SocSourceNode::element)
    }

    /// Returns the original component, including all unclassified content.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Returns the kind identified by the original component element.
    #[must_use]
    pub const fn kind(self) -> SocEntityComponentKind {
        self.kind
    }

    /// Returns the original component type, when uniquely authored.
    #[must_use]
    pub fn declared_type(self) -> SocSourceValue<&'document str> {
        self.node
            .read("__type", |name| (!name.is_empty()).then_some(name))
    }

    /// Returns whether the authored component has the exact proximity sensor type.
    #[must_use]
    pub fn is_proximity_sensor(self) -> bool {
        self.kind == SocEntityComponentKind::ProximitySensorDeclaration
            && self.declared_type() == SocSourceValue::Present("SCItemProximitySensorParams")
    }

    /// Visits original loadout declarations of a default-loadout component.
    pub fn loadouts(self) -> impl Iterator<Item = SocInlineLoadout<'document>> {
        (self.kind == SocEntityComponentKind::DefaultLoadout)
            .then_some(self.node.element())
            .into_iter()
            .flat_map(SocInlineLoadout::read)
    }
}

/// An authored component relevant to SOC entity links and placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SocEntityComponentKind {
    /// The direct Entity element `InteractionLinkController`.
    InteractionLinkController,
    /// The `PropertiesDataCore/SCItemDoor` component.
    Door,
    /// The `PropertiesDataCore/SCItemProximitySensor` component.
    ProximitySensorDeclaration,
    /// The `PropertiesDataCore/EntityGeometryResource` component.
    GeometryResource,
    /// The `PropertiesDataCore/EntityComponentRoomConnector` component.
    RoomConnector,
    /// The `PropertiesDataCore/EntityComponentDefaultLoadout` component.
    DefaultLoadout,
    /// The `PropertiesDataCore/FogVolume` component with type `FogVolumeComponentParams`.
    FogVolume,
}

impl SocEntityComponentKind {
    fn from_properties(node: SocSourceNode<'_>) -> Option<Self> {
        let element = node.element();
        if element.namespace().is_some() {
            return None;
        }
        match element.name() {
            "SCItemDoor" => Some(Self::Door),
            "SCItemProximitySensor" => Some(Self::ProximitySensorDeclaration),
            "EntityGeometryResource" => Some(Self::GeometryResource),
            "EntityComponentRoomConnector" => Some(Self::RoomConnector),
            "EntityComponentDefaultLoadout" => Some(Self::DefaultLoadout),
            "FogVolume"
                if node.read("__type", Some)
                    == SocSourceValue::Present("FogVolumeComponentParams") =>
            {
                Some(Self::FogVolume)
            }
            _ => None,
        }
    }
}
