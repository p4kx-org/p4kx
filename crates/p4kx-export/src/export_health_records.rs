// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::export_health::{ExportSourceCounts, ExportSourceMode};
use crate::export_ledger::{ExportAccountingError, SourceDisposition, SourceObservation};
use crate::health_diagnostic_observation::DiagnosticObservation as ExportDiagnostic;
use crate::health_failed_observation::FailedRecordObservation as FailedExportRecord;
use crate::selection::Kind;

type SourceKey = (Option<usize>, u32, ExportSourceRole);

/// Deduplicates identical production observations and rejects contradictory dispositions.
pub(crate) fn sources(
    observations: Vec<SourceObservation>,
) -> Result<Vec<SourceObservation>, ExportAccountingError> {
    let mut sources = BTreeMap::<SourceKey, SourceObservation>::new();
    for source in observations {
        source.key()?;
        let key = (
            source.kind.map(Kind::index),
            source.record_index,
            source.role,
        );
        if let Some(previous) = sources.get_mut(&key) {
            previous.merge(source)?;
        } else {
            sources.insert(key, source);
        }
    }
    Ok(sources.into_values().collect())
}

/// Counts actual source dispositions in Record units.
pub(crate) fn counts(
    kind: Kind,
    sources: &[SourceObservation],
) -> Result<ExportSourceCounts, ExportAccountingError> {
    let mut counts = ExportSourceCounts::default();
    let selected = sources.iter().filter(|source| source.kind == Some(kind));
    for source in selected {
        accumulate(&mut counts, source.role, source.disposition);
    }
    reconcile(kind, counts)
}

pub(crate) fn accumulate(
    counts: &mut ExportSourceCounts,
    role: ExportSourceRole,
    disposition: SourceDisposition,
) {
    match role {
        ExportSourceRole::Dependency => {
            counts.dependencies += 1;
            counts.failed_dependencies += usize::from(disposition.failed());
        }
        ExportSourceRole::SelectionCandidate => {
            counts.failed_selection_candidates += usize::from(disposition.failed());
        }
        ExportSourceRole::Selected | ExportSourceRole::DerivedInput => {
            count_selected(counts, disposition);
        }
    }
}

pub(crate) fn reconcile(
    kind: Kind,
    counts: ExportSourceCounts,
) -> Result<ExportSourceCounts, ExportAccountingError> {
    let reconciled = counts
        .projected
        .checked_add(counts.intentionally_skipped)
        .and_then(|count| count.checked_add(counts.omitted_after_read_failure))
        .and_then(|count| count.checked_add(counts.omitted_after_projection_failure))
        .ok_or(ExportAccountingError::CounterOverflow)?;
    if reconciled != counts.selected {
        return Err(ExportAccountingError::Unreconciled { kind });
    }
    Ok(counts)
}

fn count_selected(counts: &mut ExportSourceCounts, disposition: SourceDisposition) {
    counts.selected += 1;
    counts.read_successful += usize::from(disposition.read_succeeded());
    match disposition {
        SourceDisposition::Projected => counts.projected += 1,
        SourceDisposition::SkippedBeforeRead | SourceDisposition::SkippedAfterRead => {
            counts.intentionally_skipped += 1;
        }
        SourceDisposition::ReadFailed => counts.omitted_after_read_failure += 1,
        SourceDisposition::ProjectionFailed => counts.omitted_after_projection_failure += 1,
    }
}

/// Returns the source unit declared by the actual reader of a kind.
pub(crate) const fn source_mode(kind: Kind) -> ExportSourceMode {
    match kind {
        Kind::HarvestableLocations | Kind::LootContainers | Kind::Shops | Kind::ShopInventories => {
            ExportSourceMode::ArchiveEntries
        }
        Kind::Paints
        | Kind::Loadouts
        | Kind::ContractShipSpawns
        | Kind::ContractNpcSpawns
        | Kind::ContractEntitySpawns
        | Kind::ContractLocationQueries
        | Kind::MissionPropertySets => ExportSourceMode::RecordInputs,
        _ => ExportSourceMode::Records,
    }
}

/// Returns the exact failed source identities and aggregate-ready source diagnostics.
pub(crate) fn failures(
    sources: &[SourceObservation],
) -> Result<(Vec<FailedExportRecord>, Vec<ExportDiagnostic>), ExportAccountingError> {
    let mut failed = Vec::new();
    let mut diagnostics = Vec::new();
    let failed_sources = sources.iter().filter(|source| source.disposition.failed());
    for source in failed_sources {
        let (stage, code) = stage(source);
        source.key()?;
        failed.push(FailedExportRecord::new(
            source.kind,
            source.subject().clone(),
            source.role,
            stage,
            code,
        )?);
        diagnostics.push(diagnostic(source, stage, code)?);
    }
    Ok((failed, diagnostics))
}

fn stage(source: &SourceObservation) -> (ExportDiagnosticStage, ExportDiagnosticCode) {
    let (stage, code) = if source.role == ExportSourceRole::SelectionCandidate {
        (
            ExportDiagnosticStage::Selection,
            ExportDiagnosticCode::SelectionFailure,
        )
    } else if source.disposition == SourceDisposition::ReadFailed {
        (
            ExportDiagnosticStage::SourceRead,
            ExportDiagnosticCode::SourceReadFailure,
        )
    } else if source.role == ExportSourceRole::DerivedInput {
        (
            ExportDiagnosticStage::DerivedProjection,
            ExportDiagnosticCode::DerivedProjectionFailure,
        )
    } else {
        (
            ExportDiagnosticStage::Projection,
            ExportDiagnosticCode::ProjectionFailure,
        )
    };
    (stage, source.diagnosis.code.unwrap_or(code))
}

fn diagnostic(
    source: &SourceObservation,
    stage: ExportDiagnosticStage,
    code: ExportDiagnosticCode,
) -> Result<ExportDiagnostic, ExportAccountingError> {
    Ok(ExportDiagnostic {
        kind: source.kind,
        stage,
        code,
        severity: ExportDiagnosticSeverity::Error,
        source_role: Some(source.role),
        warning_code: None,
        count: 1,
        count_unit: ExportDiagnosticUnit::Records,
        examples: vec![source.diagnosis.example(source.subject().clone())?],
    })
}
