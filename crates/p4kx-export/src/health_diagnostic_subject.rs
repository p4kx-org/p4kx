// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;

use p4kx_extraction::{SourceUses, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::health_cell_order;
use super::health_header_receipt::CapturedSubject;
use super::health_text_capture::HealthTextCell;
use super::health_text_capture::SealedTexts;
use crate::ExportAccountingError;
use crate::guid_text::CatalogGuid;
use crate::health_metadata::HealthTextRef;
use crate::{HealthText, HealthTextView, schema_fields};

/// The three original subject cells carried by one sealed diagnostic occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct DiagnosticSubject {
    #[serde(rename = "recordIndex")]
    index: SourcedValue<u32>,
    #[schemars(with = "SourcedValue<CatalogGuid>")]
    #[serde(rename = "recordGuid")]
    guid: SourcedValue<HealthText>,
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    #[serde(rename = "recordPath")]
    path: HealthTextCell,
}

impl DiagnosticSubject {
    pub(super) fn finish(
        record: CapturedSubject,
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
        })
    }

    pub(super) fn absent(sources: &SourceUses) -> Self {
        Self {
            index: sources.value(p4kx_extraction::SourceValue::Absent, &[], true),
            guid: sources.value(p4kx_extraction::SourceValue::Absent, &[], true),
            path: HealthTextCell::absent(sources),
        }
    }
    pub(crate) const fn record_index(&self) -> &SourcedValue<u32> {
        &self.index
    }

    pub(crate) const fn record_guid(&self) -> &SourcedValue<HealthText> {
        &self.guid
    }

    pub(crate) const fn record_path(&self) -> &SourcedValue<HealthTextRef> {
        self.path.cell()
    }

    pub(crate) fn record_path_text(&self) -> HealthTextView<'_> {
        self.path.view()
    }
}

impl PartialOrd for DiagnosticSubject {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DiagnosticSubject {
    fn cmp(&self, other: &Self) -> Ordering {
        health_cell_order::sourced(&self.index, &other.index)
            .then_with(|| health_cell_order::sourced(&self.guid, &other.guid))
            .then_with(|| health_cell_order::text(&self.path, &other.path))
    }
}
