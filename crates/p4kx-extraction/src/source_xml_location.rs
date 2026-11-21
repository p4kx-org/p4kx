// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttributePosition, SourceElementPosition};

use super::SourceLocation;

impl SourceLocation {
    /// Addresses one original attribute by its text byte range or CryXml table coordinates.
    #[must_use]
    pub fn xml_attribute(position: &SourceAttributePosition, field: &str) -> Option<Self> {
        let (namespace, indices) = match position {
            SourceAttributePosition::Text(range) => (
                "textXmlAttribute",
                vec![range.start as u64, range.end as u64],
            ),
            SourceAttributePosition::CryXml {
                node_index,
                attribute_index,
            } => (
                "cryxmlAttribute",
                vec![u64::from(*node_index), u64::from(*attribute_index)],
            ),
            _ => return None,
        };
        Some(xml(namespace, indices, field))
    }

    /// Addresses one original element by its text byte range or CryXml node index.
    #[must_use]
    pub fn xml_element(position: &SourceElementPosition, field: &str) -> Option<Self> {
        let (namespace, indices) = match position {
            SourceElementPosition::Text(range) => {
                ("textXmlElement", vec![range.start as u64, range.end as u64])
            }
            SourceElementPosition::CryXml(index) => ("cryxmlNode", vec![u64::from(*index)]),
            _ => return None,
        };
        Some(xml(namespace, indices, field))
    }
}

fn xml(namespace: &str, indices: Vec<u64>, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices,
        field: field.to_owned(),
    }
}
