// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// One physical category database with its ordered authored reference slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintCategoryList<'source> {
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) categories: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

/// A category list and its original nominal identity for stable output order.
#[derive(Debug)]
pub(crate) struct PreparedBlueprintCategoryList<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) declaration: BlueprintCategoryList<'source>,
}
