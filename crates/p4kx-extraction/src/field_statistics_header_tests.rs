// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::Query;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::{SourceInput, SourceLocation, SourceUseRole};

use super::{FieldStatisticsReport, FieldStatisticsSelectionProof};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn complete_header_selection_keeps_a_failed_admitted_body_separate() -> TestResult {
    let bytes = fixture(Some(u16::MAX));
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(r#"from Owner at "synthetic/*""#)?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.selection_proof,
        FieldStatisticsSelectionProof::QueryHeaders { record_count: 2 },
        "the declared selector reads the complete original header scope"
    );
    assert!(
        report.selection_origin.source_set_complete(),
        "both original header decisions are proved"
    );
    assert_eq!(
        report.reduction.statistics().records(),
        2,
        "both requested Records were visited"
    );
    assert_eq!(
        report.reduction.statistics().unreadable_records(),
        1,
        "the corrupt admitted instance failed independently"
    );
    assert_eq!(
        report.root_attempts.len(),
        1,
        "the original selected root read was attempted"
    );
    let [field] = report.fields.as_slice() else {
        return Err("one known statistic remains".into());
    };
    assert!(
        !field.extrema.source_set_complete(),
        "unknown fields from the unreadable admitted body may alter the result"
    );
    let stats = report
        .reduction
        .statistics()
        .fields()
        .first()
        .ok_or("the known value survives")?;
    assert_eq!(
        stats.min().map(|minimum| minimum.to_string()),
        Some("9".into()),
        "the original surviving datum"
    );
    Ok(())
}

#[test]
fn record_filenames_and_ordering_names_share_one_exact_use() -> TestResult {
    let bytes = fixture(None);
    let mut expected = Vec::new();
    for text in [
        b"synthetic/first.xml\0synthetic/second.xml\0".as_slice(),
        b"First\0Other\0".as_slice(),
    ] {
        let start = bytes
            .windows(text.len())
            .position(|window| window == text)
            .ok_or("the original adjacent stored text")?;
        expected.extend([start as u64, (start + text.len()) as u64]);
    }
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(r#"at "synthetic/*""#)?;
    for report in [
        FieldStatisticsReport::all(&forge, SourceInput::Unknown)?,
        FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?,
    ] {
        let matches = report.inventory.iter_uses().filter(|site| {
            report
                .selection_origin
                .source_uses_refs()
                .contains(site.id())
                && site.role() == SourceUseRole::Selector
                && matches!(
                    site.location(),
                    SourceLocation::Subresource { namespace, indices, field }
                        if namespace == "dataforgeRecordPathTextRanges"
                            && field == "text"
                            && *indices == expected
                )
        });
        assert_eq!(
            matches.count(),
            1,
            "header text comes from one use with the exact observed intervals"
        );
    }
    Ok(())
}

#[test]
fn excluded_unreadable_filename_does_not_establish_a_complete_query() -> TestResult {
    let mut bytes = fixture(None);
    let mut file = None;
    DataForge::open(&bytes)?.path_order_observed(|row| {
        if row.record_index == 1 {
            file = row.file_name;
        }
    });
    let start = usize::try_from(file.ok_or("the original physical filename exists")?.start)?;
    *bytes
        .get_mut(start)
        .ok_or("the original filename byte exists")? = u8::MAX;
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(r#"at "synthetic/*""#)?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        1,
        "the invalid filename is excluded in ordinary path order"
    );
    assert!(
        !report.selection_origin.source_set_complete(),
        "an omitted physical header prevents a complete negative admission decision"
    );
    assert!(
        report.inventory.iter_uses().any(|site| {
            report.selection_attempts.contains(site.id())
                && site.role() == SourceUseRole::Attempted
                && matches!(site.location(), SourceLocation::DataForge { record_index: 1, field } if field == "fileName")
        }),
        "the excluded physical Record retains its original failed filename locator"
    );
    Ok(())
}

fn fixture(unreadable_instance: Option<u16>) -> Vec<u8> {
    let mut second = TestRecord::new(2, "Owner", "Other", "synthetic/second.xml")
        .value("number", TestValue::Int32(17));
    if let Some(instance) = unreadable_instance {
        second = second.with_instance_index(instance);
    }
    DataForgeBuilder::new()
        .structure(TestStruct::new("Owner").field("number", TestType::Int32))
        .record(
            TestRecord::new(1, "Owner", "First", "synthetic/first.xml")
                .value("number", TestValue::Int32(9)),
        )
        .record(second)
        .build()
}
