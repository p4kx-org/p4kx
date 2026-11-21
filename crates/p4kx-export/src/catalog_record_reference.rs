// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::CatalogRecordContext;
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::schema_fields;

/// A Record link with independently sourced target-header fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct CatalogRecordReference {
    pub(crate) target_record_index: SourcedValue<u32>,
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_path: SourcedValue<String>,
    pub(crate) target_type: SourcedValue<String>,
    pub(crate) status: DefinitionStatus,
}

impl CatalogRecordReference {
    /// Returns the exact target physical Record index, when known.
    #[must_use]
    pub const fn target_record_index(&self) -> &SourcedValue<u32> {
        &self.target_record_index
    }

    /// Returns the original referenced GUID.
    #[must_use]
    pub fn guid(&self) -> SourcedValue<Guid> {
        self.guid.map_ref(|guid| guid.0)
    }

    /// Returns the target Record path, when known.
    #[must_use]
    pub const fn record_path(&self) -> &SourcedValue<String> {
        &self.record_path
    }

    /// Returns the target Record type, when known.
    #[must_use]
    pub const fn target_type(&self) -> &SourcedValue<String> {
        &self.target_type
    }

    /// Returns the original target resolution state.
    #[must_use]
    pub const fn status(&self) -> &DefinitionStatus {
        &self.status
    }
    pub(crate) fn resolved(
        context: &CatalogRecordContext<'_>,
        path: &str,
        index: u32,
        guid: CatalogGuid,
        record_path: &str,
        target_type: &str,
    ) -> Result<Self, CatalogError> {
        let target = context.at_record(index);
        context.reference_observation(path)?;
        Ok(Self {
            target_record_index: target
                .header_selector(RecordHeaderField::RecordIndex, SourceValue::Present(index))?,
            guid: context.value(path, SourceValue::Present(guid))?,
            record_path: target.header_conversion(
                RecordHeaderField::FileName,
                SourceValue::Present(record_path.to_owned()),
            )?,
            target_type: target.header_selector(
                RecordHeaderField::SourceType,
                SourceValue::Present(target_type.to_owned()),
            )?,
            status: DefinitionStatus::Present,
        })
    }

    pub(crate) fn unresolved(
        context: &CatalogRecordContext<'_>,
        path: &str,
        guid: CatalogGuid,
        status: DefinitionStatus,
    ) -> Result<Self, CatalogError> {
        context.reference_observation(path)?;
        Ok(Self {
            target_record_index: context.selector(path, SourceValue::Absent)?,
            guid: context.value(path, SourceValue::Present(guid))?,
            record_path: context.selector(path, SourceValue::Absent)?,
            target_type: context.selector(path, SourceValue::Absent)?,
            status,
        })
    }
}
