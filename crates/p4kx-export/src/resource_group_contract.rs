// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// A selected physical group and its separately owned parent projection.
#[derive(Debug)]
pub(crate) struct ResourceGroup<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_path: &'source str,
    pub(crate) record_index: u32,
    pub(crate) definition: ResourceGroupDefinition<'source>,
    pub(crate) parent: ResourceGroupParent,
}

/// Direct scalar facts of one physical `ResourceTypeGroup` Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ResourceGroupDefinition<'source> {
    #[serde(flatten)]
    pub(crate) source: ResourceGroupHeader<'source>,
    pub(crate) name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: SourcedValue<GameTextRef>,
    pub(crate) default_thumbnail_path: SourcedValue<&'source str>,
    #[serde(rename = "defaultThumbnailPathSVG")]
    pub(crate) default_thumbnail_path_svg: SourcedValue<&'source str>,
}

/// The original ordered membership arrays of one selected physical group.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ResourceGroupMembers {
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) physical: ResourceGroupKey,
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_index: SourcedValue<u32>,
    pub(crate) groups: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
    pub(crate) resources: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

/// The first selected parent with a different GUID, separate from raw memberships.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ResourceGroupParent {
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_index: SourcedValue<u32>,
    pub(crate) parent: SourcedValue<Option<CatalogRecordReference>>,
}

/// Original physical Record identity with independently sourced header facts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ResourceGroupHeader<'source> {
    pub(crate) guid: SourcedValue<CatalogGuid>,
    pub(crate) record_name: SourcedValue<&'source str>,
    pub(crate) class_name: SourcedValue<&'source str>,
    pub(crate) record_path: SourcedValue<&'source str>,
    pub(crate) record_index: SourcedValue<u32>,
    pub(crate) source_type: SourcedValue<&'source str>,
}

/// Private physical sorting identity, independent of serialized field states.
#[derive(Debug)]
pub(crate) struct ResourceGroupKey {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_index: u32,
}
