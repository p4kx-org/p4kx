// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::guid_text::CatalogGuid;
use crate::resource_density_source::ResourceDensitySource;
use crate::resource_property_contract::{ResourceCargoSlot, ResourceProperty};
use crate::schema_fields::every_field_required;

/// A selected resource definition with independent internal selection identity.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ResourceType<'source> {
    #[serde(skip)]
    pub(crate) guid: CatalogGuid,
    #[serde(skip)]
    pub(crate) record_path: &'source str,
    #[serde(flatten)]
    pub(crate) data: ResourceDefinition<'source>,
}

/// Canonical resource data values with immediate source provenance.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ResourceDefinition<'source> {
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) class_name: SourcedValue<&'source str>,
    pub(crate) record_path: SourcedValue<&'source str>,
    pub(crate) name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: SourcedValue<GameTextRef>,
    pub(crate) group_guids: SourcedValue<Vec<SourcedValue<CatalogGuid>>>,
    pub(crate) refined_version: SourcedValue<CatalogRecordReference>,
    pub(crate) density: SourcedValue<ResourceDensitySource<'source>>,
    pub(crate) default_thumbnail_path: SourcedValue<&'source str>,
    #[serde(rename = "defaultThumbnailPathSVG")]
    pub(crate) default_thumbnail_path_svg: SourcedValue<&'source str>,
    pub(crate) rtt_thumbnail_entity_class: SourcedValue<CatalogRecordReference>,
    pub(crate) validate_default_cargo_box: SourcedValue<bool>,
    pub(crate) default_cargo_containers:
        SourcedValue<Vec<SourcedValue<ResourceCargoSlot<'source>>>>,
    pub(crate) properties: SourcedValue<Vec<SourcedValue<ResourceProperty<'source>>>>,
}
