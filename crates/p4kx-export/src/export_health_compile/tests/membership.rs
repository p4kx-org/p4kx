// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Kind;
use crate::export_health::{CatalogPublicationMode, ExportStageStatus};
use crate::export_health_compile;
use crate::export_ledger::SourceDisposition;

use super::TestResult;

#[test]
fn unreadable_metadata_invalidates_membership_without_multiplying_sources() -> TestResult {
    let mut ledger = super::ledger();
    let mut candidate = super::source(
        Kind::Items,
        1,
        crate::ExportSourceRole::SelectionCandidate,
        SourceDisposition::ReadFailed,
    );
    candidate.kind = None;
    candidate.record_path = None;
    ledger.observe(candidate);
    let report = export_health_compile::compile(
        ledger,
        &super::rows(Kind::Items, 0),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    for kind in &report.kinds {
        assert!(
            !kind.source_inventory_complete,
            "unreadable global metadata leaves membership unknown for {}",
            kind.kind
        );
        assert_eq!(
            kind.read,
            ExportStageStatus::Incomplete,
            "unknown membership cannot prove a complete kind read"
        );
        assert_eq!(
            kind.source_records.selected, 0,
            "uncertain membership does not invent a selected source"
        );
        assert_eq!(
            kind.source_records.failed_selection_candidates, 0,
            "one unknown candidate is not cloned into every kind's counters"
        );
    }
    assert_eq!(
        report.failed_records.len(),
        1,
        "the single physical unknown source remains globally identified"
    );
    assert_eq!(
        report
            .failed_records
            .first()
            .ok_or("global failed source")?
            .kind,
        None,
        "the unknown kind is not guessed from missing metadata"
    );
    Ok(())
}

#[test]
fn a_failed_global_dependency_does_not_invent_unknown_selection_membership() -> TestResult {
    let mut ledger = super::ledger();
    let mut dependency = super::source(
        Kind::Items,
        1,
        crate::ExportSourceRole::Dependency,
        SourceDisposition::ReadFailed,
    );
    dependency.kind = None;
    ledger.observe(dependency);
    let report = export_health_compile::compile(
        ledger,
        &super::rows(Kind::Items, 0),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    assert!(
        !report.is_complete(),
        "the global required dependency still blocks publication"
    );
    assert!(
        report
            .kinds
            .iter()
            .all(|kind| kind.source_inventory_complete),
        "a known dependency failure does not invent a selection-enumeration failure"
    );
    assert!(
        !report.permits_publication(CatalogPublicationMode::Normal),
        "catalog-wide health remains required even when individual inventories are known"
    );
    Ok(())
}

#[test]
fn a_root_recovery_limit_is_global_evidence_without_fabricated_sources() -> TestResult {
    let mut ledger = super::ledger();
    ledger.archive_inventory_problem =
        Some(crate::export_ledger::ArchiveInventoryProblem::RecoveryLimit);
    ledger.merge(crate::export_ledger::ExportLedger::default());
    let report = export_health_compile::compile(
        ledger,
        &super::rows(Kind::Items, 0),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    assert!(
        report
            .kinds
            .iter()
            .all(|kind| !kind.source_inventory_complete),
        "a root scan limit leaves required source membership unknown"
    );
    assert_eq!(
        report.unique_selected_source_records, 0,
        "the global marker is not a fake selected Record"
    );
    assert!(
        report.failed_records.is_empty() && report.failed_entries.is_empty(),
        "no Entry zero or Record identity is assigned to the root marker"
    );
    let diagnostic = report.diagnostics.first().ok_or("global diagnostic")?;
    assert_eq!(
        report.diagnostics.len(),
        1,
        "the root reason is retained once instead of per-kind copies"
    );
    assert_eq!(
        diagnostic.warning_code.as_deref(),
        Some("archive_recovery_limit"),
        "the typed source limit keeps its stable technical code"
    );
    assert_eq!(
        diagnostic.count, 1,
        "the count describes one root condition rather than guessed lost Entries"
    );
    Ok(())
}
