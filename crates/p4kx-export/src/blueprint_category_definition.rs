// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// One physical fieldless `BlueprintCategoryRecord` definition with directly sourced metadata.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintCategoryDefinition<'source> {
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'source>,
}

/// Prepared category and its original nominal GUID for stable output ordering.
#[derive(Debug)]
pub(crate) struct BlueprintCategory<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) definition: BlueprintCategoryDefinition<'source>,
}
