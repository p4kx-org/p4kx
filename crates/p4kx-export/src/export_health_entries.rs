// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::Kind;
use crate::export_diagnostic::{
    ExportDiagnostic, ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage,
    ExportDiagnosticUnit,
};
use crate::export_entry_ledger::{ArchiveEntryObservation, FailedExportEntry};
use crate::export_health::ExportSourceCounts;
use crate::export_ledger::{ExportAccountingError, SourceDisposition};

/// Actual Archive Entry outcomes after physical and logical identity deduplication.
pub(crate) struct Entries {
    pub(crate) sources: Vec<ArchiveEntryObservation>,
    pub(crate) failed: Vec<FailedExportEntry>,
    pub(crate) diagnostics: Vec<ExportDiagnostic>,
}

/// Rejects contradictory Entry dispositions without estimating reads from warning counts.
pub(crate) fn compile(
    observations: Vec<ArchiveEntryObservation>,
) -> Result<Entries, ExportAccountingError> {
    let mut unique = BTreeMap::new();
    for source in observations {
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
            std::collections::btree_map::Entry::Occupied(entry) => {
                if entry.get() != &source {
                    return Err(ExportAccountingError::ConflictingDisposition {
                        kind: Some(source.kind),
                    });
                }
            }
        }
    }
    let sources: Vec<_> = unique.into_values().collect();
    let failed: Vec<_> = sources
        .iter()
        .filter(|source| source.disposition.failed())
        .map(failure)
        .collect();
    let diagnostics = failed
        .iter()
        .map(|failure| ExportDiagnostic {
            kind: Some(failure.kind),
            stage: failure.stage,
            code: failure.code,
            severity: ExportDiagnosticSeverity::Error,
            source_role: Some(failure.role),
            warning_code: None,
            count: 1,
            count_unit: ExportDiagnosticUnit::EntryOperations,
            examples: Vec::new(),
        })
        .collect();
    Ok(Entries {
        sources,
        failed,
        diagnostics,
    })
}

fn failure(source: &ArchiveEntryObservation) -> FailedExportEntry {
    let (stage, code) = if source.disposition == SourceDisposition::ReadFailed {
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
    FailedExportEntry {
        operation: source.operation,
        kind: source.kind,
        entry_index: source.entry_index,
        container_entry_index: source.container_entry_index,
        path: source.path.clone(),
        role: source.role,
        stage,
        code: source.diagnosis.code.unwrap_or(code),
        field: source.diagnosis.field.clone(),
        source_type: source.diagnosis.source_type.clone(),
        target_guid: source.diagnosis.target_guid.map(|guid| guid.to_string()),
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
    };
    if left == ReadFailed || right == ReadFailed {
        ReadFailed
    } else if left == ProjectionFailed || right == ProjectionFailed {
        ProjectionFailed
    } else if left == Projected || right == Projected {
        Projected
    } else if left == SkippedAfterRead || right == SkippedAfterRead {
        SkippedAfterRead
    } else {
        SkippedBeforeRead
    }
}
