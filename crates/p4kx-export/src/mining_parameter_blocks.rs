// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Independently sourced numeric fields of `hitConsistencyParams`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MiningHitConsistency {
    extraction_magnitude: SourcedValue<ResourceNumber>,
    hit_history_window: SourcedValue<ResourceNumber>,
    max_effect_on_instability: SourcedValue<ResourceNumber>,
    min_deviation: SourcedValue<ResourceNumber>,
    standard_deviation_multiplier: SourcedValue<ResourceNumber>,
    time_exponent: SourcedValue<ResourceNumber>,
}
impl MiningHitConsistency {
    pub(super) fn read_source(
        fields: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        owned(fields, "hitConsistencyParams", notes, Self::read)
    }

    /// Borrows the exact original `extractionMagnitude` value.
    #[must_use]
    pub const fn extraction_magnitude(&self) -> &SourcedValue<ResourceNumber> {
        &self.extraction_magnitude
    }
    /// Borrows the exact original `hitHistoryWindow` value.
    #[must_use]
    pub const fn hit_history_window(&self) -> &SourcedValue<ResourceNumber> {
        &self.hit_history_window
    }
    /// Borrows the exact original `maxEffectOnInstability` value.
    #[must_use]
    pub const fn max_effect_on_instability(&self) -> &SourcedValue<ResourceNumber> {
        &self.max_effect_on_instability
    }
    /// Borrows the exact original `minDeviation` value.
    #[must_use]
    pub const fn min_deviation(&self) -> &SourcedValue<ResourceNumber> {
        &self.min_deviation
    }
    /// Borrows the exact original `standardDeviationMultiplier` value.
    #[must_use]
    pub const fn standard_deviation_multiplier(&self) -> &SourcedValue<ResourceNumber> {
        &self.standard_deviation_multiplier
    }
    /// Borrows the exact original `timeExponent` value.
    #[must_use]
    pub const fn time_exponent(&self) -> &SourcedValue<ResourceNumber> {
        &self.time_exponent
    }
    pub(super) fn read(
        fields: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            extraction_magnitude: fields.authored_number("extractionMagnitude", notes)?,
            hit_history_window: fields.authored_number("hitHistoryWindow", notes)?,
            max_effect_on_instability: fields.authored_number("maxEffectOnInstability", notes)?,
            min_deviation: fields.authored_number("minDeviation", notes)?,
            standard_deviation_multiplier: fields
                .authored_number("standardDeviationMultiplier", notes)?,
            time_exponent: fields.authored_number("timeExponent", notes)?,
        })
    }
}

/// Independently sourced numeric fields of `mineableInstabilityParams`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MiningInstability {
    #[serde(rename = "instabilityCurveFactor")]
    curve_factor: SourcedValue<ResourceNumber>,
    #[serde(rename = "instabilityWavePeriod")]
    wave_period: SourcedValue<ResourceNumber>,
    #[serde(rename = "instabilityWaveVariance")]
    wave_variance: SourcedValue<ResourceNumber>,
}
impl MiningInstability {
    pub(super) fn read_source(
        fields: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        owned(fields, "mineableInstabilityParams", notes, Self::read)
    }

    /// Borrows the exact original `instabilityCurveFactor` value.
    #[must_use]
    pub const fn instability_curve_factor(&self) -> &SourcedValue<ResourceNumber> {
        &self.curve_factor
    }
    /// Borrows the exact original `instabilityWavePeriod` value.
    #[must_use]
    pub const fn instability_wave_period(&self) -> &SourcedValue<ResourceNumber> {
        &self.wave_period
    }
    /// Borrows the exact original `instabilityWaveVariance` value.
    #[must_use]
    pub const fn instability_wave_variance(&self) -> &SourcedValue<ResourceNumber> {
        &self.wave_variance
    }
    pub(super) fn read(
        fields: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            curve_factor: fields.authored_number("instabilityCurveFactor", notes)?,
            wave_period: fields.authored_number("instabilityWavePeriod", notes)?,
            wave_variance: fields.authored_number("instabilityWaveVariance", notes)?,
        })
    }
}

/// Independently sourced numeric fields of `mineableExplosionParams`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MiningExplosion {
    danger_pool_factor: SourcedValue<ResourceNumber>,
    default_volume: SourcedValue<ResourceNumber>,
}
impl MiningExplosion {
    pub(super) fn read_source(
        fields: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        owned(fields, "mineableExplosionParams", notes, Self::read)
    }

    /// Borrows the exact original `dangerPoolFactor` value.
    #[must_use]
    pub const fn danger_pool_factor(&self) -> &SourcedValue<ResourceNumber> {
        &self.danger_pool_factor
    }
    /// Borrows the exact original `defaultVolume` value.
    #[must_use]
    pub const fn default_volume(&self) -> &SourcedValue<ResourceNumber> {
        &self.default_volume
    }
    pub(super) fn read(
        fields: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            danger_pool_factor: fields.authored_number("dangerPoolFactor", notes)?,
            default_volume: fields.authored_number("defaultVolume", notes)?,
        })
    }
}

fn owned<'reader, 'scope, 'read, 'source, T>(
    fields: Observed<'reader, 'scope, 'read, 'source>,
    name: &str,
    notes: &mut Vec<FieldError>,
    decode: impl FnOnce(
        Observed<'reader, 'scope, 'read, 'source>,
        &mut Vec<FieldError>,
    ) -> Result<T, CatalogError>,
) -> Result<SourcedValue<T>, CatalogError> {
    match fields.storage_type(name)? {
        Some(DataType::StrongPointer) => fields.declared_pointer(name, notes, decode),
        _ => fields.declared_class(name, notes, decode),
    }
}
