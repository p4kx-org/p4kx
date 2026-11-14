// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};

use crate::Kind;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportSourceRole};
use crate::export_ledger::SourceDisposition;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn simple(kind: &str, name: &str, path: &str) -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new(kind))
        .record(TestRecord::new(
            0x6600_0000_0000_4000_8000_0000_0000_0020,
            kind,
            name,
            path,
        ))
        .build()
}

#[test]
fn a_readable_leftover_retains_a_post_read_intentional_skip() -> TestResult {
    let bytes = simple(
        "CraftingGameplayPropertyDef",
        "CraftingGameplayPropertyDef.synthetic_test",
        "libs/foundry/records/crafting/craftedproperties/synthetic_test.xml",
    );
    let forge = DataForge::open(&bytes)?;
    let read = super::read(&forge, None, None, None)?;
    let source = read
        .ledger
        .sources
        .first()
        .ok_or("source outcome missing")?;
    assert_eq!(
        source.role,
        ExportSourceRole::Selected,
        "the selected source remains in the catalog selection"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::SkippedAfterRead,
        "the public API actually read the leftover definition"
    );
    assert_eq!(
        read.data.properties.len(),
        1,
        "the public library result remains available for dependency use"
    );
    assert_eq!(
        read.ledger.field_inspections.len(),
        1,
        "the canonical tree was inspected"
    );
    Ok(())
}

#[test]
fn a_malformed_leftover_quantization_reassigns_its_single_field_proof() -> TestResult {
    let bytes = simple(
        "CraftingQualityQuantizationRecord",
        "CraftingQualityQuantizationRecord.synthetic_test",
        "libs/foundry/records/crafting/qualityquantization/synthetic_test.xml",
    );
    let forge = DataForge::open(&bytes)?;
    let read = super::read(&forge, None, None, None)?;
    assert!(
        read.ledger
            .sources
            .iter()
            .any(|source| source.role == ExportSourceRole::Selected
                && source.disposition == SourceDisposition::SkippedBeforeRead),
        "the predetermined catalog exclusion is independent of dependency projection success"
    );
    let failed = read
        .ledger
        .sources
        .iter()
        .find(|source| source.role == ExportSourceRole::Dependency)
        .ok_or("dependency projection failure missing")?;
    assert_eq!(
        failed.disposition,
        SourceDisposition::ProjectionFailed,
        "canonical reading succeeded before the typed shape rejection"
    );
    assert_eq!(
        failed.diagnosis.code,
        Some(ExportDiagnosticCode::MalformedSource),
        "typed failure diagnosis is retained"
    );
    assert_eq!(
        failed.diagnosis.field.as_deref(),
        Some("qualityQuantization"),
        "the exact typed field is retained"
    );
    assert_eq!(
        read.ledger.field_inspections.len(),
        1,
        "no duplicate canonical walk was performed"
    );
    assert_eq!(
        read.ledger
            .field_inspections
            .first()
            .ok_or("field proof missing")?
            .role,
        ExportSourceRole::Dependency,
        "the actual read proof follows its final dependency role"
    );
    Ok(())
}

#[test]
fn a_resource_without_links_is_still_an_actual_successful_dependency_read() -> TestResult {
    let bytes = simple(
        "ResourceType",
        "ResourceType.Synthetic",
        "libs/foundry/records/resourcetypedatabase/synthetic.xml",
    );
    let forge = DataForge::open(&bytes)?;
    let read = super::read(&forge, None, None, None)?;
    assert!(
        read.data.quality_quantizations.is_empty(),
        "a source without links emits no quantization rows"
    );
    let source = read
        .ledger
        .sources
        .first()
        .ok_or("ResourceType read missing")?;
    assert_eq!(
        source.kind,
        Some(Kind::QualityQuantizations),
        "the dependency belongs to quantization linkage"
    );
    assert_eq!(
        source.role,
        ExportSourceRole::Dependency,
        "ResourceType is not a selected quantization Record"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::Projected,
        "the actual successful zero-link branch completed"
    );
    assert_eq!(
        read.ledger.field_inspections.len(),
        1,
        "the dependency was inspected before its empty projection"
    );
    Ok(())
}

#[test]
fn cancellation_aborts_crafting_instead_of_completing_a_partial_inventory() -> TestResult {
    let bytes = super::fixtures::blueprint(0, false);
    let forge = DataForge::open(&bytes)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(
        matches!(
            super::read(&forge, None, Some(&cancel), None),
            Err(crate::CatalogError::Cancelled)
        ),
        "a cancelled production read does not return a complete ledger"
    );
    Ok(())
}
