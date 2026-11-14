// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// One physical Manufacturer definition with directly sourced authored facts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ManufacturerDefinition<'source> {
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) code: SourcedValue<&'source str>,
    pub(crate) localization: SourcedValue<ManufacturerLocalization<'source>>,
}

/// The Manufacturer's owned localization declaration and its canonical text links.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ManufacturerLocalization<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: SourcedValue<GameTextRef>,
}

/// Prepared definition and the exact keys needed for sorting and target-row accounting.
#[derive(Debug)]
pub(crate) struct Manufacturer<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_path: &'source str,
    pub(crate) definition: ManufacturerDefinition<'source>,
}
