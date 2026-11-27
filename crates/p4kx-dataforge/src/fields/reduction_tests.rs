// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::finalize;
use super::layouts::SharedLayouts;
use super::part::Part;
use super::{
    FieldStatistics, FieldStatisticsInput, FieldStatisticsOperand, FieldStatisticsReduction,
    FieldStatisticsSourceScope, MAX_DISTINCT_VALUES, Number,
};
use crate::DataForge;

type TestResult = Result<(), Box<dyn Error>>;

fn merge<'source>(
    forge: &'source DataForge<'source>,
) -> Result<FieldStatisticsReduction<'source>, Box<dyn Error>> {
    let layouts = SharedLayouts::new(forge);
    let observe = |_: super::FieldStatisticsObservation<'_>| {};
    let mut left = Part::new(&layouts, Some(&observe), true);
    let mut right = Part::new(&layouts, Some(&observe), true);
    left.add(forge, 0)?;
    right.add(forge, 1)?;
    let observed = finalize::with_reduction(left.merge(right)?);

    let mut left = Part::new(&layouts, None, false);
    let mut right = Part::new(&layouts, None, false);
    left.add(forge, 0)?;
    right.add(forge, 1)?;
    let plain = finalize::ordinary(left.merge(right)?);
    assert_eq!(
        observed.statistics(),
        &plain,
        "observation preserves the actual reducer"
    );
    Ok(observed)
}

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("A")
                .array("numbers", TestType::Int32)
                .field("tie", TestType::Int32),
        )
        .structure(
            TestStruct::new("B")
                .array("numbers", TestType::Int32)
                .field("tie", TestType::Double)
                .field("extra", TestType::Int32),
        )
        .record(
            TestRecord::new(1, "A", "Left", "libs/a.xml")
                .value("numbers", numbers(0..17))
                .value("tie", TestValue::Int32(1)),
        )
        .record(
            TestRecord::new(2, "B", "Right", "libs/b.xml")
                .value("numbers", numbers(17..35))
                .value("tie", TestValue::Double(1.0))
                .value("extra", TestValue::Int32(0)),
        )
        .build()
}

fn numbers(range: std::ops::Range<i32>) -> TestValue {
    TestValue::Array(range.map(TestValue::Int32).collect())
}

fn source_record(operand: Option<&FieldStatisticsOperand>) -> Result<u32, Box<dyn Error>> {
    let source = operand.ok_or("a selected original source")?.input();
    match source {
        FieldStatisticsInput::Slot(slot) => Ok(slot.record_index),
        FieldStatisticsInput::ArrayElement { header, .. } => Ok(header.record_index),
        FieldStatisticsInput::Record(_) => Err("a Record is not a numeric cell".into()),
    }
}

#[test]
fn merged_comparisons_preserve_cap_and_the_original_mixed_numeric_tie() -> TestResult {
    let bytes = bytes();
    let forge = DataForge::open(&bytes)?;
    let observed = merge(&forge)?;
    let numbers = observed.row(1).ok_or("the sorted array row")?;
    assert_eq!(
        numbers.statistic().path(),
        "numbers",
        "array output row position"
    );
    assert_eq!(
        numbers.statistic().min(),
        Some(Number::Integer(0)),
        "minimum numeric type"
    );
    assert_eq!(
        numbers.statistic().max(),
        Some(Number::Integer(34)),
        "maximum numeric type"
    );
    assert_eq!(
        source_record(numbers.minimum())?,
        0,
        "minimum exact original owner"
    );
    assert_eq!(
        source_record(numbers.maximum())?,
        1,
        "maximum exact original owner"
    );
    assert!(
        numbers.statistic().distinct_capped(),
        "disjoint partitions together cross the cap"
    );
    check_distinct(&numbers, &bytes)?;

    check_numeric_tie(&observed.row(2).ok_or("the sorted tie row")?)?;
    assert_eq!(
        observed.source_scope(),
        FieldStatisticsSourceScope::SelectionAndPathsUnbound,
        "direct operand proof does not invent selection closure"
    );
    Ok(())
}

fn check_numeric_tie(tie: &super::FieldStatisticsReducedRow<'_, '_>) -> TestResult {
    assert_eq!(tie.statistic().path(), "tie", "tie output row position");
    assert_eq!(
        tie.statistic().min(),
        Some(Number::Double(1.0)),
        "base trie retains its equal double"
    );
    assert_eq!(
        tie.statistic().max(),
        Some(Number::Double(1.0)),
        "equal integer cannot replace the base maximum"
    );
    assert_eq!(
        source_record(tie.minimum())?,
        1,
        "minimum origin keeps the actual base trie"
    );
    assert_eq!(
        source_record(tie.maximum())?,
        1,
        "maximum origin keeps the actual base trie"
    );
    Ok(())
}

fn check_distinct(row: &super::FieldStatisticsReducedRow<'_, '_>, bytes: &[u8]) -> TestResult {
    let positions = row.distinct();
    assert_eq!(
        positions.len(),
        MAX_DISTINCT_VALUES + 1,
        "the witness keeps only cap-relevant operands"
    );
    let first = positions
        .first()
        .ok_or("the original larger trie's first operand")?;
    let last = positions.last().ok_or("physical cap operand")?;
    assert_eq!(
        source_record(first.operand())?,
        1,
        "merge retains the larger trie first"
    );
    assert_eq!(
        source_record(last.operand())?,
        0,
        "the added fingerprint comes from the other physical row"
    );
    let FieldStatisticsInput::ArrayElement {
        index,
        bytes: Some(span),
        ..
    } = last.operand().ok_or("original cap operand")?.input()
    else {
        return Err("the cap is reached by an original array value".into());
    };
    assert_eq!(
        *index, 14,
        "18 fingerprints in the base plus 15 reach the cap proof"
    );
    let range = usize::try_from(span.start)?..usize::try_from(span.end)?;
    assert_eq!(
        bytes.get(range),
        Some(14_i32.to_le_bytes().as_slice()),
        "actual cap-triggering bytes"
    );
    Ok(())
}

#[test]
fn identical_display_paths_keep_separate_structural_rows() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Child").field("value", TestType::Int32))
        .structure(
            TestStruct::new("Owner")
                .field("a/value", TestType::Int32)
                .field("a", TestType::Class("Child".to_owned())),
        )
        .record(
            TestRecord::new(1, "Owner", "One", "libs/paths.xml")
                .value("a/value", TestValue::Int32(9))
                .value(
                    "a",
                    TestValue::Struct(
                        TestInstance::new("Child").value("value", TestValue::Int32(17)),
                    ),
                ),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let reduced = FieldStatistics::of_records_with_reduction(&forge, &[0], |_| {})?;
    let mut candidates = Vec::new();
    for (ordinal, stat) in reduced.statistics().fields().iter().enumerate() {
        if stat.path() == "a/value" {
            let row = reduced.row(ordinal).ok_or("the exact row ordinal")?;
            candidates.push((
                row.leaf_to_root().map(str::to_owned).collect::<Vec<_>>(),
                row.statistic().min(),
            ));
        }
    }
    assert_eq!(
        candidates,
        [
            (vec!["a/value".to_owned()], Some(Number::Integer(9))),
            (
                vec!["value".to_owned(), "a".to_owned()],
                Some(Number::Integer(17))
            ),
        ],
        "equal printed paths cannot exchange the merged node proof"
    );
    Ok(())
}
