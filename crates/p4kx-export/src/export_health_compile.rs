// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use crate::catalog_read::original::FailedEntryObservation;
use crate::export_diagnostic::{
    ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole,
};
use crate::export_health::{CatalogPublicationMode, ExportHealthStatus, ExportStageStatus};
pub(crate) use crate::export_health_aggregate::aggregate;
use crate::export_health_prepared::{KindFacts, PreparedHealth};
use crate::export_ledger::{
    ArchiveInventoryProblem, ExportAccountingError, ExportLedger, SourceObservation,
};
use crate::health_diagnostic_observation::DiagnosticObservation as ExportDiagnostic;
use crate::health_failed_observation::FailedRecordObservation as FailedExportRecord;
use crate::{CraftingWarning, Kind, KindSummary};

#[cfg(test)]
#[path = "export_health_compile/tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use crate::export_health_prepare::compile;

struct Evidence {
    archive_inventory_problem: Option<ArchiveInventoryProblem>,
    inventories: BTreeSet<usize>,
    incomplete_inventories: BTreeSet<usize>,
    sources: Vec<SourceObservation>,
    incomplete_fields: BTreeSet<usize>,
    references: Vec<crate::catalog_read::original::ReferenceObservation>,
    entries: Vec<crate::export_entry_ledger::ArchiveEntryObservation>,
    failed_entries: Vec<FailedEntryObservation>,
    failed: Vec<FailedExportRecord>,
    diagnostics: Vec<ExportDiagnostic>,
    skipped_rows: Vec<(Kind, usize)>,
}

impl Evidence {
    fn read(
        ledger: ExportLedger,
        failures: impl IntoIterator<Item = p4kx_extraction::Guid>,
        warnings: &[CraftingWarning],
    ) -> Result<Self, ExportAccountingError> {
        if ledger.conflicting_catalog_rows {
            return Err(ExportAccountingError::ConflictingCatalogRows);
        }
        let inventories = ledger.inventories.iter().map(|kind| kind.index()).collect();
        let incomplete_inventories: BTreeSet<_> = ledger
            .incomplete_inventories
            .into_iter()
            .map(Kind::index)
            .collect();
        let sources = crate::export_health_records::sources(ledger.sources)?;
        let fields = crate::export_health_fields::resolve(
            ledger.field_inspections,
            &sources,
            ledger.catalog_rows.as_ref(),
            &ledger.excluded_catalog_rows,
        )?;
        let entries = crate::export_health_entries::compile(ledger.entries)?;
        let (mut failed, mut diagnostics) = crate::export_health_records::failures(&sources)?;
        let derived = crate::export_health_derived::failures(ledger.derived_failures)?;
        failed.extend(derived.0);
        failed.extend(fields.failed);
        failed.extend(crate::export_health_fields::failed_diagnostics(
            &ledger.field_diagnostics,
        )?);
        diagnostics.extend(derived.1);
        diagnostics.extend(entries.diagnostics);
        diagnostics.extend(fields.diagnostics);
        diagnostics.extend(ledger.field_diagnostics);
        if let Some(problem) = ledger.archive_inventory_problem {
            diagnostics.push(incomplete_archive(problem));
        }
        diagnostics.extend(
            Kind::ALL
                .into_iter()
                .filter(|kind| incomplete_inventories.contains(&kind.index()))
                .map(incomplete_inventory),
        );
        diagnostics.extend(warnings.iter().flat_map(|warning| {
            crate::export_health_warning::diagnostics(warning, &ledger.template_warning_examples)
        }));
        validate_failures(failures, &failed)?;
        order_failures(&mut failed)?;
        Ok(Self {
            archive_inventory_problem: ledger.archive_inventory_problem,
            inventories,
            incomplete_inventories,
            sources,
            incomplete_fields: fields.incomplete_kinds,
            references: fields.references,
            entries: entries.sources,
            failed_entries: entries.failed,
            failed,
            diagnostics: aggregate(diagnostics)?,
            skipped_rows: ledger.skipped_rows,
        })
    }

    fn kind(&self, kind: Kind, rows: &[KindSummary]) -> Result<KindFacts, ExportAccountingError> {
        if !self.inventories.contains(&kind.index()) {
            return Err(ExportAccountingError::MissingInventory { kind });
        }
        let uncertain = self.archive_inventory_problem.is_some() || self.uncertain_selection(kind);
        let (read, projection) = self.kind_stages(kind, uncertain);
        let counts = self.record_counts(kind)?;
        Ok(KindFacts {
            kind,
            source_mode: crate::export_health_records::source_mode(kind),
            source_inventory_complete: !uncertain
                && !self.incomplete_inventories.contains(&kind.index()),
            field_inventory_complete: !self.incomplete_fields.contains(&kind.index()),
            read,
            projection,
            source_records: counts,
            source_entries: crate::export_health_entries::counts(kind, &self.entries)?,
            written_rows: rows
                .iter()
                .find(|row| row.kind == kind)
                .map_or(0, |row| row.count),
            intentionally_skipped_rows: sum(self
                .skipped_rows
                .iter()
                .filter(|(source, _)| *source == kind)
                .map(|(_, count)| *count))?,
            failed_derived_projections: self.derived_failures(kind)?,

            unresolved_reference_count: self
                .references
                .iter()
                .filter(|reference| reference.kind == Some(kind))
                .count(),
        })
    }

    fn kind_stages(&self, kind: Kind, uncertain: bool) -> (ExportStageStatus, ExportStageStatus) {
        if uncertain {
            return (ExportStageStatus::Incomplete, ExportStageStatus::Incomplete);
        }
        stages(
            self.diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.kind == Some(kind)),
        )
    }

    fn uncertain_selection(&self, kind: Kind) -> bool {
        self.sources.iter().any(|source| {
            source.role == ExportSourceRole::SelectionCandidate
                && source.disposition.failed()
                && source.kind.is_none_or(|source_kind| source_kind == kind)
        })
    }

    fn record_counts(
        &self,
        kind: Kind,
    ) -> Result<crate::ExportSourceCounts, ExportAccountingError> {
        let mut counts = crate::export_health_records::counts(kind, &self.sources)?;
        counts.failed_dependencies = self
            .failed
            .iter()
            .filter(|source| {
                source.kind == Some(kind) && source.role == ExportSourceRole::Dependency
            })
            .map(|source| source.key().index())
            .collect::<BTreeSet<_>>()
            .len();
        Ok(counts)
    }

    fn derived_failures(&self, kind: Kind) -> Result<usize, ExportAccountingError> {
        sum(self
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.kind == Some(kind)
                    && diagnostic.stage == ExportDiagnosticStage::DerivedProjection
                    && diagnostic.count_unit == ExportDiagnosticUnit::Projections
            })
            .map(|diagnostic| diagnostic.count))
    }

    fn selected(&self) -> usize {
        self.sources
            .iter()
            .filter(|source| {
                matches!(
                    source.role,
                    ExportSourceRole::Selected | ExportSourceRole::DerivedInput
                )
            })
            .map(|source| source.record_index)
            .collect::<BTreeSet<_>>()
            .len()
    }
}

const fn incomplete_inventory(kind: Kind) -> ExportDiagnostic {
    ExportDiagnostic {
        kind: Some(kind),
        stage: ExportDiagnosticStage::Selection,
        code: crate::ExportDiagnosticCode::IncompleteInventory,
        severity: ExportDiagnosticSeverity::Error,
        source_role: None,
        warning_code: None,
        count: 1,
        count_unit: ExportDiagnosticUnit::Occurrences,
        examples: Vec::new(),
    }
}

fn incomplete_archive(problem: ArchiveInventoryProblem) -> ExportDiagnostic {
    ExportDiagnostic {
        kind: None,
        stage: ExportDiagnosticStage::Selection,
        code: crate::ExportDiagnosticCode::IncompleteInventory,
        severity: ExportDiagnosticSeverity::Error,
        source_role: None,
        warning_code: Some(problem.warning_code().to_owned()),
        count: 1,
        count_unit: ExportDiagnosticUnit::Occurrences,
        examples: Vec::new(),
    }
}

/// Resolves accounting and retained occurrences before their one original report seal.
pub(crate) fn prepare_report(
    ledger: ExportLedger,
    rows: &[KindSummary],
    failures: impl IntoIterator<Item = p4kx_extraction::Guid>,
    warnings: &[CraftingWarning],
    mode: CatalogPublicationMode,
) -> Result<PreparedHealth, ExportAccountingError> {
    let evidence = Evidence::read(ledger, failures, warnings)?;
    let kinds = Kind::ALL
        .into_iter()
        .map(|kind| evidence.kind(kind, rows))
        .collect::<Result<Vec<_>, _>>()?;
    let (read, projection) = stages(evidence.diagnostics.iter());
    let status = if evidence.diagnostics.iter().any(blocks) {
        ExportHealthStatus::Incomplete
    } else {
        ExportHealthStatus::Complete
    };
    let selected = evidence.selected();
    let owners = evidence
        .sources
        .into_iter()
        .map(SourceObservation::into_subject)
        .collect();
    Ok(PreparedHealth {
        owners,
        entry_owners: evidence.entries,
        status,
        read,
        projection,
        publication_mode: mode,
        unique_selected_source_records: selected,
        selected_kind_records: sum(kinds.iter().map(|kind| kind.source_records.selected))?,
        written_rows: sum(kinds.iter().map(|kind| kind.written_rows))?,
        kinds,
        failed_records: evidence.failed,
        failed_entries: evidence.failed_entries,
        unresolved_references: evidence.references,
        diagnostics: evidence.diagnostics,
    })
}
fn validate_failures(
    legacy: impl IntoIterator<Item = p4kx_extraction::Guid>,
    failed: &[FailedExportRecord],
) -> Result<(), ExportAccountingError> {
    let known: BTreeSet<_> = failed.iter().map(|source| source.key().guid()).collect();
    if legacy.into_iter().any(|guid| !known.contains(&guid)) {
        return Err(ExportAccountingError::UnobservedFailure);
    }
    Ok(())
}

fn stages<'a>(
    diagnostics: impl Iterator<Item = &'a ExportDiagnostic>,
) -> (ExportStageStatus, ExportStageStatus) {
    let (mut read, mut projection) = (ExportStageStatus::Complete, ExportStageStatus::Complete);
    let blocking = diagnostics.filter(|diagnostic| blocks(diagnostic));
    for diagnostic in blocking {
        if matches!(
            diagnostic.stage,
            ExportDiagnosticStage::Selection
                | ExportDiagnosticStage::SourceRead
                | ExportDiagnosticStage::SourceField
        ) {
            read = ExportStageStatus::Incomplete;
        }
        projection = ExportStageStatus::Incomplete;
    }
    (read, projection)
}

fn blocks(diagnostic: &ExportDiagnostic) -> bool {
    diagnostic.severity != ExportDiagnosticSeverity::Information
}

fn order_failures(records: &mut Vec<FailedExportRecord>) -> Result<(), ExportAccountingError> {
    records.sort_by(|left, right| {
        left.key()
            .index()
            .cmp(&right.key().index())
            .then_with(|| left.kind.map(Kind::index).cmp(&right.kind.map(Kind::index)))
            .then_with(|| left.role.cmp(&right.role))
            .then_with(|| left.stage.cmp(&right.stage))
            .then_with(|| left.code.cmp(&right.code))
    });
    let mut retained: Vec<FailedExportRecord> = Vec::with_capacity(records.len());
    for record in records.drain(..) {
        if let Some(previous) = retained.last_mut().filter(|previous| *previous == &record) {
            previous.merge(record)?;
        } else {
            retained.push(record);
        }
    }
    *records = retained;
    Ok(())
}
fn sum(mut counts: impl Iterator<Item = usize>) -> Result<usize, ExportAccountingError> {
    counts.try_fold(0_usize, |total, count| {
        total
            .checked_add(count)
            .ok_or(ExportAccountingError::CounterOverflow)
    })
}
