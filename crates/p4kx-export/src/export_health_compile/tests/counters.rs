// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Kind;
use crate::export_diagnostic::ExportSourceRole::{DerivedInput, Selected};
use crate::export_health::{CatalogPublicationMode, ExportStageStatus};
use crate::export_health_compile;
use crate::export_ledger::{
    DerivedFailure, ExportAccountingError, ExportLedger, SourceDiagnosis, SourceDisposition,
};

use super::TestResult;

#[test]
fn one_source_record_can_emit_many_rows_without_inflating_record_skips() -> TestResult {
    let mut ledger = super::ledger();
    super::observe(
        &mut ledger,
        super::source(Kind::Contracts, 1, Selected, SourceDisposition::Projected),
    );
    ledger.skipped_rows(Kind::Contracts, 3);
    let report = export_health_compile::compile(
        ledger,
        &super::rows(Kind::Contracts, 7),
        &[],
        &[],
        CatalogPublicationMode::Normal,
    )?;
    let kind = report.kind(Kind::Contracts).ok_or("contract health")?;
    assert_eq!(
        kind.source_records.selected, 1,
        "one original Record was selected"
    );
    assert_eq!(
        kind.source_records.projected, 1,
        "one typed source projection completed"
    );
    assert_eq!(
        kind.source_records.intentionally_skipped, 0,
        "row skips are not Record skips"
    );
    assert_eq!(kind.written_rows, 7, "fanout is measured in rows");
    assert_eq!(
        kind.intentionally_skipped_rows, 3,
        "skipped authored rows retain their own unit"
    );
    Ok(())
}

#[test]
fn read_failures_and_projection_failures_have_distinct_dispositions() -> TestResult {
    let mut ledger = super::ledger();
    super::observe(
        &mut ledger,
        super::source(
            Kind::ContractTemplates,
            1,
            Selected,
            SourceDisposition::ReadFailed,
        ),
    );
    super::observe(
        &mut ledger,
        super::source(
            Kind::ContractTemplates,
            2,
            Selected,
            SourceDisposition::ProjectionFailed,
        ),
    );
    let report = export_health_compile::compile(
        ledger,
        &super::rows(Kind::ContractTemplates, 0),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let kind = report
        .kind(Kind::ContractTemplates)
        .ok_or("template health")?;
    assert_eq!(
        kind.source_records.selected, 2,
        "both actual source Records were selected"
    );
    assert_eq!(
        kind.source_records.read_successful, 1,
        "only the projection failure had a readable source"
    );
    assert_eq!(
        kind.source_records.omitted_after_read_failure, 1,
        "a failed canonical read has its own count"
    );
    assert_eq!(
        kind.source_records.omitted_after_projection_failure, 1,
        "a failed typed projection has its own count"
    );
    assert_eq!(
        kind.read,
        ExportStageStatus::Incomplete,
        "source-read failure remains visible"
    );
    Ok(())
}

#[test]
fn a_retained_ship_and_its_failed_loadout_are_not_an_omitted_ship() -> TestResult {
    let mut ledger = super::ledger();
    let ship = super::source(Kind::Ships, 1, Selected, SourceDisposition::Projected);
    super::observe(&mut ledger, ship.clone());
    super::observe(
        &mut ledger,
        super::source(
            Kind::Loadouts,
            1,
            DerivedInput,
            SourceDisposition::ProjectionFailed,
        ),
    );
    ledger.derived_failure(DerivedFailure::of_source(
        &ship,
        Kind::Ships,
        Kind::Loadouts,
        Selected,
        SourceDiagnosis::default(),
    )?);
    let report = export_health_compile::compile(
        ledger,
        &super::rows(Kind::Ships, 1),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let kind = report.kind(Kind::Ships).ok_or("ship health")?;
    assert_eq!(kind.written_rows, 1, "the original ship remains present");
    assert_eq!(
        kind.source_records.omitted_after_read_failure, 0,
        "its source was readable"
    );
    assert_eq!(
        kind.source_records.omitted_after_projection_failure, 0,
        "its original projection was retained"
    );
    assert_eq!(
        kind.failed_derived_projections, 1,
        "the failed loadout is a separate operation"
    );
    assert_eq!(
        kind.read,
        ExportStageStatus::Complete,
        "the ship read is not reclassified as failed"
    );
    Ok(())
}

#[test]
fn overlapping_kind_projections_count_one_unique_original_source() -> TestResult {
    let mut ledger = super::ledger();
    super::observe(
        &mut ledger,
        super::source(Kind::Ships, 1, Selected, SourceDisposition::Projected),
    );
    super::observe(
        &mut ledger,
        super::source(Kind::Crafters, 1, Selected, SourceDisposition::Projected),
    );
    let report = export_health_compile::compile(
        ledger,
        &super::rows(Kind::Ships, 1),
        &[],
        &[],
        CatalogPublicationMode::Normal,
    )?;
    assert_eq!(
        report.unique_selected_source_records, 1,
        "overlapping projections share their original physical Record"
    );
    assert_eq!(
        report.selected_kind_records, 2,
        "each kind retains its own source participation"
    );
    Ok(())
}

#[test]
fn successful_empty_kinds_require_a_completed_inventory() -> TestResult {
    let report = export_health_compile::compile(
        super::ledger(),
        &super::rows(Kind::Items, 0),
        &[],
        &[],
        CatalogPublicationMode::Normal,
    )?;
    assert!(
        report.is_complete(),
        "an enumerated empty selection is healthy"
    );
    assert_eq!(
        report.unique_selected_source_records, 0,
        "no source Records were selected"
    );
    let missing = export_health_compile::compile(
        ExportLedger::default(),
        &super::rows(Kind::Items, 0),
        &[],
        &[],
        CatalogPublicationMode::Normal,
    );
    assert!(
        matches!(missing, Err(ExportAccountingError::MissingInventory { .. })),
        "missing enumeration cannot masquerade as a successful empty kind"
    );
    Ok(())
}

#[test]
fn incompatible_source_dispositions_cannot_be_published() {
    let mut ledger = super::ledger();
    super::observe(
        &mut ledger,
        super::source(Kind::Items, 1, Selected, SourceDisposition::Projected),
    );
    super::observe(
        &mut ledger,
        super::source(Kind::Items, 1, Selected, SourceDisposition::ReadFailed),
    );
    let result = export_health_compile::compile(
        ledger,
        &super::rows(Kind::Items, 1),
        &[],
        &[],
        CatalogPublicationMode::Normal,
    );
    assert!(
        matches!(
            result,
            Err(ExportAccountingError::ConflictingDisposition { .. })
        ),
        "contradictory production facts must be rejected"
    );
}

#[test]
fn equal_guids_at_different_physical_indices_remain_distinct_sources() -> TestResult {
    let mut ledger = super::ledger();
    let good = super::source(Kind::Items, 1, Selected, SourceDisposition::Projected);
    let bad = crate::export_ledger::SourceObservation::accounting(
        good.kind,
        2,
        good.guid,
        good.record_path.clone(),
        good.role,
        SourceDisposition::ReadFailed,
    );
    super::observe(&mut ledger, good.clone());
    super::observe(&mut ledger, good);
    super::observe(&mut ledger, bad);
    let health = export_health_compile::compile(
        ledger,
        &super::rows(Kind::Items, 1),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let items = health.kind(Kind::Items).ok_or("item health")?;
    assert_eq!(
        health.unique_selected_source_records, 2,
        "physical Records sharing a GUID are not merged"
    );
    assert_eq!(
        items.source_records.selected, 2,
        "repeated observations of the same index are deduplicated"
    );
    assert_eq!(
        items.source_records.projected, 1,
        "the successful physical Record remains projected"
    );
    assert_eq!(
        items.source_records.omitted_after_read_failure, 1,
        "the unreadable physical Record remains omitted"
    );
    assert_eq!(
        health
            .failed_records
            .first()
            .ok_or("failed identity")?
            .key()
            .index(),
        2,
        "the exact failed physical identity is retained"
    );
    Ok(())
}

#[test]
fn attempted_but_truncated_inventories_are_explicit_lower_bounds() -> TestResult {
    let mut ledger = super::ledger();
    ledger.incomplete_inventory(Kind::LootContainers);
    let report = export_health_compile::compile(
        ledger,
        &super::rows(Kind::LootContainers, 0),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let kind = report.kind(Kind::LootContainers).ok_or("loot health")?;
    assert!(
        !kind.source_inventory_complete,
        "a known incomplete traversal is not a completed empty inventory"
    );
    assert_eq!(
        kind.source_entries.selected, 0,
        "unknown inputs are not estimated from missing output rows"
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == Some(Kind::LootContainers)
                && diagnostic.code == crate::ExportDiagnosticCode::IncompleteInventory),
        "the artifact explicitly explains why observed counts are lower bounds"
    );
    assert!(
        !report.permits_publication(CatalogPublicationMode::Normal),
        "truncated source enumeration blocks normal output"
    );
    Ok(())
}
