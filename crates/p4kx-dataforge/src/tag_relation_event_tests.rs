// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{TestRecord, TestValue};

use crate::{
    DataForge, DataForgeError, Guid, RecordTree, TagBuildEvent, TagPathEdgeKind, TagRelationIssue,
    TagTree, Value, ValueError,
};

use super::TestResult;

type SkippedRelation = (u32, TagPathEdgeKind, usize, Option<Guid>, TagRelationIssue);

#[test]
fn skipped_root_and_child_relations_retain_original_slots_after_null_and_invalid_prefixes()
-> TestResult {
    let bytes = bytes();
    let forge = DataForge::open(&bytes)?;
    let target = forge.record(1).ok_or("the original Tag")?.guid();
    let mut missing = None;
    let mut controls = 0;
    let mut skipped = Vec::new();
    let tree = TagTree::build_observed(&forge, &|| false, &mut |event| {
        match event {
            TagBuildEvent::CanonicalRead { record, outcome } => {
                missing = Some(original(
                    record.index(),
                    outcome.map_err(DataForgeError::clone)?,
                )?);
                controls += 1;
            }
            TagBuildEvent::RelationSkipped {
                incoming,
                target,
                reason,
            } => {
                skipped.push((
                    incoming.record().index(),
                    incoming.kind(),
                    incoming.ordinal(),
                    target,
                    reason,
                ));
            }
            _ => {}
        }
        Ok::<_, Box<dyn Error>>(())
    })?
    .ok_or("the actual build must not be cancelled")?;
    assert_eq!(
        controls, 2,
        "both original arrays are checked independently before event assertions"
    );
    let missing = missing.ok_or("the original dangling GUID word")?;
    assert_eq!(tree.len(), 1, "the same first physical Tag is selected");
    assert_eq!(
        skipped,
        expected(missing, target),
        "skipped events keep their actual owner, typed role, original slot and reason"
    );
    Ok(())
}

fn bytes() -> Vec<u8> {
    super::builder()
        .record(
            TestRecord::new(2, "TagDatabase", "Database", "database.xml").value(
                "tags",
                TestValue::Array(vec![
                    TestValue::Null,
                    TestValue::Reference(999),
                    TestValue::Reference(super::FIRST),
                    TestValue::Reference(super::FIRST),
                ]),
            ),
        )
        .record(
            TestRecord::new(super::FIRST, "Tag", "One", "one.xml")
                .value("tagName", TestValue::text("One"))
                .value(
                    "children",
                    TestValue::Array(vec![
                        TestValue::Null,
                        TestValue::Reference(super::FIRST),
                        TestValue::Reference(999),
                    ]),
                ),
        )
        .build()
}

const fn expected(missing: Guid, target: Guid) -> [SkippedRelation; 4] {
    [
        (
            0,
            TagPathEdgeKind::Root,
            1,
            Some(missing),
            TagRelationIssue::Dangling,
        ),
        (
            1,
            TagPathEdgeKind::Child,
            2,
            Some(missing),
            TagRelationIssue::Dangling,
        ),
        (
            1,
            TagPathEdgeKind::Child,
            1,
            Some(target),
            TagRelationIssue::Repeated,
        ),
        (
            0,
            TagPathEdgeKind::Root,
            3,
            Some(target),
            TagRelationIssue::Repeated,
        ),
    ]
}

fn original(index: u32, tree: &RecordTree<'_>) -> Result<Guid, Box<dyn Error>> {
    assert_eq!(
        tree.error_count(),
        1,
        "one original dangling Reference per physical source"
    );
    let (field, length, missing) = if index == 0 {
        ("tags", 4, 1)
    } else {
        ("children", 3, 2)
    };
    let Some(Value::Array(items)) = tree.field(field) else {
        return Err("the original array".into());
    };
    assert_eq!(
        items.len(),
        length,
        "the source array retains every stored occurrence"
    );
    assert!(
        matches!(items.first(), Some(Value::Null)),
        "the original null prefix is not compressed"
    );
    let Some(Value::Error(ValueError::DanglingReference { guid })) = items.get(missing) else {
        return Err("the original dangling word at its declared stored position".into());
    };
    assert!(
        matches!(
            items.get(if index == 0 { 2 } else { 1 }),
            Some(Value::Reference(_))
        ),
        "the later original Reference is fully decoded before event assertions"
    );
    Ok(*guid)
}
