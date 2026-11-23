// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::Query;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::{SourceInput, SourceLocation, SourceUseDescription, SourceUseRole};

use super::FieldStatisticsReport;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn selected_reference_uses_first_physical_duplicate_and_proves_a_dangling_key() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Station where linked->ready = true")?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        1,
        "the first duplicate target is true, its later duplicate is false"
    );
    assert!(
        has_range(&report, "guid", 6),
        "the first target and the dangling lookup use the same physical GUID inventory"
    );
    assert!(
        has_record_field(&report, 3, "guid")
            && has_record_field(&report, 3, "sourceType")
            && !has_record_field(&report, 4, "guid"),
        "the expected original first duplicate supplies the resolved Record and type"
    );
    assert_eq!(
        attempts(&report, "reference").count(),
        1,
        "only the non-null absent key contributes a lookup attempt"
    );
    let attempt = attempts(&report, "reference")
        .next()
        .ok_or("the original dangling bytes must have an Attempted use")?;
    assert!(
        selection(&report).any(|site| site.location() == attempt.location()),
        "the actual authored key also acts as the selector proving the lookup result"
    );
    Ok(())
}

#[test]
fn path_lookup_keeps_both_physical_matches_and_the_record_bounds() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(r#"referencing "synthetic/shared""#)?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert!(
        has_range(&report, "fileName", 6),
        "the normalized path lookup is bounded by the actual raw filename column"
    );
    for index in [3, 5] {
        assert!(
            has_record_field(&report, index, "fileName"),
            "the original path filter retains both matched Record positions"
        );
    }
    for label in ["recordCount", "structCount"] {
        assert!(
            selection(&report).any(|use_site| matches!(
                use_site.location(),
                SourceLocation::Subresource { namespace, indices, field }
                    if namespace == "dataforgeHeader" && indices.is_empty() && field == label
            )),
            "the report retains the actual physical row bounds"
        );
    }
    Ok(())
}

#[test]
fn kleene_recovery_keeps_failed_scalar_outside_the_contributing_inputs() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Station where corrupt = true or fallback = true")?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        3,
        "the decisive readable branch recovers each provisional failure"
    );
    assert_eq!(
        attempts(&report, "slot").count(),
        3,
        "each unreadable scalar is attempted"
    );
    for attempted in attempts(&report, "slot") {
        assert!(
            selection(&report).all(|used| used.location() != attempted.location()),
            "a failed scalar decode contributes no predicate value"
        );
    }
    let demanded = Query::parse("from Station where corrupt = true")?;
    assert!(
        FieldStatisticsReport::query(&forge, SourceInput::Unknown, &demanded).is_err(),
        "a demanded decode failure still refuses the entire selection"
    );
    Ok(())
}

fn selection<'report>(
    report: &'report FieldStatisticsReport<'_>,
) -> impl Iterator<Item = &'report SourceUseDescription> {
    report.inventory.iter_uses().filter(move |usage| {
        usage.role() == SourceUseRole::Selector
            && report
                .selection_origin
                .source_uses_refs()
                .contains(usage.id())
    })
}

fn attempts<'report>(
    report: &'report FieldStatisticsReport<'_>,
    label: &'report str,
) -> impl Iterator<Item = &'report SourceUseDescription> {
    report.inventory.iter_uses().filter(move |usage| {
        usage.role() == SourceUseRole::Attempted
            && report.selection_attempts.contains(usage.id())
            && matches!(
                usage.location(),
                SourceLocation::Subresource { namespace, field, .. }
                    if namespace == "dataforgeDecodedBytes" && field == label
            )
    })
}

fn has_record_field(report: &FieldStatisticsReport<'_>, index: u32, label: &str) -> bool {
    selection(report).any(|site| {
        matches!(
            site.location(),
            SourceLocation::DataForge { record_index, field }
                if *record_index == index && field == label
        )
    })
}

fn has_range(report: &FieldStatisticsReport<'_>, label: &str, end: u64) -> bool {
    selection(report).any(|site| {
        matches!(
            site.location(),
            SourceLocation::Subresource { namespace, indices, field }
                if namespace == "dataforgeRecordHeaderRange"
                    && indices.as_slice() == [0, end] && field == label
        )
    })
}

fn fixture() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Target").field("ready", TestType::Bool))
        .structure(
            TestStruct::new("Station")
                .field("linked", TestType::ReferenceTo("Target".into()))
                .field("corrupt", TestType::Bool)
                .field("fallback", TestType::Bool),
        )
        .record(station(1, "synthetic/given.xml", TestValue::Reference(17)))
        .record(station(
            2,
            "synthetic/dangling.xml",
            TestValue::Reference(991),
        ))
        .record(station(3, "synthetic/null.xml", TestValue::Null))
        .record(target(17, "synthetic/shared.xml", true))
        .record(target(17, "synthetic/later.xml", false))
        .record(target(18, "synthetic/shared.xml", false))
        .build()
}

fn station(id: u128, path: &str, linked: TestValue) -> TestRecord {
    TestRecord::new(id, "Station", "Station", path)
        .value("linked", linked)
        .value("corrupt", TestValue::Raw(vec![2]))
        .value("fallback", TestValue::Bool(true))
}

fn target(id: u128, path: &str, ready: bool) -> TestRecord {
    TestRecord::new(id, "Target", "Target", path).value("ready", TestValue::Bool(ready))
}
