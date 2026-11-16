// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ptr;

use p4kx_extraction::{Archive, SourceInventory, SourceLocation, SourceUseRole};
use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};

use super::{EntrySourceXml, SourceXmlAttribute};
use crate::catalog_read::original::{EntryCapture, EntryRead, EntryReceipt, with_entry_capture};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn duplicate_attribute_occurrences_share_only_their_own_name_selector() -> TestResult {
    let bytes = TestElement::new("AnimDB")
        .attribute("T\u{e4}g", "first")
        .attribute("T\u{e4}g", "second")
        .attribute("t\u{e4}g", "third")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("animation.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("fixture.adb", bytes).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    with_entry_capture(&archive, |capture| Ok(check_attributes(&archive, capture)))?
}

fn check_attributes(archive: &Archive, capture: &EntryCapture<'_>) -> TestResult {
    let [entry] = archive.entries() else {
        return Err("one actual animation Entry".into());
    };
    let receipt = EntryReceipt::outer(capture, archive, entry)?;
    let read = EntryRead::read_verified(receipt, archive, entry, None)??;
    let xml = EntrySourceXml::parse(&read, capture)??;
    let attributes = xml.attributes(xml.root())?;
    let expected = [
        ("T\u{e4}g", "first"),
        ("T\u{e4}g", "second"),
        ("t\u{e4}g", "third"),
    ];
    assert_eq!(
        attributes.len(),
        expected.len(),
        "every source slot is retained"
    );
    let inventory = capture.source_uses().freeze()?;
    for (ordinal, (attribute, (name, value))) in attributes.iter().zip(expected).enumerate() {
        assert_eq!(
            attribute.name.value().map(String::as_str),
            Some(name),
            "the original spelling is unchanged",
        );
        assert_eq!(
            attribute.namespace.value(),
            Some(&None),
            "no CryXml namespace"
        );
        assert_eq!(
            attribute.value.value().map(String::as_str),
            Some(value),
            "the original attribute order and payload are unchanged",
        );
        check_selector(&inventory, attribute, ordinal, name)?;
    }
    Ok(())
}

fn check_selector(
    inventory: &SourceInventory,
    attribute: &SourceXmlAttribute,
    ordinal: usize,
    name: &str,
) -> TestResult {
    let references = attribute.name.origin().source_uses_refs();
    assert!(
        ptr::eq(references, attribute.namespace.origin().source_uses_refs()),
        "name and namespace share their one original selector allocation",
    );
    let [selector] = references else {
        return Err("one exact name selector".into());
    };
    let row = inventory
        .iter_uses()
        .find(|row| row.id() == selector)
        .ok_or("the original selector inventory row")?;
    assert_eq!(row.role(), SourceUseRole::Selector, "name selector role");
    assert_eq!(
        row.location(),
        &SourceLocation::Subresource {
            namespace: "xmlSource.cryXml".to_owned(),
            indices: vec![0, u64::try_from(ordinal)?],
            field: format!("@{name}"),
        },
        "the selector retains this physical attribute occurrence",
    );
    assert_eq!(
        attribute.namespace.origin().contains_encrypted_source(),
        Some(true),
        "shared selector preserves the actual byte-domain marker",
    );
    assert!(
        !attribute
            .value
            .origin()
            .source_uses_refs()
            .contains(selector),
        "authored value still points to its separate payload use",
    );
    Ok(())
}
