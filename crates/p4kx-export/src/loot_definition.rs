// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

use crate::source_value_catalog::{SourceField, SourceNode};

/// Complete ordered source fields and values of one physical Loot Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LootDefinition<'source> {
    #[serde(skip)]
    pub(crate) physical_guid: CatalogGuid,
    #[serde(skip)]
    pub(crate) physical_path: &'source str,
    #[serde(skip)]
    pub(crate) numbers: Vec<CapturedNumber>,
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) fields: Vec<SourceField<'source>>,
    pub(crate) values: Vec<SourceNode<'source>>,
}
