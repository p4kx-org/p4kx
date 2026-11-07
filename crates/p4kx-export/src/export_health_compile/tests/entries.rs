// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::export_entry_ledger::ArchiveEntryObservation;
use crate::export_ledger::{SourceDiagnosis, SourceDisposition};
use crate::{CatalogPublicationMode, EntryOperation, ExportSourceRole, Kind};

use super::{TestResult, ledger, rows};

fn entry(operation: EntryOperation, disposition: SourceDisposition) -> ArchiveEntryObservation {
    ArchiveEntryObservation {
        operation,
        kind: Kind::LootContainers,
        entry_index: Some(7),
        container_entry_index: None,
        path: "synthetic/container.socpak".to_owned(),
        role: ExportSourceRole::DerivedInput,
        disposition,
        diagnosis: SourceDiagnosis::default(),
    }
}

#[test]
fn independent_required_entry_operations_do_not_invent_extra_physical_entries() -> TestResult {
    let mut ledger = ledger();
    ledger.entry(entry(
        EntryOperation::ContainerDirectory,
        SourceDisposition::Projected,
    ));
    ledger.entry(entry(
        EntryOperation::SocTags,
        SourceDisposition::ProjectionFailed,
    ));
    let report = super::super::compile(
        ledger,
        &rows(Kind::LootContainers, 3),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let kind = report.kind(Kind::LootContainers).ok_or("loot health")?;
    assert_eq!(
        kind.source_entries.selected, 1,
        "required operations share one physical Entry identity"
    );
    assert_eq!(
        kind.source_entries.read_successful, 1,
        "the required byte read completed before projection failed"
    );
    assert_eq!(
        kind.source_entries.failed_projections, 1,
        "the failed parse operation remains visible"
    );
    assert_eq!(
        kind.written_rows, 3,
        "partial rows remain separate from Entry operation outcomes"
    );
    assert_eq!(
        report
            .failed_entries
            .first()
            .ok_or("failed entry")?
            .operation,
        EntryOperation::SocTags,
        "the exact failed operation is not hidden by a successful directory read"
    );
    Ok(())
}

#[test]
fn an_unexplored_entry_and_a_separate_successful_operation_remain_incomplete() -> TestResult {
    let mut ledger = ledger();
    ledger.entry(entry(
        EntryOperation::Enumeration,
        SourceDisposition::ReadFailed,
    ));
    ledger.entry(entry(EntryOperation::SocTags, SourceDisposition::Projected));
    let report = super::super::compile(
        ledger,
        &rows(Kind::LootContainers, 1),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let kind = report.kind(Kind::LootContainers).ok_or("loot health")?;
    assert_eq!(
        kind.source_entries.selected, 1,
        "read attempts do not multiply source units"
    );
    assert_eq!(
        kind.source_entries.failed_required_reads, 1,
        "an uncompleted required walk cannot become successful absence"
    );
    assert_eq!(
        kind.source_entries.projected, 0,
        "one successful operation cannot establish a complete Entry projection"
    );
    assert!(
        !report.is_complete(),
        "the artifact retains the incomplete required operation"
    );
    Ok(())
}

#[test]
fn identical_entry_operations_deduplicate_and_contradictory_same_operation_refuses() {
    let mut ledger = ledger();
    let source = entry(EntryOperation::SocTags, SourceDisposition::Projected);
    ledger.entry(source.clone());
    ledger.entry(source);
    ledger.entry(entry(
        EntryOperation::SocTags,
        SourceDisposition::ReadFailed,
    ));
    let result = super::super::compile(
        ledger,
        &rows(Kind::LootContainers, 1),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    );
    assert!(
        matches!(
            result,
            Err(crate::ExportAccountingError::ConflictingDisposition { .. })
        ),
        "contradictory outcomes of the same required operation are not hidden by aggregation"
    );
}

#[test]
fn failed_operation_counts_are_distinct_from_physical_entry_counts() -> TestResult {
    let mut ledger = ledger();
    for operation in [EntryOperation::EntityLinks, EntryOperation::SocTags] {
        let source = entry(operation, SourceDisposition::ProjectionFailed);
        ledger.entry(source.clone());
        ledger.entry(source);
    }
    let report = super::super::compile(
        ledger,
        &rows(Kind::LootContainers, 1),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let kind = report.kind(Kind::LootContainers).ok_or("loot health")?;
    assert_eq!(
        kind.source_entries.selected, 1,
        "one physical Entry is selected"
    );
    assert_eq!(
        kind.source_entries.failed_projections, 1,
        "both failed operations affect one physical Entry"
    );
    assert_eq!(
        report.failed_entries.len(),
        2,
        "each actual failed operation is retained once"
    );
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.kind == Some(Kind::LootContainers)
                && diagnostic.code == crate::ExportDiagnosticCode::ProjectionFailure
        })
        .ok_or("Entry projection diagnostic")?;
    assert_eq!(
        diagnostic.count, 2,
        "two distinct failed operations are aggregated"
    );
    assert_eq!(
        diagnostic.count_unit,
        crate::ExportDiagnosticUnit::EntryOperations,
        "operation counts never claim extra physical input files"
    );
    Ok(())
}
