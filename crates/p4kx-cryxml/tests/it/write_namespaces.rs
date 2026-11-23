// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_cryxml::{self as cryxml, XmlFieldKind, XmlRawKind};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

use super::TestResult;

#[test]
fn reserved_default_namespaces_use_an_ordinary_attribute_with_exact_sources() -> TestResult {
    let order = CryXmlByteOrder::Little;
    for uri in [
        "http://www.w3.org/XML/1998/namespace",
        "http://www.w3.org/2000/xmlns/",
    ] {
        let fixture = TestElement::new("Root")
            .attribute("xmlns", uri)
            .attribute("_xmlns", "occupied")
            .attribute("_xmlns_2", "also occupied")
            .attribute("xmlns", "urn:p4kx:duplicate")
            .to_cryxml(CryXmlSignature::CryXmlB, order);
        check_namespace_alias(&fixture, uri, order)?;
    }
    Ok(())
}

fn check_namespace_alias(fixture: &[u8], uri: &str, order: CryXmlByteOrder) -> TestResult {
    let input = cryxml::parse(fixture)?;
    assert_eq!(input.root().attribute("xmlns"), Some(uri), "raw namespace");

    let render = super::render(fixture)?;
    let document = roxmltree::Document::parse(std::str::from_utf8(&render.bytes)?)?;
    let root = document.root_element();
    assert_eq!(
        root.tag_name().namespace(),
        None,
        "reserved binding removed"
    );
    assert_eq!(root.attribute("_xmlns"), Some("occupied"), "sibling name");
    assert_eq!(root.attribute("_xmlns_3"), Some(uri), "ordinary alias");
    assert_eq!(
        root.attribute("xmlns_2"),
        Some("urn:p4kx:duplicate"),
        "duplicate keeps its suffix"
    );

    let alias = super::find(&render, XmlFieldKind::AttributeName, b"_xmlns_3")?;
    let (name_offset, _) = super::fact(fixture, alias, XmlRawKind::AttributeNameOffset)?;
    assert_eq!(
        name_offset,
        u64::from(super::word(fixture, 20, order)?),
        "the alias retains the original name-slot offset"
    );
    let (_, raw_name) = super::fact(fixture, alias, XmlRawKind::StringBytes)?;
    assert_eq!(
        raw_name, b"xmlns",
        "the alias retains the original name bytes"
    );
    let value = super::find(&render, XmlFieldKind::AttributeValue, uri.as_bytes())?;
    assert!(
        value
            .sources()
            .all(|source| alias.sources().any(|used| used == source)),
        "the alias uses the same original value as the emitted value"
    );

    check_observed_namespace_slots(&input)
}

fn check_observed_namespace_slots(input: &cryxml::Document<'_>) -> TestResult {
    let mut fields = Vec::new();
    input.write_xml_observed(io::sink(), |field| {
        fields.push(field);
        Ok(())
    })?;
    let field = fields
        .iter()
        .find(|field| field.namespace_alias_names().is_some())
        .ok_or("namespace alias field")?;
    assert_eq!(
        field.attribute_index(),
        Some(0),
        "the original slot is retained"
    );
    assert_eq!(
        field.namespace_value_index(),
        Some(0),
        "the default value selects the alias"
    );
    assert_eq!(
        field.namespace_alias_names(),
        Some(&(0..4)),
        "the alias avoids every sibling attribute name"
    );
    Ok(())
}

#[test]
fn a_legal_default_namespace_and_its_empty_override_are_preserved() -> TestResult {
    let fixture = TestElement::new("Root")
        .attribute("xmlns", "urn:p4kx:test")
        .attribute("xmlns", "http://www.w3.org/XML/1998/namespace")
        .child(TestElement::new("Child").attribute("xmlns", ""))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let render = super::render(&fixture)?;
    let document = roxmltree::Document::parse(std::str::from_utf8(&render.bytes)?)?;
    let root = document.root_element();
    assert_eq!(
        root.tag_name().namespace(),
        Some("urn:p4kx:test"),
        "the legal default still applies"
    );
    assert_eq!(
        root.attribute("xmlns_2"),
        Some("http://www.w3.org/XML/1998/namespace"),
        "a suffixed name is an ordinary attribute"
    );
    let child = root.first_element_child().ok_or("the child is present")?;
    assert_eq!(
        child.tag_name().namespace().unwrap_or_default(),
        "",
        "empty default override"
    );
    Ok(())
}
