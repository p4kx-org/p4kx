// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, Guid, OpenOptions};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{EquipmentCoverageFold, EquipmentDependencyCoverage, EquipmentFailureStage};
use crate::catalog_read::original::{EntryDiagnosis, RecordSubject};
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::export_entry_ledger::{ArchiveEntryObservation, EntryOperation};
use crate::export_field_inspection::{FieldInspection, InspectionFindings};
use crate::export_ledger::{
    DerivedFailure, ExportLedger, SourceDiagnosis, SourceDisposition, SourceObservation,
};
use crate::health_diagnostic_observation::DiagnosticObservation;
use crate::{CatalogError, ExportAccountingError, HealthText, Kind};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn folded_dependency_batches_preserve_record_order_entry_order_and_counts() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = Archive::open(archive_path(scratch.path())?)?;
    let batches = ledgers()?;
    let mut merged = ExportLedger::default();
    let mut fold = EquipmentCoverageFold::default();
    for batch in batches {
        fold.observe(&batch)?;
        merged.merge(batch);
    }
    let mut batch = EquipmentDependencyCoverage::default();
    let mut single = EquipmentDependencyCoverage::default();
    for coverage in [&mut batch, &mut single] {
        coverage.record_failure(4);
        coverage.source_incomplete(9);
    }
    fold.finish(&archive, &mut batch)?;
    single.finish(&archive, &merged)?;
    assert_eq!(batch.failed_records(), single.failed_records());
    assert_eq!(batch.incomplete_records(), single.incomplete_records());
    assert_eq!(batch.failed_entries(), single.failed_entries());
    assert_eq!(batch.incomplete_fields(), single.incomplete_fields());
    assert_eq!(batch.directory_complete(), single.directory_complete());
    assert_eq!(batch.failed_records(), [2, 4, 8]);
    assert_eq!(batch.incomplete_records(), [3, 7, 9]);
    assert_eq!(batch.incomplete_fields(), 9);
    assert!(
        batch.directory_complete(),
        "the intact Archive has complete directory coverage"
    );
    assert_eq!(
        batch
            .failed_entries()
            .iter()
            .map(|row| (row.path(), row.stage()))
            .collect::<Vec<_>>(),
        [
            ("z.xml", EquipmentFailureStage::SourceRead),
            ("a.xml", EquipmentFailureStage::Projection),
            ("z.xml", EquipmentFailureStage::Projection)
        ],
        "a repeated failure identity is omitted and known stages retain observation order"
    );
    Ok(())
}

#[test]
fn an_empty_fold_preserves_directory_scope() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = archive_path(scratch.path())?;
    let mut coverage = EquipmentDependencyCoverage::default();
    EquipmentCoverageFold::default().finish(&Archive::open(&path)?, &mut coverage)?;
    assert!(
        coverage.is_complete(),
        "the complete empty inventory has no missing dependencies"
    );
    let mut bytes = fs::read(&path)?;
    let end = bytes
        .windows(4)
        .rposition(|signature| signature == b"PK\x05\x06")
        .ok_or("outer end record")?;
    let zip64_end = bytes
        .windows(4)
        .rposition(|signature| signature == b"PK\x06\x06")
        .ok_or("outer ZIP64 end record")?;
    bytes
        .get_mut(zip64_end..end)
        .ok_or("outer ZIP64 end and locator range")?
        .fill(0);
    fs::write(&path, bytes)?;
    let archive = Archive::open_with(&path, OpenOptions::new().recover(true))?;
    assert!(crate::export_archive_health::inventory(&archive).is_some());
    let mut coverage = EquipmentDependencyCoverage::default();
    EquipmentCoverageFold::default().finish(&archive, &mut coverage)?;
    assert!(
        !coverage.directory_complete() && !coverage.is_complete(),
        "an incomplete directory cannot certify empty dependency coverage"
    );
    Ok(())
}

#[test]
fn field_count_overflow_is_detected_across_batches() -> TestResult {
    let mut fold = EquipmentCoverageFold::default();
    let mut ledger = ExportLedger::default();
    ledger.field_diagnostic(diagnostic(ExportDiagnosticSeverity::Warning, usize::MAX));
    fold.observe(&ledger)?;
    let mut ledger = ExportLedger::default();
    ledger.field_diagnostic(diagnostic(
        ExportDiagnosticSeverity::Information,
        usize::MAX,
    ));
    fold.observe(&ledger)?;
    ledger.field_diagnostic(diagnostic(ExportDiagnosticSeverity::Error, 1));
    assert!(
        matches!(
            fold.observe(&ledger),
            Err(CatalogError::Accounting(
                ExportAccountingError::CounterOverflow
            ))
        ),
        "only non-information counts participate in checked addition"
    );
    Ok(())
}

fn ledgers() -> TestResult<[ExportLedger; 2]> {
    let guid: Guid = "00000000-0000-4000-8000-000000000001".parse()?;
    let mut first = ExportLedger::default();
    first.entry(entry("z.xml", SourceDisposition::ReadFailed));
    first.observe(source(8, guid, SourceDisposition::ReadFailed));
    first.field_inspections.push(inspection(7, guid, false));
    first.field_diagnostic(diagnostic(ExportDiagnosticSeverity::Warning, 3));
    let mut second = ExportLedger::default();
    second.entry(entry("z.xml", SourceDisposition::ReadFailed));
    second.entry(entry("a.xml", SourceDisposition::ProjectionFailed));
    second.entry(entry("z.xml", SourceDisposition::ProjectionFailed));
    second.entry(entry("ok.xml", SourceDisposition::Projected));
    second.observe(source(8, guid, SourceDisposition::ReadFailed));
    second.observe(source(6, guid, SourceDisposition::Projected));
    second.derived_failure(DerivedFailure::of_source(
        &source(2, guid, SourceDisposition::Projected),
        Kind::Ships,
        Kind::Ships,
        ExportSourceRole::Dependency,
        SourceDiagnosis::EMPTY,
    )?);
    second.field_inspections.extend([
        inspection(7, guid, false),
        inspection(3, guid, false),
        inspection(6, guid, true),
    ]);
    second.field_diagnostic(diagnostic(ExportDiagnosticSeverity::Information, 20));
    second.field_diagnostic(diagnostic(ExportDiagnosticSeverity::Error, 6));
    Ok([first, second])
}

fn entry(path: &str, disposition: SourceDisposition) -> ArchiveEntryObservation {
    ArchiveEntryObservation {
        operation: EntryOperation::VehicleImplementation,
        kind: Kind::Ships,
        entry_index: Some(12),
        container_entry_index: None,
        path: HealthText::new(path),
        role: ExportSourceRole::Dependency,
        disposition,
        diagnosis: EntryDiagnosis::default(),
        receipt: None,
    }
}

fn source(index: u32, guid: Guid, disposition: SourceDisposition) -> SourceObservation {
    SourceObservation::accounting(
        Some(Kind::Ships),
        index,
        guid,
        None,
        ExportSourceRole::Dependency,
        disposition,
    )
}

fn inspection(record_index: u32, guid: Guid, complete: bool) -> FieldInspection {
    FieldInspection {
        kind: Some(Kind::Ships),
        record_index,
        guid,
        record_path: HealthText::new("ship.xml"),
        subject: RecordSubject::accounting(record_index, guid, None),
        role: ExportSourceRole::Dependency,
        complete,
        root_field: None,
        configuration: None,
        findings: InspectionFindings::default(),
        catalog_references: Vec::new(),
    }
}

fn diagnostic(severity: ExportDiagnosticSeverity, count: usize) -> DiagnosticObservation {
    DiagnosticObservation {
        kind: Some(Kind::Ships),
        stage: ExportDiagnosticStage::SourceField,
        code: ExportDiagnosticCode::UnreadableSource,
        severity,
        source_role: Some(ExportSourceRole::Dependency),
        warning_code: None,
        count,
        count_unit: ExportDiagnosticUnit::Values,
        examples: Vec::new(),
    }
}

fn archive_path(directory: &Path) -> TestResult<std::path::PathBuf> {
    let path = directory.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("fixture.xml", b"fixture".to_vec()))
        .write_to(&path)?;
    Ok(path)
}
