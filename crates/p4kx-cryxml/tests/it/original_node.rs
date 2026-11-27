// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_cryxml::Element;
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

#[test]
fn original_node_positions_stay_bound_to_the_actual_borrowed_document() -> Result<(), Box<dyn Error>>
{
    let bytes = TestElement::new("Root")
        .attribute("key", "original")
        .child(TestElement::new("First").attribute("path", "one.xml"))
        .child(TestElement::new("Second").attribute("path", "two.xml"))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let first = p4kx_cryxml::parse(&bytes)?;
    let other = p4kx_cryxml::parse(&bytes)?;
    let copied = first.clone();
    let root = first.root();
    let mut children = root.children();
    let one = children.next().ok_or("original first element")?;
    let two = children.next().ok_or("original second element")?;
    source_values(root, one, two);
    assert!(
        children.next().is_none(),
        "the original source has exactly two children"
    );
    assert_eq!(
        other.root().name(),
        root.name(),
        "the separate original parse has equal visible data"
    );
    assert_eq!(root.index(), 0, "the original root node-table position");
    assert_eq!(
        one.index(),
        1,
        "the original first child node-table position"
    );
    assert_eq!(
        two.index(),
        2,
        "the original second child node-table position"
    );
    assert!(
        root.is_from(&first) && one.is_from(&first) && two.is_from(&first),
        "all genuine borrows retain their actual owner"
    );
    assert!(
        !root.is_from(&other) && !one.is_from(&other),
        "equal parsed bytes do not substitute another owner"
    );
    assert!(
        !one.is_from(&copied),
        "cloning the whole document cannot adopt a foreign borrowed node"
    );
    let unchanged = one;
    assert!(
        unchanged.is_from(&first),
        "an unchanged node clone keeps its original borrow"
    );
    assert!(
        copied.root().is_from(&copied),
        "the cloned document's own real node is admitted"
    );
    Ok(())
}

fn source_values(root: Element<'_>, one: Element<'_>, two: Element<'_>) {
    assert_eq!(root.name(), "Root", "the real parsed root");
    assert_eq!(
        root.attribute("key"),
        Some("original"),
        "the actual root attribute"
    );
    assert_eq!(
        (one.name(), one.attribute("path")),
        ("First", Some("one.xml")),
        "the actual first node"
    );
    assert_eq!(
        (two.name(), two.attribute("path")),
        ("Second", Some("two.xml")),
        "the actual second node"
    );
}
