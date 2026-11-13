// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_cryxml::SourceElementPosition;

use crate::Transform;
use crate::mtl_xml::XmlElement;
use crate::soc_source_node::SocSourceValue;

/// Authored placement shared by a light group's projected baked lights.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SocGroupPlacement {
    /// Original entity and component coordinates.
    pub source: Option<SocGroupCoordinates>,
    /// The group entity's original transform in the containing SOC.
    pub authored: Transform,
    /// The original tile selector, with repeated or malformed declarations retained as invalid.
    pub tile: SocSourceValue<String>,
    /// The original helper requested by the group component.
    pub helper: SocSourceValue<String>,
}

/// Element coordinates within the XML payload of one SOC chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SocGroupCoordinates {
    /// Entity chunk ordinal in the SOC chunk table.
    pub chunk: usize,
    /// Light group's original enclosing entity element.
    pub entity: SourceElementPosition,
    /// Original light group component element.
    pub component: SourceElementPosition,
    /// Original tile attachment element when unique.
    pub tile: Option<SourceElementPosition>,
}

impl SocGroupPlacement {
    pub(crate) fn read<'source, E: XmlElement<'source>>(
        entity: E,
        group: E,
        authored: Transform,
        chunk: Option<usize>,
    ) -> Arc<Self> {
        let mut attachments = entity
            .children()
            .filter(|node| node.name() == "TileAttachmentParams");
        let (first, second) = (attachments.next(), attachments.next());
        let tile = match (first, second) {
            (None, _) => SocSourceValue::Absent,
            (Some(node), None) => match field(node, "ItemPortName") {
                SocSourceValue::Present(name) => SocSourceValue::Present(name),
                _ => SocSourceValue::Invalid,
            },
            _ => SocSourceValue::Invalid,
        };
        Arc::new(Self {
            source: chunk.map(|chunk| SocGroupCoordinates {
                chunk,
                entity: entity.position(),
                component: group.position(),
                tile: first.filter(|_| second.is_none()).map(XmlElement::position),
            }),
            authored,
            tile,
            helper: field(group, "helper"),
        })
    }
}

fn field<'source, E: XmlElement<'source>>(element: E, field: &str) -> SocSourceValue<String> {
    let mut attributes = element.attributes().filter(|(name, _)| *name == field);
    match (attributes.next(), attributes.next()) {
        (None, _) => SocSourceValue::Absent,
        (Some((_, value)), None) if !value.is_empty() => SocSourceValue::Present(value.to_owned()),
        _ => SocSourceValue::Invalid,
    }
}
