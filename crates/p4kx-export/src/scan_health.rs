// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Record;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::catalog_v3::catalog_entity_configurations::EntityConfigurationRows;
use crate::catalog_v3::tag_catalog::EntityTags;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::picked::Picked;
use crate::record_observation::RecordObservation;
use crate::selection::Kind;

/// Source observations and rows produced by one Record check.
pub(crate) struct RecordScan<'f> {
    pub(crate) picked: Option<Picked<'f>>,
    pub(crate) entity: RecordEntityRows<'f>,
    pub(crate) ledger: ExportLedger,
}

/// Independent declarations from one selected Entity Record.
#[derive(Default)]
pub(crate) struct RecordEntityRows<'f> {
    pub(crate) tags: Option<EntityTags<'f>>,
    pub(crate) configurations: EntityConfigurationRows<'f>,
}

/// Rows and production accounting of the generic catalog scan.
pub(crate) struct ScanRead<'f> {
    pub(crate) picked: Vec<Picked<'f>>,
    pub(crate) entity_tags: Vec<EntityTags<'f>>,
    pub(crate) entity_configurations: EntityConfigurationRows<'f>,
    pub(crate) resource_group_members:
        Vec<crate::catalog_v3::resource_group_catalog::ResourceGroupMembers>,
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
    input: &OriginalCatalogInput<'_, '_>,
    record: &Record<'f>,
    trace: &SourceTrace,
    result: Result<Option<Picked<'f>>, ScanError>,
    mut ledger: ExportLedger,
) -> Result<Option<RecordScan<'f>>, crate::CatalogError> {
    let picked = match result {
        Ok(picked) => {
            trace.completed(input, record, picked.as_ref(), &mut ledger)?;
            picked
        }
        Err(ScanError::Source(error)) => {
            trace.failed(input, record, &error, &mut ledger)?;
            Some(Picked::Failed(CatalogReadFailure::of(record, &error)))
        }
        Err(ScanError::Fatal(error)) => return Err(error),
        Err(ScanError::Projection(error)) => {
            trace.failed(input, record, &error, &mut ledger)?;
            Some(Picked::Failed(CatalogReadFailure::of(record, &error)))
        }
    };
    Ok(
        (picked.is_some() || !ledger.sources.is_empty() || !ledger.field_inspections.is_empty())
            .then_some(RecordScan {
                picked,
                entity: RecordEntityRows::default(),
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
            kind: record
                .type_name()
                .ok()
                .and_then(|type_name| match type_name {
                    crate::catalog_v3::faction_definition::SOURCE_TYPE => Some(Kind::Factions),
                    crate::catalog_v3::inventory_container_definition::SOURCE_TYPE => {
                        Some(Kind::InventoryContainers)
                    }

                    crate::catalog_v3::reputation_perk_list::SOURCE_TYPE => {
                        Some(Kind::ReputationPerks)
                    }
                    crate::catalog_v3::sandbox_trigger_record_definition::SOURCE_TYPE => {
                        Some(Kind::ReputationTriggerRecords)
                    }
                    crate::catalog_v3::reputation_reward_amount_definition::SOURCE_TYPE => {
                        Some(Kind::ReputationRewardAmounts)
                    }
                    _ => crate::catalog_v3::law_kinds::kind_for_type(type_name)
                        .or_else(|| {
                            crate::catalog_v3::location_vocabulary::kind_for_type(type_name)
                        })
                        .or_else(|| {
                            record
                                .record_path()
                                .ok()
                                .and_then(|path| Kind::by_record(path, type_name))
                        }),
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
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
        picked: Option<&Picked<'_>>,
        ledger: &mut ExportLedger,
    ) -> Result<(), crate::CatalogError> {
        if !self.selected {
            return Ok(());
        }
        let disposition = picked.map_or(SourceDisposition::Projected, |picked| {
            disposition(picked, self.stage)
        });
        let admission = RecordObservation::capture(input, record)?;
        ledger.observe(admission.source(self.kind, ExportSourceRole::Selected, disposition)?);
        if let Some(picked) = picked {
            additional(&admission, picked, ledger)?;
        }
        Ok(())
    }

    /// Captures the stage of an actual failure without interpreting its display text.
    pub(crate) fn failed(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
        error: &(dyn std::error::Error + 'static),
        ledger: &mut ExportLedger,
    ) -> Result<(), crate::CatalogError> {
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
        let admission = RecordObservation::capture(input, record)?;
        ledger.observe(
            admission
                .source(self.kind, role, disposition)?
                .diagnosed(crate::export_source_failure::diagnosis(error)),
        );
        Ok(())
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

fn additional(
    admission: &RecordObservation<'_, '_>,
    picked: &Picked<'_>,
    ledger: &mut ExportLedger,
) -> Result<(), crate::CatalogError> {
    match picked {
        Picked::Crafted(picked, _) => {
            ledger.observe(admission.source(
                Some(Kind::Crafters),
                ExportSourceRole::Selected,
                SourceDisposition::Projected,
            )?);
            additional(admission, picked, ledger)?;
        }
        Picked::SkippedPaint => ledger.observe(admission.source(
            Some(Kind::Paints),
            ExportSourceRole::DerivedInput,
            SourceDisposition::SkippedAfterRead,
        )?),
        _ => {}
    }
    Ok(())
}
