// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::{DataForge, FieldStat};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::{SourceInput, SourceLocation, SourceUseRef, SourceUseRole};

use super::{FieldStatisticsReport, FieldStatisticsRowProof};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn uncapped_empty_text_keeps_the_original_distinct_exclusion() -> TestResult {
    let bytes = fixture(TestType::String, "");
    let forge = DataForge::open(&bytes)?;
    let report = FieldStatisticsReport::all(&forge, SourceInput::Unknown)?;
    let (stat, proof) = field(&report)?;
    let distinct = proof.distinct.source_uses_refs();
    assert_eq!(stat.distinct(), 0, "the empty text admits no fingerprint");
    assert!(
        has_field(&report, distinct, "dataforgeHeader", "textLength"),
        "the distinct exclusion consults the authored text bound"
    );
    assert!(
        has_field(&report, distinct, "dataforgeDecodedBytes", "slot"),
        "the actual original offset selects the empty text"
    );
    assert!(
        report.inventory.iter_uses().any(|site| {
            distinct.contains(site.id())
                && site.role() == SourceUseRole::Payload
                && matches!(site.location(), SourceLocation::Subresource {
                    namespace, field, indices,
                } if namespace == "dataforgeDecodedBytes" && field == "text"
                    && text_bytes(&bytes, indices) == Some([0].as_slice()))
        }),
        "the original empty text's NUL contributes to the distinct exclusion"
    );
    assert!(
        !has_field(
            &report,
            proof.structure.source_uses_refs(),
            "dataforgeHeader",
            "textLength",
        ),
        "the row path does not depend on decoding its string value"
    );
    Ok(())
}

#[test]
fn enum_value_retains_the_exact_count_target_and_text_inputs() -> TestResult {
    let bytes = fixture(TestType::Enum("State".into()), "Open");
    let forge = DataForge::open(&bytes)?;
    let report = FieldStatisticsReport::all(&forge, SourceInput::Unknown)?;
    let (stat, proof) = field(&report)?;
    assert_eq!(
        stat.distinct(),
        1,
        "the readable enum admits one fingerprint"
    );
    for refs in [
        proof.distinct.source_uses_refs(),
        proof.empty.source_uses_refs(),
    ] {
        for (namespace, field) in [
            ("dataforgeHeader", "enumCount"),
            ("dataforgeHeader", "textLength"),
            ("dataforgePropertyDefinition", "targetedInstanceLayout"),
            ("dataforgeDecodedBytes", "textTerminator"),
        ] {
            assert!(
                has_field(&report, refs, namespace, field),
                "the enum value uses its actual {namespace}/{field}"
            );
            assert!(
                !has_field(
                    &report,
                    proof.structure.source_uses_refs(),
                    namespace,
                    field
                ),
                "reading the enum {field} does not select the row path"
            );
        }
    }
    Ok(())
}

#[test]
fn invalid_enum_counts_original_enum_bound_without_reading_text() -> TestResult {
    let bytes = fixture(TestType::Enum("Unknown".into()), "Open");
    let forge = DataForge::open(&bytes)?;
    let report = FieldStatisticsReport::all(&forge, SourceInput::Unknown)?;
    let (_, proof) = field(&report)?;
    let checked = report.counters.unreadable_values.source_uses_refs();
    assert!(
        has_field(&report, checked, "dataforgeHeader", "enumCount"),
        "the failed enum read consulted the original enum count"
    );
    assert!(
        !has_field(&report, checked, "dataforgeHeader", "textLength"),
        "an invalid enum target never consults the text bound"
    );
    assert_eq!(
        proof.attempts.len(),
        1,
        "the original enum scalar slot was attempted"
    );
    Ok(())
}

#[test]
fn malformed_text_retains_actual_attempt_and_successful_read_bound_separately() -> TestResult {
    let original = "DamagedTextOperand";
    let mut bytes = fixture(TestType::String, original);
    let start = bytes
        .windows(original.len())
        .position(|span| span == original.as_bytes())
        .ok_or("the original fixture text exists")?;
    *bytes
        .get_mut(start)
        .ok_or("the first original text byte exists")? = u8::MAX;
    let forge = DataForge::open(&bytes)?;
    let report = FieldStatisticsReport::all(&forge, SourceInput::Unknown)?;
    let (_, proof) = field(&report)?;
    assert!(
        report.inventory.iter_uses().any(|site| {
            site.role() == SourceUseRole::Attempted
                && proof.attempts.contains(site.id())
                && report.counters.value_attempts.contains(site.id())
                && matches!(site.location(), SourceLocation::Subresource {
                    namespace, field, indices,
                } if namespace == "dataforgeDecodedBytes" && field == "text"
                    && text_bytes(&bytes, indices) == bytes.get(start..start + original.len() + 1))
        }),
        "the failed UTF-8 read retains its true text extent in both attempt scopes"
    );
    let checked = report.counters.unreadable_values.source_uses_refs();
    assert!(
        has_field(&report, checked, "dataforgeHeader", "textLength"),
        "the readable text bound belongs to the unreadable-value counter"
    );
    assert!(
        !has_field(
            &report,
            proof.distinct.source_uses_refs(),
            "dataforgeHeader",
            "textLength"
        ),
        "a failed text decode does not prove a distinct value"
    );
    Ok(())
}

fn field<'report>(
    report: &'report FieldStatisticsReport<'_>,
) -> Result<(&'report FieldStat, &'report FieldStatisticsRowProof), Box<dyn Error>> {
    report
        .reduction
        .statistics()
        .fields()
        .iter()
        .zip(&report.fields)
        .find(|(stat, _)| stat.path() == "scalar")
        .ok_or_else(|| "the authored scalar's statistics and row proof exist".into())
}

fn has_field(
    report: &FieldStatisticsReport<'_>,
    refs: &[SourceUseRef],
    expected_namespace: &str,
    expected_field: &str,
) -> bool {
    report.inventory.iter_uses().any(|site| {
        refs.contains(site.id())
            && matches!(
                site.location(),
                SourceLocation::Subresource { namespace, field, .. }
                    if namespace == expected_namespace && field == expected_field
            )
    })
}

fn text_bytes<'bytes>(bytes: &'bytes [u8], indices: &[u64]) -> Option<&'bytes [u8]> {
    let [start, end] = indices else { return None };
    bytes.get(usize::try_from(*start).ok()?..usize::try_from(*end).ok()?)
}

fn fixture(kind: TestType, value: &str) -> Vec<u8> {
    DataForgeBuilder::new()
        .enumeration("State", ["Open", "Closed"])
        .structure(TestStruct::new("ScalarOwner").field("scalar", kind))
        .record(
            TestRecord::new(1, "ScalarOwner", "Owner", "libs/scalars/one.xml")
                .value("scalar", TestValue::text(value)),
        )
        .build()
}
