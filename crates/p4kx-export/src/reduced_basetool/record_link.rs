// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::record_targets::RecordTargets;
use crate::Kind;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_reference;
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::source_value_catalog::SourceRecordReference;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecordLink<'source> {
    guid: Cell<&'source CatalogGuid>,
    target_record_index: Cell<&'source u32>,
    record_path: Cell<&'source str>,
    target_type: Cell<&'source str>,
    status: &'source DefinitionStatus,
}

impl<'source> From<&'source CatalogRecordReference> for RecordLink<'source> {
    fn from(link: &'source CatalogRecordReference) -> Self {
        Self {
            guid: (&link.guid).into(),
            target_record_index: link.target_record_index().into(),
            record_path: Cell::from(link.record_path()).map(String::as_str),
            target_type: Cell::from(link.target_type()).map(String::as_str),
            status: link.status(),
        }
    }
}

impl<'source> RecordLink<'source> {
    pub(super) fn from_source(
        path: &'static str,
        link: &'source SourceRecordReference<'_>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            guid: completeness::cell(path, &link.guid)?,
            target_record_index: completeness::cell(path, link.target_record_index())?,
            record_path: completeness::cell(path, link.record_path())?.map(|value| *value),
            target_type: completeness::cell(path, link.target_type())?.map(|value| *value),
            status: link.status(),
        })
    }

    pub(super) fn read_source(
        path: &'static str,
        link: &'source SourceRecordReference<'_>,
        targets: &RecordTargets,
        selected: impl FnOnce(&str) -> bool,
    ) -> Result<Self, ReducedError> {
        let reference = Self::from_source(path, link)?;
        reference.verify(path, targets, selected)?;
        Ok(reference)
    }

    pub(super) fn read(
        path: &'static str,
        link: &'source CatalogRecordReference,
        target_kind: Kind,
        targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        let reference = Self::from(link);
        reference.verify(path, targets, |source_type| {
            catalog_reference::actual_kind(source_type) == Some(target_kind)
        })?;
        Ok(reference)
    }

    fn verify(
        &self,
        path: &'static str,
        targets: &RecordTargets,
        selected: impl FnOnce(&str) -> bool,
    ) -> Result<(), ReducedError> {
        if !matches!(self.status, DefinitionStatus::Present) {
            return Ok(());
        }
        let source_type = self
            .target_type
            .value
            .ok_or(ReducedError::IncompleteScope(path))?;
        if selected(source_type) {
            let index = self
                .target_record_index
                .value
                .ok_or(ReducedError::IncompleteScope(path))?;
            let guid = self.guid.value.ok_or(ReducedError::IncompleteScope(path))?;
            targets.verify(path, *index, guid.get())?;
        }
        Ok(())
    }
}
