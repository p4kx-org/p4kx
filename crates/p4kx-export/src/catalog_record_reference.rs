// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::CatalogRecordContext;
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::schema_fields;

/// A Record link with independently sourced target-header fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct CatalogRecordReference {
    pub(crate) target_record_index: SourcedValue<u32>,
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_path: SourcedValue<String>,
    pub(crate) target_type: SourcedValue<String>,
    pub(crate) status: DefinitionStatus,
}

impl CatalogRecordReference {
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
            target_record_index: target.selector("$recordIndex", SourceValue::Present(index))?,
            guid: context.value(path, SourceValue::Present(guid))?,
            record_path: target
                .selector("$recordPath", SourceValue::Present(record_path.to_owned()))?,
            target_type: target.selector("$type", SourceValue::Present(target_type.to_owned()))?,
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
