// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUses, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::health_record_capture::CapturedRecord;
use super::health_text_capture::{HealthTextCell, SealedTexts};
use crate::health_metadata::HealthTextRef;
use crate::{ExportAccountingError, HealthText, HealthTextView};

/// Sealed header occurrences whose semantic text views retain their own pairing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[schemars(transform = crate::schema_fields::every_field_required)]
pub(crate) struct RecordCells {
    #[serde(rename = "recordIndex")]
    index: SourcedValue<u32>,
    #[schemars(with = "SourcedValue<crate::guid_text::CatalogGuid>")]
    guid: SourcedValue<HealthText>,
    #[serde(rename = "recordPath")]
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    path: HealthTextCell,
    #[serde(rename = "sourceType")]
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    source_type: HealthTextCell,
}

impl RecordCells {
    pub(super) fn finish(
        record: CapturedRecord,
        texts: &SealedTexts,
        sources: &SourceUses,
    ) -> Result<Self, ExportAccountingError> {
        Ok(Self {
            index: record.record_index.into_cell(sources),
            guid: record
                .guid
                .map(|guid| HealthText::new(&guid.to_string()))?
                .into_cell(sources),
            path: texts.cell(record.record_path, sources)?,
            source_type: texts.cell(record.source_type, sources)?,
        })
    }

    pub(crate) const fn index(&self) -> &SourcedValue<u32> {
        &self.index
    }

    pub(crate) const fn guid(&self) -> &SourcedValue<HealthText> {
        &self.guid
    }

    pub(crate) const fn path(&self) -> &SourcedValue<HealthTextRef> {
        self.path.cell()
    }

    pub(crate) const fn source_type(&self) -> &SourcedValue<HealthTextRef> {
        self.source_type.cell()
    }

    pub(crate) fn path_text(&self) -> HealthTextView<'_> {
        self.path.view()
    }

    pub(crate) fn type_text(&self) -> HealthTextView<'_> {
        self.source_type.view()
    }
}
