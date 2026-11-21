// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, TreeBuilder, Value, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const GUID: u128 = 0x8b00_0000_0000_4000_8000_0000_0000_0001;

fn record() -> TestRecord {
    TestRecord::new(GUID, "Root", "Root.Same", "same.xml")
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Root").field("flag", TestType::Bool))
        .record(record().value("flag", TestValue::Bool(true)))
        .build()
}

#[test]
fn equal_canonical_values_do_not_make_two_original_tree_owners_interchangeable() -> TestResult {
    let bytes = database();
    let left = DataForge::open(&bytes)?;
    let right = DataForge::open(&bytes)?;
    let actual = left.record(0).ok_or("actual Record absent")?;
    let foreign = right.record(0).ok_or("foreign Record absent")?;
    assert!(
        actual.is_from(&left) && !actual.is_from(&right),
        "the actual source handles are distinct"
    );
    let tree = actual.tree()?;
    let other = foreign.tree()?;
    assert_eq!(
        tree, other,
        "the original seven-field value equality excludes private source authority"
    );
    assert!(
        tree.is_from(&actual),
        "the actual completed read retains its original owner"
    );
    assert!(
        !tree.is_from(&foreign),
        "equal values, GUID and index cannot lend foreign authority"
    );
    let cloned = tree.clone();
    assert_eq!(
        tree, cloned,
        "a genuine clone preserves every canonical value"
    );
    assert!(
        cloned.is_from(&actual) && !cloned.is_from(&foreign),
        "a whole original clone retains its exact owner"
    );
    Ok(())
}

#[test]
fn duplicate_guid_rows_of_one_forge_keep_their_distinct_original_positions() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Root"))
        .record(record())
        .record(record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let first = forge.record(0).ok_or("first physical Record absent")?;
    let second = forge.record(1).ok_or("second physical Record absent")?;
    assert_eq!(
        first.guid(),
        second.guid(),
        "the fixture deliberately shares a nominal GUID"
    );
    assert_ne!(
        first.index(),
        second.index(),
        "both original physical rows exist"
    );
    let tree = first.tree()?;
    assert!(
        tree.is_from(&first),
        "the tree belongs to its exact original position"
    );
    assert!(
        !tree.is_from(&second),
        "same Forge and GUID do not replace the actual source row"
    );
    Ok(())
}

#[test]
fn empty_fields_and_empty_header_text_cannot_replace_a_genuine_owner_with_value_guesses()
-> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Root"))
        .record(
            TestRecord::new(GUID, "Root", "", "")
                .without_name()
                .without_tag(),
        )
        .build();
    let actual = DataForge::open(&bytes)?;
    let other = DataForge::open(&bytes)?;
    let record = actual.record(0).ok_or("empty original Record absent")?;
    let foreign = other.record(0).ok_or("empty foreign Record absent")?;
    let tree = record.tree()?;
    assert!(
        tree.fields().is_empty(),
        "the actual source has no payload fields"
    );
    assert!(
        tree.name().is_empty() && tree.record_path().is_empty(),
        "empty source text is genuine"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "the fieldless original is a successful canonical read"
    );
    assert!(
        tree.is_from(&record) && !tree.is_from(&foreign),
        "empty descriptors cannot mint or erase the actual owner"
    );
    Ok(())
}

#[test]
fn a_failed_read_and_builder_reuse_cannot_reassign_a_previous_trees_owner() -> TestResult {
    let good_bytes = database();
    let bad_bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Root")
                .field("value", TestType::Int32)
                .declared_size(6),
        )
        .record(record())
        .build();
    let left = DataForge::open(&good_bytes)?;
    let broken = DataForge::open(&bad_bytes)?;
    let right = DataForge::open(&good_bytes)?;
    let first = left.record(0).ok_or("first original Record absent")?;
    let bad = broken.record(0).ok_or("malformed original Record absent")?;
    let last = right.record(0).ok_or("last original Record absent")?;
    assert!(
        bad.tree().is_err(),
        "the actual declared-layout source fails before a tree exists"
    );
    let mut builder = TreeBuilder::new();
    let old = builder.build(&first)?;
    assert!(
        builder.build(&bad).is_err(),
        "the failing original read supplies no successful tree"
    );
    let new = builder.build(&last)?;
    assert!(
        old.is_from(&first) && !old.is_from(&last),
        "the earlier completed read keeps its owner after builder reuse"
    );
    assert!(
        new.is_from(&last) && !new.is_from(&first),
        "the later completed read uses its actual owner after failure"
    );
    Ok(())
}

#[test]
fn an_actual_read_with_retained_value_errors_still_has_its_genuine_source_owner() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Root").field("flag", TestType::Bool))
        .record(record().value("flag", TestValue::Raw(vec![0xff])))
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual Record absent")?;
    let tree = record.tree()?;
    assert!(
        matches!(
            tree.field("flag"),
            Some(Value::Error(ValueError::InvalidBool { byte: 0xff }))
        ),
        "the original malformed scalar remains an error value at its actual source slot"
    );
    assert_eq!(
        tree.error_count(),
        1,
        "source ownership is independent of value-read health"
    );
    assert!(
        tree.is_from(&record),
        "retained source errors do not invent a missing original owner"
    );
    Ok(())
}
