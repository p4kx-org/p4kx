// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::Query;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::{SourceInput, SourceLocation, SourceUseRole};

use super::FieldStatisticsReport;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn rejected_scalar_keeps_the_inline_width_used_to_locate_its_bytes() -> TestResult {
    let bytes = inline_fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Owner where score > 100")?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        0,
        "the original scalar is read at the offset after the inline Class"
    );
    assert!(
        report.selection_origin.source_set_complete(),
        "complete rejection does not require an admitted Record"
    );
    assert!(
        declaration(&report, "dataforgePropertyDefinition", 1, "target"),
        "the preceding Class property supplies its physical target"
    );
    assert!(
        declaration(&report, "dataforgeStructDefinition", 0, "size"),
        "the inline target Struct supplies the consumed width"
    );
    assert!(
        !declaration(&report, "dataforgeStructDefinition", 0, "propertyCount"),
        "the rejected query only consumes the inline target's size"
    );
    Ok(())
}

#[test]
fn boolean_by_head_keeps_its_original_physical_dispatch_key() -> TestResult {
    let bytes = inline_fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(r#"at "synthetic/*" where enabled = true"#)?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        0,
        "the Boolean operand decides the original rejection"
    );
    assert!(
        report.selection_origin.source_set_complete(),
        "the simple Boolean predicate can certify complete selection"
    );
    assert!(
        declaration(&report, "dataforgePropertyDefinition", 3, "target"),
        "the nominal-free plan consumes the Boolean head's original target word"
    );
    Ok(())
}

#[test]
fn compound_fixed_comparisons_bind_only_demanded_operand_bytes() -> TestResult {
    let bytes = inline_fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(
        "from Owner where not ((score < 10 and enabled = false) or aux.unrelated = 999)",
    )?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(report.reduction.statistics().records(), 0);
    assert!(report.selection_origin.source_set_complete());

    let mut operands = selection_bytes(&report, &bytes, "slot");
    operands.sort_by_key(|bytes| bytes.len());
    assert_eq!(operands, [&[0][..], &[7, 0, 0, 0][..]]);
    Ok(())
}

#[test]
fn fixed_text_selection_binds_the_original_bytes_and_terminator() -> TestResult {
    let bytes = text_fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(r#"from Owner where label = "Other""#)?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(report.reduction.statistics().records(), 0);
    assert!(report.selection_origin.source_set_complete());
    assert!(selection_bytes(&report, &bytes, "text").contains(&b"Verdigris".as_slice()));
    assert_eq!(selection_bytes(&report, &bytes, "textTerminator"), &[b"\0"]);
    assert!(matches!(
        selection_bytes(&report, &bytes, "slot").as_slice(),
        [slot] if slot.len() == 4
    ));
    assert!(report.inventory.iter_uses().any(|entry| {
        report
            .selection_origin
            .source_uses_refs()
            .contains(entry.id())
            && matches!(entry.location(), SourceLocation::Subresource { namespace, field, .. }
                if namespace == "dataforgeHeader" && field == "textLength")
    }));
    Ok(())
}

#[test]
fn an_unreadable_fixed_text_operand_keeps_a_recovered_selection_incomplete() -> TestResult {
    let mut bytes = text_fixture();
    let start = bytes
        .windows(b"Verdigris\0".len())
        .position(|slice| slice == b"Verdigris\0")
        .ok_or("original fixture label")?;
    *bytes.get_mut(start).ok_or("first fixture label byte")? = 0xff;
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(r#"from Owner where label = "Other" or enabled = true"#)?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(report.reduction.statistics().records(), 1);
    assert!(!report.selection_origin.source_set_complete());
    assert!(
        selection_bytes(&report, &bytes, "text")
            .iter()
            .all(|text| !text.contains(&0xff))
    );
    assert!(report.inventory.iter_uses().any(|entry| {
        report.selection_attempts.contains(entry.id())
            && entry.role() == SourceUseRole::Attempted
            && matches!(entry.location(), SourceLocation::Subresource { namespace, field, .. }
                if namespace == "dataforgeDecodedBytes" && field == "text")
    }));
    Ok(())
}

#[test]
fn a_retained_unreadable_predicate_root_keeps_selection_incomplete() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Owner").field("score", TestType::Int32))
        .record(
            TestRecord::new(1, "Owner", "Known", "synthetic/known.xml")
                .value("score", TestValue::Int32(7)),
        )
        .record(
            TestRecord::new(2, "Owner", "Unreadable", "synthetic/unreadable.xml")
                .with_instance_index(u16::MAX),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Owner where score > 3")?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        (
            report.reduction.statistics().records(),
            report.reduction.statistics().unreadable_records()
        ),
        (2, 1),
        "ordinary Query retains the unreadable row beside the successful comparison"
    );
    assert!(
        !report.selection_origin.source_set_complete() && !report.unavailable_traversals.is_empty(),
        "a missing instance leaves predicate selection partial"
    );
    assert_eq!(
        report.root_attempts.len(),
        1,
        "the original physical failed root locator is retained"
    );
    Ok(())
}

#[test]
fn numeric_where_behind_a_pointer_keeps_selection_incomplete() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Child").field("score", TestType::Int32))
        .structure(TestStruct::new("Owner").field("child", TestType::StrongPointer("Child".into())))
        .record(
            TestRecord::new(1, "Owner", "Known", "synthetic/known.xml").value(
                "child",
                TestValue::Struct(TestInstance::new("Child").value("score", TestValue::Int32(7))),
            ),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Owner where child.score > 3")?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        1,
        "the original comparison reads through the pointer"
    );
    assert!(
        !report.selection_origin.source_set_complete(),
        "a pointer path is outside the fixed scalar proof"
    );
    Ok(())
}

fn selection_bytes<'a>(
    report: &FieldStatisticsReport<'_>,
    bytes: &'a [u8],
    expected_field: &str,
) -> Vec<&'a [u8]> {
    let selected = report.selection_origin.source_uses_refs();
    report
        .inventory
        .iter_uses()
        .filter(|entry| selected.contains(entry.id()))
        .filter_map(|entry| match entry.location() {
            SourceLocation::Subresource {
                namespace,
                indices,
                field,
            } if namespace == "dataforgeDecodedBytes" && field == expected_field => {
                let [start, end] = indices.as_slice() else {
                    return None;
                };
                bytes.get(usize::try_from(*start).ok()?..usize::try_from(*end).ok()?)
            }
            _ => None,
        })
        .collect()
}

fn text_fixture() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Owner")
                .field("label", TestType::String)
                .field("enabled", TestType::Bool),
        )
        .record(
            TestRecord::new(1, "Owner", "Known", "synthetic/known.xml")
                .value("label", TestValue::Text("Verdigris".into()))
                .value("enabled", TestValue::Bool(true)),
        )
        .build()
}

fn declaration(
    report: &FieldStatisticsReport<'_>,
    expected_namespace: &str,
    index: u64,
    name: &str,
) -> bool {
    let origin = report.selection_origin.source_uses_refs();
    report.inventory.iter_uses().any(|site| {
        origin.contains(site.id()) && site.role() == SourceUseRole::Selector
            && matches!(site.location(), SourceLocation::Subresource { namespace, indices, field }
                if namespace == expected_namespace && indices.as_slice() == [index] && field == name)
    })
}

fn inline_fixture() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Child").field("unrelated", TestType::Int32))
        .structure(
            TestStruct::new("Owner")
                .field("aux", TestType::Class("Child".into()))
                .field("score", TestType::Int32)
                .field("enabled", TestType::Bool),
        )
        .record(
            TestRecord::new(1, "Owner", "Known", "synthetic/known.xml")
                .value(
                    "aux",
                    TestValue::Struct(
                        TestInstance::new("Child").value("unrelated", TestValue::Int32(999)),
                    ),
                )
                .value("score", TestValue::Int32(7))
                .value("enabled", TestValue::Bool(false)),
        )
        .build()
}
