// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_projection::ProtectedKey;
use crate::catalog_text_contract::GameTextRef;
use crate::mission_property_source::SourceTagList;
use crate::owned_reference::OwnedReference;
use crate::scoped_location_link::PropertyReference;
use crate::spawn_model::TagPaths;

/// Metadata inherited from `BaseMissionPropertyValueOption`.
#[derive(Debug, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
#[non_exhaustive]
pub struct OptionMetadata<'f> {
    /// The original nullable `textId`, retaining empty and placeholder tokens.
    pub text_id: Option<&'f str>,
    /// The protected text reference derived from this option's original key.
    // The unprojected metadata compares authored payloads before choosing an exact source owner.
    /// The display name of this projection.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "SourcedValue<GameTextRef>", extend("x-gameText" = true))]
    pub display_name: Option<SourcedValue<GameTextRef>>,
    #[serde(skip)]
    pub(crate) pending_name: Option<ProtectedKey<'f>>,
    /// The source `Single` weighting, including fractional values.
    pub weighting: Option<f32>,
    /// The authored `DEBUG_forceChooseThisOption` flag.
    pub debug_force_choose_this_option: Option<bool>,
    /// Source dependencies in order, retaining null declarations.
    pub dependent_properties: Option<Vec<Option<DependentProperty>>>,
}

/// A dependent mission property and the exact option nodes it names.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
#[non_exhaustive]
pub struct DependentProperty {
    /// The original owner-local property link and resolved variable when available.
    pub property: Option<PropertyReference>,
    /// The owner-local option links in source order.
    pub values: Option<Vec<Option<OwnedReference>>>,
}

/// A string-hash option with complete authored metadata.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
#[non_exhaustive]
pub struct StringOption<'f> {
    /// The metadata of this projection.
    #[serde(flatten)]
    pub metadata: OptionMetadata<'f>,
    /// The exact source string, retaining an empty value.
    pub value: Option<String>,
}

/// An exact signed integer option.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
#[non_exhaustive]
pub struct IntegerOption<'f> {
    /// The metadata of this projection.
    #[serde(flatten)]
    pub metadata: OptionMetadata<'f>,
    /// The source `Int32`, without float conversion.
    #[schemars(range(min = i32::MIN, max = i32::MAX))]
    pub value: Option<i32>,
    /// The source `Int32` variation.
    #[schemars(range(min = i32::MIN, max = i32::MAX))]
    pub variation: Option<i32>,
}

/// A floating-point option with complete authored metadata.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
#[non_exhaustive]
pub struct FloatOption<'f> {
    /// The metadata of this projection.
    #[serde(flatten)]
    pub metadata: OptionMetadata<'f>,
    /// The finite source value.
    pub value: Option<f32>,
    /// The finite source variation.
    pub variation: Option<f32>,
}

/// A tag option with complete authored metadata and raw tag identities.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::contract::every_field_required)]
#[non_exhaustive]
pub struct TagOption<'f> {
    /// The metadata of this projection.
    #[serde(flatten)]
    pub metadata: OptionMetadata<'f>,
    /// Resolved positive tag paths.
    pub tags: TagPaths,
    /// Resolved negative tag paths.
    pub negative_tags: TagPaths,
    /// The source positive tag list with raw identities.
    pub source_tags: Option<SourceTagList>,
    /// The source negative tag list with raw identities.
    pub source_negative_tags: Option<SourceTagList>,
}
