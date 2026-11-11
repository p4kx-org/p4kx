// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUses, SourceValue};

use super::OriginalDatum;
use super::health_entry_cells::EntryCells;
use super::health_entry_observation::FailedEntryObservation;
use super::health_failed_entry::FailedExportEntry;
use super::health_text_capture::{CapturedText, SealedTexts};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError, HealthText};

/// Only retained Entry operations materialized before the report's single seal.
pub(super) struct RetainedEntry {
    observation: FailedEntryObservation,
    entry_index: OriginalDatum<usize>,
    container_entry_index: OriginalDatum<usize>,
    path: OriginalDatum<CapturedText>,
    field: OriginalDatum<CapturedText>,
    source_type: OriginalDatum<CapturedText>,
}

impl RetainedEntry {
    pub(super) fn read(
        observation: FailedEntryObservation,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<Self, CatalogError> {
        let receipt = observation.source.original()?;
        receipt.check_input(input)?;
        let texts = input.origins().health_texts();
        let [field, source_type] = observation.source.diagnosis.retain(receipt)?;
        let entry_index = receipt.index_fact(false)?;
        let container_entry_index = receipt.index_fact(true)?;
        let path = texts.capture_shared(receipt.path_fact()?)?;
        let field = texts.capture_shared(field)?;
        let source_type = texts.capture_shared(source_type)?;
        Ok(Self {
            observation,
            entry_index,
            container_entry_index,
            path,
            field,
            source_type,
        })
    }

    pub(super) const fn texts(&self) -> [&OriginalDatum<CapturedText>; 3] {
        [&self.path, &self.field, &self.source_type]
    }

    pub(super) fn finish(
        self,
        texts: &SealedTexts,
        sources: &SourceUses,
    ) -> Result<FailedExportEntry, ExportAccountingError> {
        let body = EntryCells {
            entry_index: self.entry_index.into_cell(sources),
            container_entry_index: self.container_entry_index.into_cell(sources),
            path: texts.cell(self.path, sources)?,
            field: texts.cell(self.field, sources)?,
            source_type: texts.cell(self.source_type, sources)?,
            target_guid: OriginalDatum::<HealthText>::observed(
                SourceValue::Absent,
                Vec::new(),
                true,
            )
            .into_cell(sources),
        };
        Ok(FailedExportEntry::new(&self.observation, body))
    }
}
