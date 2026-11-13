// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::Transform;
use crate::mtl_xml::XmlElement;
use crate::soc_source_node::SocSourceValue;

/// Authored placement shared by a light group's projected baked lights.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SocGroupPlacement {
    /// The group entity's original transform in the containing SOC.
    pub authored: Transform,
    /// The original tile selector, with repeated or malformed declarations retained as invalid.
    pub tile: SocSourceValue<String>,
    /// The original helper requested by the group component.
    pub helper: SocSourceValue<String>,
}

impl SocGroupPlacement {
    pub(crate) fn read<'source, E: XmlElement<'source>>(
        entity: E,
        group: E,
        authored: Transform,
    ) -> Arc<Self> {
        let mut attachments = entity
            .children()
            .filter(|node| node.name() == "TileAttachmentParams");
        let tile = match (attachments.next(), attachments.next()) {
            (None, _) => SocSourceValue::Absent,
            (Some(node), None) => match field(node, "ItemPortName") {
                SocSourceValue::Present(name) => SocSourceValue::Present(name),
                _ => SocSourceValue::Invalid,
            },
            _ => SocSourceValue::Invalid,
        };
        Arc::new(Self {
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
