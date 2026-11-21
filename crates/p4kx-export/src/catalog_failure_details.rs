// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUses, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::health_failure_human::FailureHuman;
use super::health_record_capture::CapturedRecord;
use super::health_text_capture::{CapturedText, HealthTextCell, SealedTexts};
use super::{OriginalDatum, health_failure_reason};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::guid_text::CatalogGuid;
use crate::health_metadata::HealthTextRef;
use crate::{CatalogError, HealthText, HealthTextView, schema_fields};

/// Source cells of one retained catalog failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct CatalogFailureDetails {
    /// The original physical Record position.
    record_index: SourcedValue<u32>,
    /// The original canonical Record GUID.
    #[schemars(with = "SourcedValue<CatalogGuid>")]
    guid: SourcedValue<HealthText>,
    /// The original normalized filename, or its actual unreadable state.
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    record_path: HealthTextCell,
    /// The original technical Record type.
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    source_type: HealthTextCell,
    /// The original human failure and retained formatter contributions.
    #[schemars(with = "SourcedValue<HealthTextRef>")]
    reason: HealthTextCell,
    #[serde(skip)]
    #[schemars(skip)]
    human: FailureHuman,
}

/// Original failure facts waiting for deterministic text-index normalization.
pub(super) struct CapturedFailure {
    header: CapturedRecord,
    reason: OriginalDatum<CapturedText>,
}

impl CapturedFailure {
    pub(super) fn read(
        input: &OriginalCatalogInput<'_, '_>,
        failure: &CatalogReadFailure<'_>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            header: CapturedRecord::read(input, failure.record())?,
            reason: health_failure_reason::capture(input, failure)?,
        })
    }

    pub(super) const fn texts(&self) -> [&OriginalDatum<CapturedText>; 3] {
        [
            &self.header.record_path,
            &self.header.source_type,
            &self.reason,
        ]
    }
    pub(super) fn finish(
        self,
        texts: &SealedTexts,
        sources: &SourceUses,
    ) -> Result<CatalogFailureDetails, CatalogError> {
        let guid = self
            .header
            .guid
            .map(|guid| HealthText::new(&guid.to_string()))?
            .into_cell(sources);
        let record_path = texts.cell(self.header.record_path, sources)?;
        let source_type = texts.cell(self.header.source_type, sources)?;
        let reason = texts.cell(self.reason, sources)?;
        let human = FailureHuman::new(&guid, &record_path, &reason)?;
        Ok(CatalogFailureDetails {
            record_index: self.header.record_index.into_cell(sources),
            guid,
            record_path,
            source_type,
            reason,
            human,
        })
    }
}
impl CatalogFailureDetails {
    /// Borrows the original path occurrence without a separate report lookup.
    #[must_use]
    pub fn record_path_text(&self) -> HealthTextView<'_> {
        self.record_path.view()
    }

    /// Borrows the original technical type occurrence.
    #[must_use]
    pub fn source_type_text(&self) -> HealthTextView<'_> {
        self.source_type.view()
    }

    /// Borrows the retained formatter occurrence.
    #[must_use]
    pub fn reason_text(&self) -> HealthTextView<'_> {
        self.reason.view()
    }

    pub(crate) fn human_guid(&self) -> &str {
        self.human.guid()
    }
    pub(crate) fn human_path(&self) -> &str {
        self.human.path()
    }
    pub(crate) fn human_reason(&self) -> &str {
        self.human.reason()
    }

    /// Returns the original physical Record-position cell.
    #[must_use]
    pub const fn record_index(&self) -> &SourcedValue<u32> {
        &self.record_index
    }

    /// Returns the original canonical GUID cell.
    #[must_use]
    pub const fn guid(&self) -> &SourcedValue<HealthText> {
        &self.guid
    }

    /// Returns the original normalized filename cell.
    #[must_use]
    pub const fn record_path(&self) -> &SourcedValue<HealthTextRef> {
        self.record_path.cell()
    }

    /// Returns the original technical type cell.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<HealthTextRef> {
        self.source_type.cell()
    }

    /// Returns the semantic formatter-result cell.
    #[must_use]
    pub const fn reason(&self) -> &SourcedValue<HealthTextRef> {
        self.reason.cell()
    }
}
