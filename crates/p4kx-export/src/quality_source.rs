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
pub struct QualitySource<'source, T> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<QualityPayload<T>>,
}

/// Closed ownership forms of quality payload declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum QualityPayload<T> {
    /// A payload authored inline in the same original Record.
    Inline(InlineQuality<T>),
    /// An authored reference without reading its target body.
    RecordRef(QualityRecordReference),
    /// An authored base source without a quality payload.
    EmptyBase,
}

/// An inline payload from the same original Record owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct InlineQuality<T> {
    pub(crate) value: SourcedValue<T>,
}

/// An authored Reference, without reading the target payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct QualityRecordReference {
    pub(crate) reference: SourcedValue<CatalogRecordReference>,
}

/// Exact source bounds and numeric representations of one inline distribution.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct QualityCurveSource {
    pub(crate) min: SourcedValue<ResourceNumber>,
    pub(crate) max: SourcedValue<ResourceNumber>,
    pub(crate) mean: SourcedValue<ResourceNumber>,
    pub(crate) stddev: SourcedValue<ResourceNumber>,
}

/// An ordered nullable list of source location overrides.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct QualityOverridesSource<'source> {
    pub(crate) location_override_list:
        SourcedValue<Vec<SourcedValue<QualityOverrideSource<'source>>>>,
}

/// A concrete location-override entry with its actual source type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct QualityOverrideSource<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<QualityOverridePayload<'source>>,
}

/// The closed override-entry shapes accepted from the source.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum QualityOverridePayload<'source> {
    /// An original location and distribution override.
    Entry(Box<QualityOverrideFields<'source>>),
    /// An authored base override without additional fields.
    EmptyBase,
}

/// Authored location and distribution fields of an override entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct QualityOverrideFields<'source> {
    pub(crate) location: SourcedValue<CatalogRecordReference>,
    pub(crate) quality_distribution: SourcedValue<QualitySource<'source, QualityCurveSource>>,
}

/// Ordered source quantization bands, retaining every nullable position.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct QualityQuantizationSource<'source> {
    pub(crate) bands: SourcedValue<Vec<SourcedValue<QualityBandSource<'source>>>>,
}

/// Exact numeric storage of one source quantization band.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct QualityBandSource<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) start: SourcedValue<ResourceNumber>,
    pub(crate) end: SourcedValue<ResourceNumber>,
    pub(crate) mapped_value: SourcedValue<ResourceNumber>,
}

impl<'source, T> QualitySource<'source, T> {
    /// Borrows the original concrete quality-source type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the inline, referenced or empty payload without flattening its state.
    #[must_use]
    pub const fn payload(&self) -> &SourcedValue<QualityPayload<T>> {
        &self.payload
    }
}

impl<T> InlineQuality<T> {
    /// Borrows the original inline payload and its immediate evidence.
    #[must_use]
    pub const fn value(&self) -> &SourcedValue<T> {
        &self.value
    }
}

impl QualityRecordReference {
    /// Borrows the authored quality Record reference without loading its body.
    #[must_use]
    pub const fn reference(&self) -> &SourcedValue<CatalogRecordReference> {
        &self.reference
    }
}

impl QualityCurveSource {
    /// Borrows the source lower bound with its numeric width and bits.
    #[must_use]
    pub const fn min(&self) -> &SourcedValue<ResourceNumber> {
        &self.min
    }

    /// Borrows the source upper bound with its numeric width and bits.
    #[must_use]
    pub const fn max(&self) -> &SourcedValue<ResourceNumber> {
        &self.max
    }

    /// Borrows the authored mean with its exact numeric representation.
    #[must_use]
    pub const fn mean(&self) -> &SourcedValue<ResourceNumber> {
        &self.mean
    }

    /// Borrows the authored standard deviation without normalization.
    #[must_use]
    pub const fn stddev(&self) -> &SourcedValue<ResourceNumber> {
        &self.stddev
    }
}

impl<'source> QualityOverridesSource<'source> {
    /// Borrows original location-override positions, including unavailable entries.
    #[must_use]
    pub const fn location_override_list(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<QualityOverrideSource<'source>>>> {
        &self.location_override_list
    }
}

impl<'source> QualityOverrideSource<'source> {
    /// Borrows the original concrete override type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the original override variant and its source state.
    #[must_use]
    pub const fn payload(&self) -> &SourcedValue<QualityOverridePayload<'source>> {
        &self.payload
    }
}

impl<'source> QualityOverrideFields<'source> {
    /// Borrows the authored override location reference.
    #[must_use]
    pub const fn location(&self) -> &SourcedValue<CatalogRecordReference> {
        &self.location
    }

    /// Borrows the original distribution source for this location.
    #[must_use]
    pub const fn quality_distribution(
        &self,
    ) -> &SourcedValue<QualitySource<'source, QualityCurveSource>> {
        &self.quality_distribution
    }
}

impl<'source> QualityQuantizationSource<'source> {
    /// Borrows ordered quantization bands without removing unavailable positions.
    #[must_use]
    pub const fn bands(&self) -> &SourcedValue<Vec<SourcedValue<QualityBandSource<'source>>>> {
        &self.bands
    }
}

impl<'source> QualityBandSource<'source> {
    /// Borrows the original concrete quantization-band type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the authored start with its exact numeric representation.
    #[must_use]
    pub const fn start(&self) -> &SourcedValue<ResourceNumber> {
        &self.start
    }

    /// Borrows the authored end with its exact numeric representation.
    #[must_use]
    pub const fn end(&self) -> &SourcedValue<ResourceNumber> {
        &self.end
    }

    /// Borrows the authored mapped value without rounding or conversion.
    #[must_use]
    pub const fn mapped_value(&self) -> &SourcedValue<ResourceNumber> {
        &self.mapped_value
    }
}
