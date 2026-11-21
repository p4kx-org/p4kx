// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_extraction::{Archive, SourceLocation, cryxml};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};

use crate::build::original_input::test_support::TestResult;

use super::fixture::XML_PATH;

pub(super) fn bytes(binary: bool) -> Vec<u8> {
    if binary {
        TestElement::new("Loadout")
            .child(
                TestElement::new("Items").child(
                    TestElement::new("Item")
                        .attribute("portName", "self")
                        .attribute("itemName", "Self")
                        .attribute("neighbor", "after"),
                ),
            )
            .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
    } else {
        br#"<Loadout><Items><Item xmlns:x="urn:original" portName="self" x:itemName="Self" itemName="Other" neighbor="after"/></Items></Loadout>"#.to_vec()
    }
}

pub(super) fn entry(bytes: &[u8]) -> TestEntry {
    TestEntry::stored(format!("Data/{XML_PATH}"), bytes).encrypted()
}

pub(super) fn original(
    archive: &Archive,
    wanted: &[u8],
    binary: bool,
) -> TestResult<SourceLocation> {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == format!("Data/{XML_PATH}"))
        .ok_or("the genuine original XML entry")?;
    assert!(
        entry.is_encrypted(),
        "the original directory marks the XML entry encrypted"
    );
    let dcb = archive
        .entries()
        .iter()
        .find(|entry| entry.path().ends_with("Game2.dcb"))
        .ok_or("the original DCB entry")?;
    assert!(
        !dcb.is_encrypted(),
        "the selected Record header originates in an unencrypted DCB"
    );
    let mut actual = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut actual)?;
    assert_eq!(
        actual, wanted,
        "the independent actual archive read preserves every encoded XML byte"
    );
    let node = if binary {
        original_binary(&actual)?
    } else {
        original_text(&actual)?
    };
    Ok(SourceLocation::Subresource {
        namespace: "xmlAttribute".to_owned(),
        indices: vec![u64::from(node), 1],
        field: "@itemName".to_owned(),
    })
}

fn original_binary(bytes: &[u8]) -> TestResult<u32> {
    let document = cryxml::parse(bytes)?;
    let items = document
        .root()
        .children()
        .next()
        .ok_or("original binary Items")?;
    let item = items.children().next().ok_or("original binary Item")?;
    assert_eq!(item.index(), 2, "the original binary decoder node ordinal");
    let attributes: Vec<_> = item
        .attributes()
        .map(|attribute| (attribute.name(), attribute.value()))
        .collect();
    assert_eq!(
        attributes,
        [
            ("portName", "self"),
            ("itemName", "Self"),
            ("neighbor", "after")
        ],
        "the actual binary winner is between its original neighboring slots"
    );
    Ok(item.index())
}

fn original_text(bytes: &[u8]) -> TestResult<u32> {
    let document = roxmltree::Document::parse(std::str::from_utf8(bytes)?)?;
    let item = document
        .descendants()
        .find(|node| node.tag_name().name() == "Item")
        .ok_or("original text Item")?;
    assert_eq!(
        item.id().get(),
        3,
        "the original text decoder node ordinal includes its document node"
    );
    let attributes: Vec<_> = item
        .attributes()
        .map(|attribute| (attribute.name(), attribute.namespace(), attribute.value()))
        .collect();
    assert_eq!(
        attributes,
        [
            ("portName", None, "self"),
            ("itemName", Some("urn:original"), "Self"),
            ("itemName", None, "Other"),
            ("neighbor", None, "after")
        ],
        "the actual decoder preserves namespaced winner and neighbors"
    );
    assert_eq!(
        item.attribute("itemName"),
        Some("Self"),
        "the actual original getter chooses the first matching local name"
    );
    Ok(item.id().get())
}
