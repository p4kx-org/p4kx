// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use crate::CatalogError;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// The exact nominal `FloatModifier` variant and its authored fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct FloatModifier<'source> {
    value: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> FloatModifier<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let value = source.single("value", notes)?;
        let original = UnitSource::read(source, [("value", value.state())])?;
        Ok(Self { value, original })
    }
}

fn modifier<'source>(
    source: Observed<'_, '_, '_, 'source>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<FloatModifier<'source>>, CatalogError> {
    source.strong_pointer_family_to(name, "FloatModifier", notes, FloatModifier::read)
}

/// The authored float modifiers used by a mining laser or attachable modifier.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MiningModifiers<'source> {
    is_optimal_rate_good: SourcedValue<bool>,
    laser_instability: SourcedValue<FloatModifier<'source>>,
    optimal_charge_window_size_modifier: SourcedValue<FloatModifier<'source>>,
    resistance_modifier: SourcedValue<FloatModifier<'source>>,
    shatterdamage_modifier: SourcedValue<FloatModifier<'source>>,
    cluster_factor_modifier: SourcedValue<FloatModifier<'source>>,
    optimal_charge_window_rate_modifier: SourcedValue<FloatModifier<'source>>,
    catastrophic_charge_window_rate_modifier: SourcedValue<FloatModifier<'source>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> MiningModifiers<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let is_optimal_rate_good = source.boolean("isOptimalRateGood", notes)?;
        let laser_instability = modifier(source, "laserInstability", notes)?;
        let optimal_charge_window_size_modifier =
            modifier(source, "optimalChargeWindowSizeModifier", notes)?;
        let resistance_modifier = modifier(source, "resistanceModifier", notes)?;
        let shatterdamage_modifier = modifier(source, "shatterdamageModifier", notes)?;
        let cluster_factor_modifier = modifier(source, "clusterFactorModifier", notes)?;
        let optimal_charge_window_rate_modifier =
            modifier(source, "optimalChargeWindowRateModifier", notes)?;
        let catastrophic_charge_window_rate_modifier =
            modifier(source, "catastrophicChargeWindowRateModifier", notes)?;
        let original = UnitSource::read(
            source,
            [
                ("isOptimalRateGood", is_optimal_rate_good.state()),
                ("laserInstability", laser_instability.state()),
                (
                    "optimalChargeWindowSizeModifier",
                    optimal_charge_window_size_modifier.state(),
                ),
                ("resistanceModifier", resistance_modifier.state()),
                ("shatterdamageModifier", shatterdamage_modifier.state()),
                ("clusterFactorModifier", cluster_factor_modifier.state()),
                (
                    "optimalChargeWindowRateModifier",
                    optimal_charge_window_rate_modifier.state(),
                ),
                (
                    "catastrophicChargeWindowRateModifier",
                    catastrophic_charge_window_rate_modifier.state(),
                ),
            ],
        )?;
        Ok(Self {
            is_optimal_rate_good,
            laser_instability,
            optimal_charge_window_size_modifier,
            resistance_modifier,
            shatterdamage_modifier,
            cluster_factor_modifier,
            optimal_charge_window_rate_modifier,
            catastrophic_charge_window_rate_modifier,
            original,
        })
    }
}

/// The original mining filter and its authored float modifier.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MiningFilter<'source> {
    filter_modifier: SourcedValue<FloatModifier<'source>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> MiningFilter<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let filter_modifier = modifier(source, "filterModifier", notes)?;
        let original = UnitSource::read(source, [("filterModifier", filter_modifier.state())])?;
        Ok(Self {
            filter_modifier,
            original,
        })
    }
}
