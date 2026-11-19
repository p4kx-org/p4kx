// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttributePosition, SourceElementPosition};

use super::{encode, subresource};
use crate::source_inventory::PhysicalSourceRef;
use crate::source_use::LocationView;
use crate::{InputScope, SourceLocation, SourceUseRole};

#[test]
fn vehicle_xml_positions_use_short_opaque_ids() {
    let source = PhysicalSourceRef::new("a0.1.2");
    for (location, expected, generic) in [
        (
            SourceLocation::xml_element(&SourceElementPosition::CryXml(0), "$name"),
            "c6:a0.1.2psynnA",
            "c6:a0.1.2pss?10:cryxmlNode0.:$name",
        ),
        (
            SourceLocation::xml_attribute(
                &SourceAttributePosition::CryXml {
                    node_index: 0,
                    attribute_index: 64,
                },
                "name",
            ),
            "c6:a0.1.2psyaiA.BA",
            "c6:a0.1.2pss?15:cryxmlAttribute0.64.:name",
        ),
        (
            SourceLocation::xml_element(&SourceElementPosition::Text(63..64), "$children"),
            "c6:a0.1.2psyec_.BA",
            "c6:a0.1.2pss?14:textXmlElement63.64.:$children",
        ),
        (
            SourceLocation::xml_attribute(&SourceAttributePosition::Text(5..70), "skipPart"),
            "c6:a0.1.2psytsF.BG",
            "c6:a0.1.2pss?16:textXmlAttribute5.70.:skipPart",
        ),
    ] {
        let location = location.expect("the XML constructor supplies the original coordinate");
        let reference = encode(
            &source,
            InputScope::Payload,
            SourceUseRole::Selector,
            LocationView::Original(&location),
        );
        assert_eq!(
            reference.as_str(),
            expected,
            "typed XML position {location:?}"
        );
        assert!(
            reference.as_str().len() < generic.len(),
            "abbreviate the coordinate"
        );
    }
}

#[test]
fn unsupported_vehicle_xml_coordinates_keep_the_generic_encoding() {
    let source = PhysicalSourceRef::new("a0.1.2");
    for (namespace, indices, field, generic) in [
        ("cryxmlNode", vec![3], "Name", "s?10:cryxmlNode3.:Name"),
        (
            "cryxmlAttribute",
            vec![1, 2],
            "é",
            "s?15:cryxmlAttribute1.2.:é",
        ),
        ("cryxmlNode", vec![], "$name", "s?10:cryxmlNode:$name"),
        (
            "cryxmlAttribute",
            vec![1],
            "skipPart",
            "s?15:cryxmlAttribute1.:skipPart",
        ),
        (
            "textXmlElement",
            vec![32, 12],
            "$node",
            "s?14:textXmlElement32.12.:$node",
        ),
        (
            "textXmlAttribute",
            vec![1, 1],
            "name",
            "s?16:textXmlAttribute1.1.:name",
        ),
        (
            "textXmlAttribute",
            vec![1, 2, 3],
            "name",
            "s?16:textXmlAttribute1.2.3.:name",
        ),
        (
            "cryxmlNode",
            vec![u64::from(u32::MAX) + 1],
            "$children",
            "s?10:cryxmlNode4294967296.:$children",
        ),
        (
            "cryxmlAttribute",
            vec![0, u64::from(u32::MAX) + 1],
            "skipPart",
            "s?15:cryxmlAttribute0.4294967296.:skipPart",
        ),
        ("CryxmlNode", vec![3], "$name", "s?10:CryxmlNode3.:$name"),
    ] {
        let location = subresource(namespace, &indices, field);
        let reference = encode(
            &source,
            InputScope::Payload,
            SourceUseRole::Selector,
            LocationView::Original(&location),
        );
        assert_eq!(
            reference.as_str(),
            format!("c6:a0.1.2ps{generic}"),
            "fallback retains the exact original coordinate {location:?}"
        );
    }
}

pub(super) fn physical_xml_locations() -> [SourceLocation; 6] {
    [
        SourceLocation::xml_element(&SourceElementPosition::CryXml(u32::MAX), "$children")
            .expect("maximum CryXml node index"),
        SourceLocation::xml_attribute(
            &SourceAttributePosition::CryXml {
                node_index: 0,
                attribute_index: u32::MAX,
            },
            "skipPart",
        )
        .expect("maximum CryXml attribute index"),
        SourceLocation::xml_element(&SourceElementPosition::Text(0..usize::MAX), "$attributes")
            .expect("maximum text XML offset"),
        SourceLocation::xml_attribute(&SourceAttributePosition::Text(0..usize::MAX), "name")
            .expect("text XML attribute has the same byte coordinate and a separate namespace"),
        subresource("textXmlElement", &[1, u64::MAX], "$attributes"),
        subresource("textXmlAttribute", &[u64::MAX - 1, u64::MAX], "skipPart"),
    ]
}
