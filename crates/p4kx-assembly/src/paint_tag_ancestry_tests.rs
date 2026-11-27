// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, TagTree};
use p4kx_extraction::SourceLocation;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::append_ancestry;
use crate::paint_provenance::PaintSelectionInput;
use crate::{PaintPort, PaintTagIndex};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const FIRST_ANCESTOR: u128 = 1;
const SIDE_ROOT: u128 = 2;
const EARLY_OFFER: u128 = 3;
const SELECTED_ANCESTOR: u128 = 4;
const MIDDLE: u128 = 5;
const SELECTED_OFFER: u128 = 6;
const DISCARDED: u128 = 7;
const WRONG_TYPE: u128 = 8;
const DATABASE: u128 = 9;
const UPPER: u128 = 10;

#[test]
fn ancestry_uses_the_matching_physical_names_and_original_child_slots() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let tree = TagTree::build(&forge);
    let index = PaintTagIndex::new(&tree);
    let wanted = PaintPort::new(None, Some("WANTED"));
    let offered = [String::from("offered")];
    assert_eq!(
        tree.issues().duplicate(),
        1,
        "the fixture has a rejected physical alias"
    );
    assert_eq!(
        tree.issues().repeated(),
        2,
        "the fixture has a cycle and alternate parent edge"
    );
    assert!(
        wanted.accepts(&offered, &[], &[], &index),
        "the original boolean decision matches"
    );
    let matched = wanted
        .match_with_ancestry(&offered, &[], &[], &index)
        .ok_or("the actual scoped paint match")?;
    let [ancestry] = matched.ancestries() else {
        return Err("one exact winning ancestry was required".into());
    };
    assert_eq!(
        ancestry
            .tags()
            .map(|tag| tag.record_index())
            .collect::<Vec<_>>(),
        [Some(5), Some(4), Some(3)],
        "the duplicate names and GUID alias cannot replace the actually used physical chain"
    );
    let mut sources = Vec::new();
    append_ancestry(&mut sources, ancestry)?;
    let expected = [
        location(5, "tagName"),
        location(3, "tagName"),
        location(4, "children[2]"),
        location(3, "children[1]"),
    ];
    assert_eq!(
        sources, expected,
        "the existing paint provenance reads only physical matching endpoints and chosen stored edges"
    );
    Ok(())
}

#[test]
fn rejected_alternatives_and_equal_names_do_not_create_ancestry_evidence() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let tree = TagTree::build(&forge);
    let index = PaintTagIndex::new(&tree);
    let offered = [String::from("offered")];
    for requirement in ["discarded", "forged", "remembered-alias", "wanted missing"] {
        let port = PaintPort::new(None, Some(requirement));
        assert!(
            !port.accepts(&offered, &[], &[], &index)
                && port
                    .match_with_ancestry(&offered, &[], &[], &index)
                    .is_none(),
            "{requirement} cannot add a rejected edge, type or later physical GUID alias"
        );
    }
    let exact = PaintPort::new(None, Some("Wanted"));
    let alias = [String::from("remembered-alias")];
    assert!(
        !exact.accepts(&alias, &[], &[], &index)
            && exact
                .match_with_ancestry(&alias, &[], &[], &index)
                .is_none(),
        "the later duplicate-GUID alias cannot reach the canonical tag's ancestors"
    );
    let match_without_tree = exact
        .match_with_ancestry(&[String::from("wanted")], &[], &[], &index)
        .ok_or("the exact same-name fit")?;
    assert!(
        match_without_tree.ancestries().is_empty(),
        "the equal-name branch uses authored words without selecting any duplicate tree entries"
    );
    Ok(())
}

#[test]
fn required_tags_keep_port_before_vehicle_and_preserve_requirement_order() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let tree = TagTree::build(&forge);
    let index = PaintTagIndex::new(&tree);
    let port = PaintPort::new(Some("offered"), Some("wanted"));
    let offered = [String::from("offered")];
    let required = [String::from("wanted"), String::from("side")];
    let vehicle = [String::from("wanted"), String::from("side")];
    let matched = port
        .match_with_ancestry(&offered, &required, &vehicle, &index)
        .ok_or("both reverse requirements are met")?;
    assert!(
        matched.uses_port_tags() && !matched.uses_vehicle_tags(),
        "a matching port offer wins over an equal vehicle offer"
    );
    let endpoints = matched
        .ancestries()
        .iter()
        .map(|path| {
            (
                path.offered().record_index(),
                path.required().record_index(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        endpoints,
        [(Some(5), Some(3)), (Some(5), Some(3)), (Some(2), Some(1))],
        "port requirements follow their original order and choose each matching offered occurrence"
    );
    let fallback_port = PaintPort::new(None, Some("wanted"));
    let fallback = fallback_port
        .match_with_ancestry(&offered, &required, &vehicle, &index)
        .ok_or("a required port tag and vehicle fallback both match")?;
    assert!(
        !fallback.uses_port_tags() && fallback.uses_vehicle_tags(),
        "without an optional port offer the vehicle contributes only the unmet requirement"
    );
    assert_eq!(
        fallback.ancestries().len(),
        1,
        "the two lexical reverse matches add no topology"
    );
    Ok(())
}

fn location(record_index: u32, field: &str) -> PaintSelectionInput {
    PaintSelectionInput::DataForge(SourceLocation::DataForge {
        record_index,
        field: field.to_owned(),
    })
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .field("legacyGUID", TestType::UInt32)
                .array("children", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(TestStruct::new("NotATag").field("tagName", TestType::String))
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .record(tag(FIRST_ANCESTOR, "wanted", &[]))
        .record(tag(SIDE_ROOT, "side", &[EARLY_OFFER]))
        .record(tag(EARLY_OFFER, "offered", &[]))
        .record(tag(SELECTED_ANCESTOR, "Wanted", &[0, MIDDLE]))
        .record(tag(MIDDLE, "middle", &[0, 0, SELECTED_OFFER]))
        .record(tag(SELECTED_OFFER, "Offered", &[SELECTED_ANCESTOR]))
        .record(tag(SELECTED_OFFER, "remembered-alias", &[]))
        .record(tag(DISCARDED, "discarded", &[SELECTED_OFFER]))
        .record(
            TestRecord::new(
                WRONG_TYPE,
                "NotATag",
                "NotATag.Forged",
                "synthetic/not-tag.xml",
            )
            .value("tagName", TestValue::text("forged")),
        )
        .record(tag(UPPER, "higher", &[SELECTED_ANCESTOR]))
        .record(
            TestRecord::new(
                DATABASE,
                "TagDatabase",
                "TagDatabase.Roots",
                "synthetic/tags.xml",
            )
            .value(
                "tags",
                references(&[FIRST_ANCESTOR, SIDE_ROOT, UPPER, DISCARDED, WRONG_TYPE]),
            ),
        )
        .build()
}

fn tag(identifier: u128, name: &str, children: &[u128]) -> TestRecord {
    TestRecord::new(
        identifier,
        "Tag",
        format!("Tag.{name}"),
        "synthetic/tags.xml",
    )
    .value("tagName", TestValue::text(name))
    .value("legacyGUID", TestValue::UInt32(u32::MAX))
    .value("children", references(children))
}

fn references(identifiers: &[u128]) -> TestValue {
    TestValue::Array(
        identifiers
            .iter()
            .map(|&id| {
                if id == 0 {
                    TestValue::Null
                } else {
                    TestValue::Reference(id)
                }
            })
            .collect(),
    )
}
