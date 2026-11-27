// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::Kind;
use crate::catalog_read::original::FailedEntryObservation;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::export_entry_ledger::{ArchiveEntryObservation, EntryOperation};
use crate::export_health::ExportSourceCounts;
use crate::export_ledger::{ExportAccountingError, SourceDisposition};
use crate::health_diagnostic_observation::DiagnosticObservation as ExportDiagnostic;

/// Actual Archive Entry outcomes after physical and logical identity deduplication.
pub(crate) struct Entries {
    pub(crate) sources: Vec<ArchiveEntryObservation>,
    pub(crate) failed: Vec<FailedEntryObservation>,
    pub(crate) diagnostics: Vec<ExportDiagnostic>,
}

/// Rejects contradictory Entry dispositions without estimating reads from warning counts.
pub(crate) fn compile(
    observations: Vec<ArchiveEntryObservation>,
) -> Result<Entries, ExportAccountingError> {
    let mut unique = BTreeMap::new();
    for source in observations {
        check_verified_missing(&source)?;
        let key = (
            source.kind.index(),
            source.entry_index,
            source.container_entry_index,
            source.path.clone(),
            source.role,
            source.operation,
        );
        match unique.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(source);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                merge_occurrences(entry.get_mut(), source)?;
            }
        }
    }
    let sources: Vec<_> = unique.into_values().collect();
    let failed: Vec<_> = sources
        .iter()
        .filter(|source| source.disposition.failed())
        .map(failure)
        .collect();
    let mut diagnostics: Vec<_> = failed
        .iter()
        .map(|failure| ExportDiagnostic {
            kind: Some(failure.source.kind),
            stage: failure.stage,
            code: failure.code,
            severity: ExportDiagnosticSeverity::Error,
            source_role: Some(failure.source.role),
            warning_code: None,
            count: 1,
            count_unit: ExportDiagnosticUnit::EntryOperations,
            examples: Vec::new(),
        })
        .collect();
    diagnostics.extend(
        sources
            .iter()
            .filter(|source| source.disposition == SourceDisposition::VerifiedMissingReference)
            .map(missing_reference_warning),
    );
    Ok(Entries {
        sources,
        failed,
        diagnostics,
    })
}

fn merge_occurrences(
    prior: &mut ArchiveEntryObservation,
    mut current: ArchiveEntryObservation,
) -> Result<(), ExportAccountingError> {
    use SourceDisposition::{ReadFailed, VerifiedMissingReference};

    match (prior.disposition, current.disposition) {
        (VerifiedMissingReference, ReadFailed) => {
            prior.disposition = ReadFailed;
            prior.diagnosis = current.diagnosis.clone();
        }
        (ReadFailed, VerifiedMissingReference) => {
            current.disposition = ReadFailed;
            current.diagnosis = prior.diagnosis.clone();
        }
        _ => {}
    }
    if *prior != current {
        return Err(ExportAccountingError::ConflictingDisposition {
            kind: Some(current.kind),
        });
    }
    prior.merge(&current)
}

fn check_verified_missing(source: &ArchiveEntryObservation) -> Result<(), ExportAccountingError> {
    if source.disposition != SourceDisposition::VerifiedMissingReference {
        return Ok(());
    }
    let allowed_scope = matches!(
        (source.kind, source.role),
        (
            Kind::StarmapObjects | Kind::LootContainers,
            ExportSourceRole::Dependency | ExportSourceRole::DerivedInput,
        ) | (Kind::HarvestableLocations, ExportSourceRole::Dependency)
    );
    if source.operation == EntryOperation::Enumeration
        && allowed_scope
        && source.entry_index.is_none()
        && source.container_entry_index.is_none()
    {
        source.original()?;
        Ok(())
    } else {
        Err(ExportAccountingError::ConflictingDisposition {
            kind: Some(source.kind),
        })
    }
}

const fn missing_reference_warning(source: &ArchiveEntryObservation) -> ExportDiagnostic {
    ExportDiagnostic {
        kind: Some(source.kind),
        stage: ExportDiagnosticStage::DerivedProjection,
        code: ExportDiagnosticCode::VerifiedMissingReference,
        severity: ExportDiagnosticSeverity::Warning,
        source_role: Some(source.role),
        warning_code: None,
        count: 1,
        count_unit: ExportDiagnosticUnit::EntryOperations,
        examples: Vec::new(),
    }
}

fn failure(source: &ArchiveEntryObservation) -> FailedEntryObservation {
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
    } else {
        (
            ExportDiagnosticStage::Projection,
            ExportDiagnosticCode::ProjectionFailure,
        )
    };
    FailedEntryObservation {
        source: source.clone(),
        stage,
        code: source.diagnosis.code.unwrap_or(code),
    }
}

/// Counts enumerated and consumed Archive Entries in their own source unit.
pub(crate) fn counts(
    kind: Kind,
    entries: &[ArchiveEntryObservation],
) -> Result<crate::export_entry_counts::ExportEntryCounts, ExportAccountingError> {
    let mut counts = ExportSourceCounts::default();
    let mut physical = BTreeMap::new();
    let selected = entries.iter().filter(|source| source.kind == kind);
    for source in selected {
        let key = (
            source.entry_index,
            source.container_entry_index,
            source.path.as_str(),
            source.role,
        );
        physical
            .entry(key)
            .and_modify(|disposition: &mut SourceDisposition| {
                *disposition = required(*disposition, source.disposition);
            })
            .or_insert(source.disposition);
    }
    for ((_, _, _, role), disposition) in physical {
        crate::export_health_records::accumulate(&mut counts, role, disposition);
    }
    crate::export_health_records::reconcile(kind, counts).map(Into::into)
}

fn required(left: SourceDisposition, right: SourceDisposition) -> SourceDisposition {
    use SourceDisposition::{
        Projected, ProjectionFailed, ReadFailed, SkippedAfterRead, SkippedBeforeRead,
        VerifiedMissingReference,
    };
    if left == ReadFailed || right == ReadFailed {
        ReadFailed
    } else if left == ProjectionFailed || right == ProjectionFailed {
        ProjectionFailed
    } else if left == Projected || right == Projected {
        Projected
    } else if left == VerifiedMissingReference || right == VerifiedMissingReference {
        VerifiedMissingReference
    } else if left == SkippedAfterRead || right == SkippedAfterRead {
        SkippedAfterRead
    } else {
        SkippedBeforeRead
    }
}
