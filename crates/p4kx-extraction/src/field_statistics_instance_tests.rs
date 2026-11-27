// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::{Query, QueryUnavailable};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::{SourceInput, SourceLocation, SourceUseRole};

use super::FieldStatisticsReport;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn where_retains_examined_root_links_and_only_the_actual_mapping_prefix() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Selected where score > 3")?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        2,
        "the true and retained unreadable Records keep ordinary Query admission"
    );
    assert_eq!(
        report.reduction.statistics().unreadable_records(),
        1,
        "the unreadable admitted Record does not alter other field stats"
    );
    assert_root_links(&report);
    assert_prefix(&report, &forge)?;
    assert!(
        !report.selection_origin.source_set_complete(),
        "the new exact original operands do not complete arbitrary WHERE"
    );
    Ok(())
}

fn assert_root_links(report: &FieldStatisticsReport<'_>) {
    for record in [1, 2, 3] {
        assert!(
            report.inventory.iter_uses().any(|site| {
                site.role() == SourceUseRole::Selector
                    && report
                        .selection_origin
                        .source_uses_refs()
                        .contains(site.id())
                    && matches!(site.location(), SourceLocation::DataForge { record_index, field }
                        if *record_index == record && field == "instance")
            }),
            "the selected, rejected and unreadable predicate roots retain their actual instance word"
        );
    }
    assert!(
        report.unavailable_traversals.iter().any(|gap| matches!(gap,
            QueryUnavailable::Instance { structure: 1, instance, .. }
                if *instance == u32::from(u16::MAX)
        )),
        "the missing root instance has an exact logical coordinate and no fabricated payload"
    );
}

fn assert_prefix(report: &FieldStatisticsReport<'_>, forge: &DataForge<'_>) -> TestResult {
    let root = forge
        .data_mappings()
        .find(|row| row.struct_index == 1)
        .ok_or("the selected original Struct has a mapping")?;
    for field in ["structIndex", "instanceCount"] {
        assert!(
            selector(
                report,
                "dataforgeDataMapping",
                &[u64::from(root.index)],
                field
            ),
            "the actual selected mapping supplies {field}"
        );
        assert!(
            selector(
                report,
                "dataforgeDataMappingRange",
                &[0, u64::from(root.index)],
                field
            ),
            "the actual preceding mapping rows supply {field}"
        );
    }
    assert!(
        selector(report, "dataforgeStructDefinition", &[0], "size")
            && selector(report, "dataforgeStructDefinition", &[1], "size"),
        "the preceding block and selected Struct widths determine the original instance extent"
    );
    assert!(
        !selector(report, "dataforgeStructDefinition", &[2], "size"),
        "later mappings and unrelated original sizes cannot affect the offset"
    );
    Ok(())
}

pub(super) fn selector(
    report: &FieldStatisticsReport<'_>,
    want_ns: &str,
    want_indices: &[u64],
    want_field: &str,
) -> bool {
    report.inventory.iter_uses().any(|site| {
        site.role() == SourceUseRole::Selector
            && report.selection_origin.source_uses_refs().contains(site.id())
            && matches!(site.location(), SourceLocation::Subresource { namespace, indices, field }
                if namespace == want_ns && indices.as_slice() == want_indices && field == want_field)
    })
}

fn fixture() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Before").field("marker", TestType::Double))
        .structure(TestStruct::new("Selected").field("score", TestType::Int32))
        .structure(TestStruct::new("After").field("marker", TestType::Int64))
        .record(TestRecord::new(1, "Before", "Earlier", "libs/first.xml"))
        .record(
            TestRecord::new(2, "Selected", "True", "libs/true.xml")
                .value("score", TestValue::Int32(9)),
        )
        .record(
            TestRecord::new(3, "Selected", "False", "libs/false.xml")
                .value("score", TestValue::Int32(1)),
        )
        .record(
            TestRecord::new(4, "Selected", "Unreadable", "libs/unreadable.xml")
                .with_instance_index(u16::MAX),
        )
        .record(TestRecord::new(5, "After", "Later", "libs/after.xml"))
        .build()
}
