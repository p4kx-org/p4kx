// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::{CanonicalRecordHeader, CatalogRecordSource};
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
    #[serde(flatten)]
    pub(crate) fields: GameplayPropertyFields<'source>,
}

/// A gameplay-property declaration and its original physical Record identity.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct GameplayProperty {
    /// The original physical Record identity and header fields.
    #[serde(flatten)]
    pub source: CatalogRecordSource,
    /// The original gameplay-property fields.
    #[serde(flatten)]
    pub fields: GameplayPropertyFields<'static>,
}

/// Source-backed gameplay-property fields without condition or transformation evaluation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct GameplayPropertyFields<'source> {
    /// The default public name reference.
    pub property_name: SourcedValue<PublicNameRef>,
    /// The ordered declarations for displaying a value.
    pub display_transformation: SourcedValue<DisplayTransformationSource<'source>>,
    /// The public unit format reference.
    pub unit_format: SourcedValue<PublicNameRef>,
    /// Original override slots in authored order.
    pub name_overrides: SourcedValue<Vec<SourcedValue<PropertyNameOverrideSource<'source>>>>,
}

/// A concrete authored name override independently of condition evaluation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct PropertyNameOverrideSource<'source> {
    /// The concrete source class.
    pub source_type: SourcedValue<Cow<'source, str>>,
    /// The authored override fields.
    pub payload: SourcedValue<PropertyNameOverrideFields<'source>>,
}

/// The authored name and condition of one override occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct PropertyNameOverrideFields<'source> {
    /// The public name supplied by this override.
    pub property_name: SourcedValue<PublicNameRef>,
    /// The authored condition attached to this override.
    pub condition: SourcedValue<PropertyNameOverrideConditionSource<'source>>,
}

/// An original condition class with its separately sourced interpretation state.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct PropertyNameOverrideConditionSource<'source> {
    /// The concrete source condition class.
    pub source_type: SourcedValue<Cow<'source, str>>,
    /// The condition fields of the concrete source class.
    pub payload: SourcedValue<PropertyNameOverrideConditionPayload<'source>>,
}

/// Supported condition declarations without executing a match.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum PropertyNameOverrideConditionPayload<'source> {
    /// The base condition has no fields.
    EmptyBase,
    /// An authored item-type and subtype condition.
    #[serde(rename_all = "camelCase")]
    ItemType {
        /// Original accepted type slots.
        match_item_types: SourcedValue<Vec<SourcedValue<Cow<'source, str>>>>,
        /// Original accepted subtype slots.
        match_sub_types: SourcedValue<Vec<SourcedValue<Cow<'source, str>>>>,
    },
}
