// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceRecordReference;

/// The selected authored crew-manifest link, independently of resolved crew membership.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShipPlayabilityInputs<'source> {
    pub(crate) crew_manifest_component_ordinal: SourcedValue<u32>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    pub(crate) crew_manifest: SourcedValue<Box<SourceRecordReference<'source>>>,
}
