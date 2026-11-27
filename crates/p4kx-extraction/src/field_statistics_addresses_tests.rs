// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::{FieldPath, Filter, SourceInput, SourceLocation, SourceUseRole};

use super::FieldStatisticsReport;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn filtered_addresses_reuse_the_exact_selected_original_row() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let report = FieldStatisticsReport::all(&forge, SourceInput::Unknown)?;
    let filter = Filter::parse("zSelected")?;
    let displayed = report.addressed_origins(Some(&filter))?;
    let [ordinal] = displayed.ordinals.as_slice() else {
        return Err("one displayed statistic was selected".into());
    };
    assert!(
        *ordinal > 0,
        "the earlier original row was excluded before display ordinals are assigned"
    );
    let selected = report
        .fields
        .get(*ordinal)
        .ok_or("the original proof exists")?;
    for (path, proof) in [
        ("/fields/0/path", &selected.structure),
        ("/fields/0/kinds/0/kind", &selected.structure),
        ("/fields/0/kinds/0/count", &selected.structure),
        ("/fields/0/max", &selected.extrema),
        ("/fields/0/empty", &selected.empty),
        ("/fields/0/distinct_capped", &selected.distinct),
    ] {
        assert_eq!(
            displayed.fields.get(&FieldPath::parse(path)?),
            Some(proof),
            "the existing JSON address has the exact original metric proof"
        );
    }
    assert!(
        displayed
            .fields
            .get(&FieldPath::parse("/fields/1/max")?)
            .is_none(),
        "origins for filtered out rows are not emitted"
    );
    assert!(
        displayed
            .fields
            .get(&FieldPath::parse("/max_distinct")?)
            .is_none(),
        "the tool-authored algorithm parameter has no original input"
    );
    Ok(())
}

#[test]
fn summary_keeps_unreadable_value_attempts_for_paths_without_output() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let report = FieldStatisticsReport::all(&forge, SourceInput::Unknown)?;
    assert_eq!(
        report.reduction.statistics().unreadable_values(),
        1,
        "the original failed array header contributes one actual skip"
    );
    assert!(
        report
            .reduction
            .statistics()
            .fields()
            .iter()
            .all(|field| field.path() != "orphan"),
        "the failed array supplies no counted field row"
    );
    let [attempt] = report.counters.value_attempts.as_slice() else {
        return Err("the non-emitted path contributes one failed original use".into());
    };
    let description = report
        .inventory
        .iter_uses()
        .find(|site| site.id() == attempt)
        .ok_or("the failed original use resolves")?;
    assert!(
        description.role() == SourceUseRole::Attempted
            && matches!(
                description.location(),
                SourceLocation::Subresource { namespace, field, .. }
                    if namespace == "dataforgeInstance" && field == "arrayExtent"
            ),
        "the summary points to the exact failed source position"
    );
    assert!(
        !report
            .counters
            .unreadable_values
            .source_uses_refs()
            .contains(attempt),
        "an attempted input stays out of the summary's successful source-use set"
    );
    assert!(
        !report.counters.field_count.source_set_complete(),
        "unreadable original structure may conceal paths outside the retained rows"
    );
    Ok(())
}

fn fixture() -> Vec<u8> {
    let mut array_header = 1_u32.to_le_bytes().to_vec();
    array_header.extend_from_slice(&0_u32.to_le_bytes());
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Owner")
                .field("aIgnored", TestType::Int32)
                .array("orphan", TestType::UInt8)
                .field("zSelected", TestType::Int32),
        )
        .record(
            TestRecord::new(1, "Owner", "Owner", "synthetic/owner.xml")
                .value("aIgnored", TestValue::Int32(8))
                .value("zSelected", TestValue::Int32(11))
                .value("orphan", TestValue::Raw(array_header)),
        )
        .build()
}
