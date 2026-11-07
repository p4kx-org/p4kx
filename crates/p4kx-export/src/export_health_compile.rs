// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use crate::export_diagnostic::{
    EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT, ExportDiagnostic, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole, FailedExportRecord,
};
use crate::export_health::{
    CatalogPublicationMode, ExportHealth, ExportHealthStatus, ExportStageStatus, KindExportHealth,
};
use crate::export_ledger::{
    ArchiveInventoryProblem, ExportAccountingError, ExportLedger, SourceObservation,
};
use crate::{CatalogFailure, CraftingWarning, Kind, KindSummary};

#[cfg(test)]
mod tests;

type DiagnosticKey = (
    Option<usize>,
    ExportDiagnosticStage,
    crate::ExportDiagnosticCode,
    ExportDiagnosticSeverity,
    Option<ExportSourceRole>,
    Option<String>,
    ExportDiagnosticUnit,
);

struct Evidence {
    archive_inventory_problem: Option<ArchiveInventoryProblem>,
    inventories: BTreeSet<usize>,
    incomplete_inventories: BTreeSet<usize>,
    sources: Vec<SourceObservation>,
    incomplete_fields: BTreeSet<usize>,
    references: Vec<crate::ExportUnresolvedReference>,
    entries: Vec<crate::export_entry_ledger::ArchiveEntryObservation>,
    failed_entries: Vec<crate::FailedExportEntry>,
    failed: Vec<FailedExportRecord>,
    diagnostics: Vec<ExportDiagnostic>,
    skipped_rows: Vec<(Kind, usize)>,
}

impl Evidence {
    fn read(
        ledger: ExportLedger,
        failures: &[CatalogFailure],
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
        let (mut failed, mut diagnostics) = crate::export_health_records::failures(&sources);
        let derived = crate::export_health_derived::failures(ledger.derived_failures);
        failed.extend(derived.0);
        failed.extend(fields.failed);
        failed.extend(crate::export_health_fields::failed_diagnostics(
            &ledger.field_diagnostics,
        ));
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
        diagnostics.extend(
            warnings
                .iter()
                .flat_map(crate::export_health_warning::diagnostics),
        );
        validate_failures(failures, &failed)?;
        order_failures(&mut failed);
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

    fn kind(
        &self,
        kind: Kind,
        rows: &[KindSummary],
    ) -> Result<KindExportHealth, ExportAccountingError> {
        if !self.inventories.contains(&kind.index()) {
            return Err(ExportAccountingError::MissingInventory { kind });
        }
        let uncertain = self.archive_inventory_problem.is_some() || self.uncertain_selection(kind);
        let (read, projection) = self.kind_stages(kind, uncertain);
        let (counts, failed) = self.record_counts(kind)?;
        Ok(KindExportHealth {
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
            failed_records: failed,
            failed_entries: self
                .failed_entries
                .iter()
                .filter(|entry| entry.kind == kind)
                .cloned()
                .collect(),
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
    ) -> Result<(crate::ExportSourceCounts, Vec<FailedExportRecord>), ExportAccountingError> {
        let mut counts = crate::export_health_records::counts(kind, &self.sources)?;
        let failed: Vec<_> = self
            .failed
            .iter()
            .filter(|source| source.kind == Some(kind))
            .cloned()
            .collect();
        counts.failed_dependencies = failed
            .iter()
            .filter(|source| source.role == ExportSourceRole::Dependency)
            .map(|source| source.record_index)
            .collect::<BTreeSet<_>>()
            .len();
        Ok((counts, failed))
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

/// Compiles production observations after the exact final row counts are known.
pub(crate) fn compile(
    ledger: ExportLedger,
    rows: &[KindSummary],
    failures: &[CatalogFailure],
    warnings: &[CraftingWarning],
    mode: CatalogPublicationMode,
) -> Result<ExportHealth, ExportAccountingError> {
    let evidence = Evidence::read(ledger, failures, warnings)?;
    let kinds: Vec<_> = Kind::ALL
        .into_iter()
        .map(|kind| evidence.kind(kind, rows))
        .collect::<Result<_, _>>()?;
    let (read, projection) = stages(evidence.diagnostics.iter());
    Ok(ExportHealth {
        status: if evidence.diagnostics.iter().any(blocks) {
            ExportHealthStatus::Incomplete
        } else {
            ExportHealthStatus::Complete
        },
        read,
        projection,
        publication_mode: mode,
        unique_selected_source_records: evidence.selected(),
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
    legacy: &[CatalogFailure],
    failed: &[FailedExportRecord],
) -> Result<(), ExportAccountingError> {
    let known: BTreeSet<_> = failed.iter().map(|source| source.guid.as_str()).collect();
    if legacy
        .iter()
        .any(|failure| !known.contains(failure.guid.to_string().as_str()))
    {
        return Err(ExportAccountingError::UnobservedFailure);
    }
    Ok(())
}

pub(crate) fn aggregate(
    diagnostics: impl IntoIterator<Item = ExportDiagnostic>,
) -> Result<Vec<ExportDiagnostic>, ExportAccountingError> {
    let mut grouped = BTreeMap::<DiagnosticKey, ExportDiagnostic>::new();
    for mut diagnostic in diagnostics {
        let key = (
            diagnostic.kind.map(Kind::index),
            diagnostic.stage,
            diagnostic.code,
            diagnostic.severity,
            diagnostic.source_role,
            diagnostic.warning_code.clone(),
            diagnostic.count_unit,
        );
        if let Some(existing) = grouped.get_mut(&key) {
            existing.count = existing
                .count
                .checked_add(diagnostic.count)
                .ok_or(ExportAccountingError::CounterOverflow)?;
            existing.examples.append(&mut diagnostic.examples);
            examples(existing);
        } else {
            examples(&mut diagnostic);
            grouped.insert(key, diagnostic);
        }
    }
    Ok(grouped.into_values().collect())
}

fn examples(diagnostic: &mut ExportDiagnostic) {
    diagnostic.examples.sort();
    diagnostic.examples.dedup();
    diagnostic
        .examples
        .truncate(EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT);
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

fn order_failures(records: &mut Vec<FailedExportRecord>) {
    records.sort_by(|left, right| {
        left.record_index
            .cmp(&right.record_index)
            .then_with(|| left.kind.map(Kind::index).cmp(&right.kind.map(Kind::index)))
            .then_with(|| left.role.cmp(&right.role))
            .then_with(|| left.stage.cmp(&right.stage))
            .then_with(|| left.code.cmp(&right.code))
    });
    records.dedup();
}

fn sum(mut counts: impl Iterator<Item = usize>) -> Result<usize, ExportAccountingError> {
    counts.try_fold(0_usize, |total, count| {
        total
            .checked_add(count)
            .ok_or(ExportAccountingError::CounterOverflow)
    })
}
