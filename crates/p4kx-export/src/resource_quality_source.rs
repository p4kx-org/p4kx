// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_number::ResourceNumber;
use crate::schema_fields;

/// An authored quality source retaining its inline or Reference form.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceQualitySource<'source, T> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<ResourceQualityPayload<T>>,
}

/// Closed ownership forms of quality payload declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ResourceQualityPayload<T> {
    Inline(InlineQuality<T>),
    RecordRef(QualityRecordReference),
    EmptyBase,
}

/// An inline payload from the same original Record owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct InlineQuality<T> {
    pub(crate) value: SourcedValue<T>,
}

/// An authored Reference, without reading the target payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct QualityRecordReference {
    pub(crate) reference: SourcedValue<CatalogRecordReference>,
}

/// Exact source bounds and numeric representations of one inline distribution.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceQualityCurve<'source> {
    pub(crate) kind: SourcedValue<&'source str>,
    pub(crate) min: SourcedValue<i32>,
    pub(crate) max: SourcedValue<i32>,
    pub(crate) mean: SourcedValue<ResourceNumber>,
    pub(crate) stddev: SourcedValue<ResourceNumber>,
}

/// An ordered nullable list of source location overrides.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceQualityOverrides<'source> {
    pub(crate) location_override_list:
        SourcedValue<Vec<SourcedValue<ResourceQualityOverride<'source>>>>,
}

/// A concrete location-override entry with its actual source type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceQualityOverride<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<ResourceQualityOverridePayload<'source>>,
}

/// The closed override-entry shapes accepted from the source.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ResourceQualityOverridePayload<'source> {
    Entry(Box<ResourceQualityOverrideFields<'source>>),
    EmptyBase,
}

/// Authored location and distribution fields of an override entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceQualityOverrideFields<'source> {
    pub(crate) location: SourcedValue<CatalogRecordReference>,
    pub(crate) quality_distribution:
        SourcedValue<ResourceQualitySource<'source, ResourceQualityCurve<'source>>>,
}

/// Ordered source quantization bands, retaining every nullable position.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceQualityQuantization<'source> {
    pub(crate) bands: SourcedValue<Vec<SourcedValue<ResourceQuantizationBand<'source>>>>,
}

/// Exact Int32 fields of one source quantization band.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub(crate) struct ResourceQuantizationBand<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) start: SourcedValue<i32>,
    pub(crate) end: SourcedValue<i32>,
    pub(crate) mapped_value: SourcedValue<i32>,
}
