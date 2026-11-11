// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::OriginalInstanceLocation;
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// A stored Reference whose retained target text borrows the original admitted input.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceRecordReference<'source> {
    pub(crate) target_record_index: SourcedValue<u32>,
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_path: SourcedValue<&'source str>,
    pub(crate) target_type: SourcedValue<&'source str>,
    pub(crate) status: DefinitionStatus,
}

/// A pointer-derived admitted target with independently sourced original facts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SourceWeakReference<'source> {
    pub(crate) target_record_index: SourcedValue<u32>,
    pub(crate) record_guid: SourcedValue<CatalogGuid>,
    pub(crate) record_path: SourcedValue<&'source str>,
    pub(crate) target: SourceWeakTarget<'source>,
}

/// Actual Record versus owned-instance applicability from the original admission.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum SourceWeakTarget<'source> {
    Record {
        source_type: SourcedValue<&'source str>,
    },
    Instance {
        source_type: SourcedValue<&'source str>,
        #[schemars(with = "SourcedValue<String>")]
        location: SourcedValue<OriginalInstanceLocation<'source>>,
    },
}
