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
fn unknown_vehicle_xml_names_keep_the_generic_encoding() {
    assert_generic_xml_coordinate("cryxmlNode", &[3], "Name", "s?10:cryxmlNode3.:Name");
    assert_generic_xml_coordinate(
        "cryxmlAttribute",
        &[1, 2],
        "é",
        "s?15:cryxmlAttribute1.2.:é",
    );
    assert_generic_xml_coordinate("CryxmlNode", &[3], "$name", "s?10:CryxmlNode3.:$name");
}

#[test]
fn invalid_vehicle_xml_coordinates_keep_the_generic_encoding() {
    let cases: [(&str, &[u64], &str, &str); 7] = [
        ("cryxmlNode", &[], "$name", "s?10:cryxmlNode:$name"),
        (
            "cryxmlAttribute",
            &[1],
            "skipPart",
            "s?15:cryxmlAttribute1.:skipPart",
        ),
        (
            "textXmlElement",
            &[32, 12],
            "$node",
            "s?14:textXmlElement32.12.:$node",
        ),
        (
            "textXmlAttribute",
            &[1, 1],
            "name",
            "s?16:textXmlAttribute1.1.:name",
        ),
        (
            "textXmlAttribute",
            &[1, 2, 3],
            "name",
            "s?16:textXmlAttribute1.2.3.:name",
        ),
        (
            "cryxmlNode",
            &[u64::from(u32::MAX) + 1],
            "$children",
            "s?10:cryxmlNode4294967296.:$children",
        ),
        (
            "cryxmlAttribute",
            &[0, u64::from(u32::MAX) + 1],
            "skipPart",
            "s?15:cryxmlAttribute0.4294967296.:skipPart",
        ),
    ];
    for (namespace, indices, field, generic) in cases {
        assert_generic_xml_coordinate(namespace, indices, field, generic);
    }
}

fn assert_generic_xml_coordinate(namespace: &str, indices: &[u64], field: &str, generic: &str) {
    let source = PhysicalSourceRef::new("a0.1.2");
    let location = subresource(namespace, indices, field);
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
