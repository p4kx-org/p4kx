// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_cryxml as cryxml;
use p4kx_cryxml::XmlFieldKind::{
    AttributeName, AttributeValue, ClosingElementName, ElementName, Text,
};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn emitted_fields_cite_actual_xml_bytes_and_original_duplicate_attribute_slots() -> TestResult {
    let fixture = TestElement::new("r:oot")
        .attribute("dup", "a&b")
        .attribute("dup", "")
        .child(TestElement::new("leaf").content("<v>"))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Big);
    let document = cryxml::parse(&fixture)?;
    let mut expected_bytes = Vec::new();
    document.write_xml(&mut expected_bytes)?;
    let mut bytes = Vec::new();
    let mut fields = Vec::new();
    document.write_xml_observed(&mut bytes, |field| {
        fields.push(field);
        Ok(())
    })?;
    assert_eq!(
        bytes, expected_bytes,
        "observing exact output keeps the existing serialization"
    );
    let text = std::str::from_utf8(&bytes)?;
    let actual = fields
        .iter()
        .map(|field| {
            let range = field.range();
            let bytes = text
                .get(usize::try_from(range.start)?..usize::try_from(range.end)?)
                .ok_or("emitted XML field extent")?;
            Ok((
                field.kind(),
                field.node_index(),
                field.attribute_index(),
                bytes,
            ))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    assert_eq!(
        actual,
        [
            (ElementName, 0, None, "r_oot"),
            (AttributeName, 0, Some(0), "dup"),
            (AttributeValue, 0, Some(0), "a&amp;b"),
            (AttributeName, 0, Some(1), "dup_2"),
            (AttributeValue, 0, Some(1), ""),
            (ElementName, 1, None, "leaf"),
            (Text, 1, None, "&lt;v&gt;"),
            (ClosingElementName, 1, None, "leaf"),
            (ClosingElementName, 0, None, "r_oot"),
        ],
        "the observer preserves escaping, zero-length values, duplicate slots and both tag occurrences"
    );
    assert_eq!(
        fields
            .iter()
            .filter_map(|field| field
                .preceding_names()
                .map(|range| (range.start, range.end)))
            .collect::<Vec<_>>(),
        [(0, 1)]
    );
    Ok(())
}
