// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::catalog_number_capture::FloatToken;
use crate::catalog_reference::CatalogReference;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_field_inspection::FieldInspection;
use crate::export_reference_issue::ExportReferenceStatus;

#[path = "export_field_reference_inventory_fixture.rs"]
mod fixture;

use fixture::{Case, TestResult};

#[test]
fn reference_count_keeps_every_original_slot_past_4096() -> TestResult {
    fixture::inspect(Case::Count, |inspection, bytes| {
        references(&inspection, bytes, Case::Count)
    })
}

#[test]
fn reference_metadata_keeps_every_original_slot_past_one_mebibyte() -> TestResult {
    fixture::inspect(Case::Metadata, |inspection, bytes| {
        references(&inspection, bytes, Case::Metadata)
    })
}

fn references(inspection: &FieldInspection, bytes: usize, case: Case) -> TestResult {
    assert_eq!(
        inspection.catalog_references.len(),
        case.references(),
        "all original resolved Reference metadata survives"
    );
    assert!(
        inspection.complete,
        "the Inspector visits every materialized source value despite represented field errors"
    );
    for (ordinal, reference) in inspection.catalog_references.iter().enumerate() {
        assert_eq!(
            reference.original.data().field.as_str(),
            format!("links[{ordinal}]"),
            "original array order is retained"
        );
        assert_eq!(
            reference.target()?.index,
            1,
            "every retained target has its actual physical header"
        );
    }
    state(inspection, bytes)
}

fn state(inspection: &FieldInspection, original_bytes: usize) -> TestResult {
    use std::io::Write as _;

    let mut logical_utf8 = 0usize;
    let mut field_utf8 = 0usize;
    for reference in &inspection.catalog_references {
        let data = reference.original.data();
        let target = reference.target()?;
        for value in [
            data.field.as_str(),
            data.source_type.as_str(),
            target.path,
            target.source_type,
        ] {
            logical_utf8 = logical_utf8
                .checked_add(value.len())
                .ok_or("metadata length overflow")?;
        }
        field_utf8 = field_utf8
            .checked_add(data.field.as_str().len())
            .ok_or("metadata capacity overflow")?;
    }
    let shared_utf8 = shared_metadata(inspection)?;
    let item = size_of::<CatalogReference>();
    let capacity = inspection.catalog_references.capacity();
    let vector_bytes = capacity
        .checked_mul(item)
        .ok_or("vector capacity overflow")?;
    writeln!(
        std::io::stderr().lock(),
        "CATALOG_REFERENCE_STATE count={} logical_utf8={} field_utf8={} shared_utf8={} vec_capacity={} item_size={} vector_bytes={} original_bytes={}",
        inspection.catalog_references.len(),
        logical_utf8,
        field_utf8,
        shared_utf8,
        capacity,
        item,
        vector_bytes,
        original_bytes
    )?;
    Ok(())
}

fn shared_metadata(inspection: &FieldInspection) -> TestResult<usize> {
    use std::collections::HashSet;

    let first = inspection
        .catalog_references
        .first()
        .ok_or("original retained references")?;
    let first_data = first.original.data();
    let first_target = first.target()?;
    let mut texts = HashSet::new();
    for reference in &inspection.catalog_references {
        let data = reference.original.data();
        let target = reference.target()?;
        for (actual, expected) in [
            (data.source_type.as_str(), first_data.source_type.as_str()),
            (target.path, first_target.path),
            (target.source_type, first_target.source_type),
        ] {
            assert!(
                std::ptr::eq(actual, expected),
                "each repeated original metadata spelling shares its retained allocation"
            );
            texts.insert(actual);
        }
    }
    texts.into_iter().try_fold(0usize, |total, text| {
        total
            .checked_add(text.len())
            .ok_or_else(|| "shared metadata length overflow".into())
    })
}

#[test]
fn original_errors_after_reference_count_boundary_remain_inspected() -> TestResult {
    fixture::inspect(Case::Count, |inspection, _| errors(&inspection))
}

#[test]
fn original_errors_after_reference_metadata_boundary_remain_inspected() -> TestResult {
    fixture::inspect(Case::Metadata, |inspection, _| errors(&inspection))
}

fn errors(inspection: &FieldInspection) -> TestResult {
    let dangling: Vec<_> = inspection
        .unresolved_references
        .iter()
        .filter(|reference| reference.data().field.as_str() == "lateDangling")
        .collect();
    assert_eq!(
        dangling.len(),
        1,
        "the later actual dangling token is retained independently of diagnostic sampling"
    );
    let dangling = dangling.first().ok_or("one retained dangling Reference")?;
    assert_eq!(
        dangling.status(),
        Some(ExportReferenceStatus::Dangling),
        "the actual decoder state survives the former cutoff"
    );
    assert!(
        inspection
            .diagnostics
            .iter()
            .any(
                |diagnostic| diagnostic.code == ExportDiagnosticCode::UnreadableSource
                    && diagnostic
                        .examples
                        .iter()
                        .any(|example| example.field.as_deref() == Some("lateBad"))
            ),
        "the later actual InvalidBool still contributes its blocking diagnostic"
    );
    Ok(())
}

#[test]
fn original_nonfinite_after_reference_count_boundary_keeps_exact_requirement() -> TestResult {
    fixture::inspect(Case::Count, |inspection, _| {
        number(&inspection, Case::Count)
    })
}

#[test]
fn original_nonfinite_after_reference_metadata_boundary_keeps_exact_requirement() -> TestResult {
    fixture::inspect(Case::Metadata, |inspection, _| {
        number(&inspection, Case::Metadata)
    })
}

fn number(inspection: &FieldInspection, case: Case) -> TestResult {
    assert_eq!(
        inspection.number_requirements.len(),
        1,
        "the later original nonfinite cannot bypass final raw coverage checks"
    );
    let number = inspection
        .number_requirements
        .first()
        .ok_or("one original nonfinite requirement")?;
    assert_eq!(
        number.record_index, 0,
        "the genuine original physical owner"
    );
    assert_eq!(number.field, "lateNumber", "the genuine later field");
    assert_eq!(
        number.source_type, "Source",
        "the actual original owner type"
    );
    assert_eq!(
        number.token,
        FloatToken::Single(fixture::NONFINITE_BITS),
        "original width and payload bits"
    );
    assert_eq!(
        number.original_ordinal,
        u64::try_from(case.references())? + 3,
        "root-field-first DFS counts the array, all slots and prior errors"
    );
    assert_eq!(
        number.contract, None,
        "generic raw occurrence coverage is not inferred from a catalog kind"
    );
    Ok(())
}
