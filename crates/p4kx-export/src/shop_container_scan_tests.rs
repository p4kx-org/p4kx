// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

use crate::shop_container_xml;

use crate::starmap_xml::Document;

fn entities(bytes: &[u8]) -> Option<shop_container_xml::Entities<String>> {
    shop_container_xml::guids(Document::parse(bytes).ok()?.root(), |_, value| {
        Ok::<_, std::convert::Infallible>(value.to_ascii_lowercase())
    })
    .ok()
}

fn shop_objects<'node>(
    root: impl crate::starmap_xml::Node<'node>,
) -> shop_container_xml::Objects<String> {
    shop_container_xml::objects(
        root,
        |_, _, guid| Ok::<_, std::convert::Infallible>(guid.to_ascii_lowercase()),
        |_, _| {},
    )
    .unwrap_or_else(|never| match never {})
}

const METADATA: &str =
    r#"<Entities><Entity CryGUID="AB-1"/><Other/><Entity CryGUID="cd-2"/></Entities>"#;

#[test]
fn the_shop_entities_are_read_from_text_xml_with_or_without_a_byte_order_mark() {
    let expected = Some(vec!["ab-1".to_owned(), "cd-2".to_owned()]);
    assert_eq!(
        entities(METADATA.as_bytes()).map(|scan| scan.guids),
        expected
    );
    assert_eq!(
        entities(format!("\u{feff}{METADATA}").as_bytes()).map(|scan| scan.guids),
        expected
    );
}

#[test]
fn the_shop_entities_are_read_from_cryxml() {
    let cry = TestElement::new("Entities")
        .child(TestElement::new("Entity").attribute("CryGUID", "AB-1"))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    assert_eq!(
        entities(&cry).map(|scan| scan.guids),
        Some(vec!["ab-1".to_owned()])
    );
}

#[test]
fn bytes_that_are_no_xml_give_no_shop_entities() {
    assert!(entities(b"not xml").is_none(), "malformed text is not XML");
    assert!(
        entities(b"<a b=\"\xff\"/>").is_none(),
        "attribute bytes must be UTF-8"
    );
    assert!(
        entities(b"<a><b></a>").is_none(),
        "unclosed XML is not a source"
    );
}

#[test]
fn a_shop_object_name_that_two_guids_claim_is_dropped_in_text_xml_with_a_mark() {
    let editor = "\u{feff}<Root><Object type=\"SCShop\" name=\"one\" guid=\"A\"/>\
        <Object type=\"SCShop\" name=\"two\" guid=\"B\"/>\
        <Object type=\"SCShop\" name=\"two\" guid=\"C\"/>\
        <Object type=\"Other\" name=\"three\" guid=\"D\"/></Root>";
    let document = Document::parse(editor.as_bytes()).expect("text XML");
    let named = shop_objects(document.root()).names;
    assert_eq!(named.get("one").map(String::as_str), Some("a"));
    assert!(!named.contains_key("two"), "ambiguous");
    assert!(!named.contains_key("three"), "not a shop");
}
