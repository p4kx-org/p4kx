// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract;
use crate::contract_quantization::QuantizationBand;
use crate::contract_rewards::QualityCurve;
use crate::definition_reference::DefinitionReference;

/// An authored quality source with its concrete struct and inline or Record-linked payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceQualitySource<'source, T> {
    /// The concrete source struct name.
    pub(crate) source_type: &'source str,
    /// How this source supplies its quality data.
    pub(crate) payload: ResourceQualityPayload<T>,
}

/// The stored form of an authored quality source.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ResourceQualityPayload<T> {
    /// The owning Record contains the quality payload.
    Inline(InlineQuality<T>),
    /// The owning Record names an existing quality-definition Record.
    RecordRef(QualityRecordReference),
    /// The source is a known fieldless base struct.
    EmptyBase,
    /// The concrete subtype is retained with a blocking Export health diagnostic.
    Unsupported,
}

/// A quality payload stored in its original owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct InlineQuality<T> {
    /// The shared quality payload or its source-preserving wrapper.
    pub(crate) value: T,
}

/// A link to a quality-distribution, override or quantization definition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct QualityRecordReference {
    /// The configured Record Reference, including an unresolved GUID.
    pub(crate) reference: Option<DefinitionReference>,
}

/// The authored ordered list of location-specific quality overrides.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceQualityOverrides<'source> {
    /// The source list, retaining null positions and repeated entries.
    pub(crate) location_override_list: Option<Vec<Option<ResourceQualityOverride<'source>>>>,
}

/// One concrete source entry of a location-override list.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceQualityOverride<'source> {
    /// The concrete source struct name, independent of the pointer's declared base.
    pub(crate) source_type: &'source str,
    /// The entry's stored fields or explicit base/unsupported state.
    pub(crate) payload: ResourceQualityOverridePayload<'source>,
}

/// The payload of a concrete or fieldless location-override entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ResourceQualityOverridePayload<'source> {
    /// The entry holds a location and an independently authored distribution source.
    Entry(ResourceQualityOverrideFields<'source>),
    /// The source is a known fieldless entry base.
    EmptyBase,
    /// The concrete entry subtype has a blocking Export health diagnostic.
    Unsupported,
}

/// The declared fields of a quality location-override entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceQualityOverrideFields<'source> {
    /// The source `StarMapObject` Reference.
    pub(crate) location: Option<DefinitionReference>,
    /// The distribution source, retaining inline and Record-linked forms.
    pub(crate) quality_distribution: Option<ResourceQualitySource<'source, QualityCurve<'source>>>,
}

/// Ordered quantization bands stored inline by the resource's quality source.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = contract::every_field_required)]
pub(crate) struct ResourceQualityQuantization {
    /// The shared exact-integer payload, retaining source positions.
    pub(crate) bands: Option<Vec<Option<QuantizationBand>>>,
}
