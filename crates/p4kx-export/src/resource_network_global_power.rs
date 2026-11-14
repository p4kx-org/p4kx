// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

const FILL_MIN: &str = "powerDistPoolsFillMin";
const COOLER_MIN: &str = "powerDistCoolerMin";
const POOL_PROPORTION: &str = "powerDistPoolsProportion";
const WEAPONS: &str = "poolDefaultWeapons";
const ENGINES: &str = "poolDefaultEngines";
const SHIELDS: &str = "poolDefaultShields";
const MODIFIERS: &str = "modifiers";
const ADDITIVE_MODIFIER: &str = "additiveModifier";

/// Stored distribution pool defaults and their control settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct DefaultPowerDistribution<'source> {
    power_dist_pools_fill_min: SourcedValue<bool>,
    power_dist_cooler_min: SourcedValue<ResourceFloat32>,
    power_dist_pools_proportion: SourcedValue<ResourceFloat32>,
    pool_default_weapons: SourcedValue<ResourceFloat32>,
    pool_default_engines: SourcedValue<ResourceFloat32>,
    pool_default_shields: SourcedValue<ResourceFloat32>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> DefaultPowerDistribution<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let power_dist_pools_fill_min = fields.boolean(FILL_MIN, notes)?;
        let power_dist_cooler_min = fields.single(COOLER_MIN, notes)?;
        let power_dist_pools_proportion = fields.single(POOL_PROPORTION, notes)?;
        let pool_default_weapons = fields.single(WEAPONS, notes)?;
        let pool_default_engines = fields.single(ENGINES, notes)?;
        let pool_default_shields = fields.single(SHIELDS, notes)?;
        let additional_fields = fields.authored_members_except([
            (FILL_MIN, power_dist_pools_fill_min.state()),
            (COOLER_MIN, power_dist_cooler_min.state()),
            (POOL_PROPORTION, power_dist_pools_proportion.state()),
            (WEAPONS, pool_default_weapons.state()),
            (ENGINES, pool_default_engines.state()),
            (SHIELDS, pool_default_shields.state()),
        ])?;
        Ok(Self {
            power_dist_pools_fill_min,
            power_dist_cooler_min,
            power_dist_pools_proportion,
            pool_default_weapons,
            pool_default_engines,
            pool_default_shields,
            additional_fields,
        })
    }
}

/// Ordered modifiers of the original global power declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct PowerParameters<'source> {
    modifiers: SourcedValue<Vec<SourcedValue<PowerModifier<'source>>>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> PowerParameters<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let modifiers = fields.class_array_to(
            MODIFIERS,
            "ItemResourceNetworkPowerModifier",
            notes,
            PowerModifier::read,
        )?;
        let additional_fields = fields.authored_members_except([(MODIFIERS, modifiers.state())])?;
        Ok(Self {
            modifiers,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct PowerModifier<'source> {
    source_type: SourcedValue<&'source str>,
    additive_modifier: SourcedValue<i32>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> PowerModifier<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let additive_modifier = fields.int32(ADDITIVE_MODIFIER, notes)?;
        let additional_fields =
            fields.authored_members_except([(ADDITIVE_MODIFIER, additive_modifier.state())])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            additive_modifier,
            additional_fields,
        })
    }
}
