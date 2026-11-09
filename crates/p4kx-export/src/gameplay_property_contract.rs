// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_text_contract::PublicNameRef;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

use super::transformation_contract::DisplayTransformationSource;

/// Original gameplay-property fields of one physical source Record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GameplayPropertyDefinition<'source> {
    #[serde(skip)]
    pub(crate) physical_guid: CatalogGuid,
    #[serde(skip)]
    pub(crate) physical_path: &'source str,
    #[serde(skip)]
    pub(crate) numbers: Vec<CapturedNumber>,
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) property_name: SourcedValue<PublicNameRef>,
    pub(crate) display_transformation: SourcedValue<DisplayTransformationSource<'source>>,
    pub(crate) unit_format: SourcedValue<PublicNameRef>,
    pub(crate) name_overrides: SourcedValue<Vec<SourcedValue<PropertyNameOverrideSource<'source>>>>,
}

/// A concrete authored name override independently of condition evaluation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PropertyNameOverrideSource<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<PropertyNameOverrideFields<'source>>,
}

/// The authored name and condition of one override occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PropertyNameOverrideFields<'source> {
    pub(crate) property_name: SourcedValue<PublicNameRef>,
    pub(crate) condition: SourcedValue<PropertyNameOverrideConditionSource<'source>>,
}

/// An original condition class with its separately sourced interpretation state.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PropertyNameOverrideConditionSource<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<PropertyNameOverrideConditionPayload<'source>>,
}

/// Supported condition declarations without executing a match.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) enum PropertyNameOverrideConditionPayload<'source> {
    EmptyBase,
    #[serde(rename_all = "camelCase")]
    ItemType {
        match_item_types: SourcedValue<Vec<SourcedValue<&'source str>>>,
        match_sub_types: SourcedValue<Vec<SourcedValue<&'source str>>>,
    },
}
