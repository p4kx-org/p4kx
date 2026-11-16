// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::display::ManufacturerDisplayFeatures;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
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
    pub(crate) audio_manufacturer_tag: SourcedValue<CatalogRecordReference>,
    pub(crate) building_blocks_style: SourcedValue<CatalogRecordReference>,
    pub(crate) dashboard_canvas_config: SourcedValue<CatalogRecordReference>,
    pub(crate) light_amplification: SourcedValue<CatalogRecordReference>,
    pub(crate) logo: SourcedValue<&'source str>,
    pub(crate) logo_full_color: SourcedValue<&'source str>,
    pub(crate) logo_simplified_white: SourcedValue<&'source str>,
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
    pub(crate) short_name: SourcedValue<PublicNameRef>,
    pub(crate) display_features: SourcedValue<ManufacturerDisplayFeatures<'source>>,
}

/// Prepared definition and the exact keys needed for sorting and target-row accounting.
#[derive(Debug)]
pub(crate) struct Manufacturer<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_path: &'source str,
    pub(crate) definition: ManufacturerDefinition<'source>,
}
