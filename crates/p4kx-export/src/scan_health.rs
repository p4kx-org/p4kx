// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Record;

use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::picked::Picked;
use crate::selection::Kind;

/// Source observations and rows produced by one Record check.
pub(crate) struct RecordScan<'f> {
    pub(crate) picked: Option<Picked<'f>>,
    pub(crate) entity_tags: Option<crate::catalog_v3::tag_catalog::EntityTags<'f>>,
    pub(crate) ledger: ExportLedger,
}

/// Rows and production accounting of the generic catalog scan.
pub(crate) struct ScanRead<'f> {
    pub(crate) picked: Vec<Picked<'f>>,
    pub(crate) entity_tags: Vec<crate::catalog_v3::tag_catalog::EntityTags<'f>>,
    pub(crate) ledger: ExportLedger,
}

/// A source-Record failure remains separate from cancellation or accounting failure.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ScanError {
    #[error(transparent)]
    Source(#[from] p4kx_extraction::DataForgeError),
    #[error(transparent)]
    Fatal(#[from] crate::CatalogError),
    #[error(transparent)]
    Projection(#[from] crate::source_fields::FieldError),
}

/// Completes the production evidence captured by a Record worker.
pub(crate) fn finish<'f>(
    record: &Record<'_>,
    trace: &SourceTrace,
    result: Result<Option<Picked<'f>>, ScanError>,
    mut ledger: ExportLedger,
) -> Result<Option<RecordScan<'f>>, crate::CatalogError> {
    let picked = match result {
        Ok(picked) => {
            trace.completed(record, picked.as_ref(), &mut ledger);
            picked
        }
        Err(ScanError::Source(error)) => {
            trace.failed(record, &error, &mut ledger);
            Some(Picked::Failed(crate::CatalogFailure::of(record, &error)))
        }
        Err(ScanError::Fatal(error)) => return Err(error),
        Err(ScanError::Projection(error)) => {
            trace.failed(record, &error, &mut ledger);
            Some(Picked::Failed(crate::CatalogFailure::of(record, &error)))
        }
    };
    Ok(
        (picked.is_some() || !ledger.sources.is_empty() || !ledger.field_inspections.is_empty())
            .then_some(RecordScan {
                picked,
                entity_tags: None,
                ledger,
            }),
    )
}

/// The exact stage reached by one generic Record scan.
#[derive(Clone, Copy)]
enum Stage {
    Selection,
    SourceRead,
    Projection,
}

/// Source-stage evidence kept by the worker that reads and projects one Record.
pub(crate) struct SourceTrace {
    kind: Option<Kind>,
    stage: Stage,
    selected: bool,
}

impl SourceTrace {
    /// Creates a selection candidate without claiming that it is a selected source.
    pub(crate) fn new(record: &Record<'_>) -> Self {
        Self {
            kind: record.record_path().ok().and_then(|path| {
                record
                    .type_name()
                    .ok()
                    .and_then(|type_name| Kind::by_record(path, type_name))
            }),
            stage: Stage::Selection,
            selected: false,
        }
    }

    /// Records the kind whose source metadata is being checked.
    pub(crate) const fn candidate(&mut self, kind: Kind) {
        self.kind = Some(kind);
        self.stage = Stage::Selection;
    }

    /// Records that the source passed selection before its canonical read starts.
    pub(crate) const fn selected(&mut self, kind: Kind) {
        self.kind = Some(kind);
        self.stage = Stage::SourceRead;
        self.selected = true;
    }

    /// Records the successful canonical read before typed projection starts.
    pub(crate) const fn projecting(&mut self) {
        self.stage = Stage::Projection;
    }

    /// Captures the actual completed source disposition and any secondary crafter selection.
    pub(crate) fn completed(
        &self,
        record: &Record<'_>,
        picked: Option<&Picked<'_>>,
        ledger: &mut ExportLedger,
    ) {
        if !self.selected {
            return;
        }
        let disposition = picked.map_or(SourceDisposition::Projected, |picked| {
            disposition(picked, self.stage)
        });
        ledger.observe(self.observation(record, ExportSourceRole::Selected, disposition));
        if let Some(picked) = picked {
            additional(record, picked, ledger);
        }
    }

    /// Captures the stage of an actual failure without interpreting its display text.
    pub(crate) fn failed(
        &self,
        record: &Record<'_>,
        error: &(dyn std::error::Error + 'static),
        ledger: &mut ExportLedger,
    ) {
        let role = if self.selected {
            ExportSourceRole::Selected
        } else {
            ExportSourceRole::SelectionCandidate
        };
        let disposition = if matches!(self.stage, Stage::Projection) {
            SourceDisposition::ProjectionFailed
        } else {
            SourceDisposition::ReadFailed
        };
        ledger.observe(
            self.observation(record, role, disposition)
                .diagnosed(crate::export_source_failure::diagnosis(error)),
        );
    }

    fn observation(
        &self,
        record: &Record<'_>,
        role: ExportSourceRole,
        disposition: SourceDisposition,
    ) -> SourceObservation {
        self.kind.map_or_else(
            || SourceObservation::catalog_source(record, role, disposition),
            |kind| SourceObservation::new(kind, record, role, disposition),
        )
    }
}

fn disposition(picked: &Picked<'_>, stage: Stage) -> SourceDisposition {
    match picked {
        Picked::Failed(_) => SourceDisposition::ProjectionFailed,
        Picked::Skipped(..) if matches!(stage, Stage::Projection) => {
            SourceDisposition::SkippedAfterRead
        }
        Picked::SkippedPaint | Picked::Skipped(..) => SourceDisposition::SkippedBeforeRead,
        Picked::Crafted(picked, _) => disposition(picked, stage),
        _ => SourceDisposition::Projected,
    }
}

fn additional(record: &Record<'_>, picked: &Picked<'_>, ledger: &mut ExportLedger) {
    match picked {
        Picked::Crafted(picked, _) => {
            ledger.observe(SourceObservation::new(
                Kind::Crafters,
                record,
                ExportSourceRole::Selected,
                SourceDisposition::Projected,
            ));
            additional(record, picked, ledger);
        }
        Picked::SkippedPaint => ledger.observe(SourceObservation::new(
            Kind::Paints,
            record,
            ExportSourceRole::DerivedInput,
            SourceDisposition::SkippedAfterRead,
        )),
        _ => {}
    }
}
