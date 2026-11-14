// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const SOURCE_TYPE: &str = "SCItemShieldGeneratorParams";

/// The original configuration of one physical shield generator component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShieldGenerator<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    max_health: SourcedValue<ResourceNumber>,
    max_regen: SourcedValue<ResourceNumber>,
    decay_ratio: SourcedValue<ResourceNumber>,
    downed_regen_delay: SourcedValue<ResourceNumber>,
    damaged_regen_delay: SourcedValue<ResourceNumber>,
    #[serde(flatten)]
    reserve: ShieldReserve,
    electrical_charge_damage_resistance: SourcedValue<ResourceNumber>,
    stun_params: SourcedValue<ShieldStun<'source>>,
    shield_resistance: SourcedValue<Vec<SourcedValue<ShieldRange<'source>>>>,
    shield_absorption: SourcedValue<Vec<SourcedValue<ShieldRange<'source>>>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ShieldGenerator<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let max_health = source.authored_number("MaxShieldHealth", notes)?;
        let max_regen = source.authored_number("MaxShieldRegen", notes)?;
        let decay_ratio = source.authored_number("DecayRatio", notes)?;
        let downed_regen_delay = source.authored_number("DownedRegenDelay", notes)?;
        let damaged_regen_delay = source.authored_number("DamagedRegenDelay", notes)?;
        let reserve = ShieldReserve::read(source, notes)?;
        let electrical_charge_damage_resistance =
            source.authored_number("ElectricalChargeDamageResistance", notes)?;
        let stun_params = source.declared_class("stunParams", notes, ShieldStun::read)?;
        let shield_resistance =
            source.declared_class_array("ShieldResistance", notes, ShieldRange::read)?;
        let shield_absorption =
            source.declared_class_array("ShieldAbsorption", notes, ShieldRange::read)?;
        let projected = [
            ("MaxShieldHealth", max_health.state()),
            ("MaxShieldRegen", max_regen.state()),
            ("DecayRatio", decay_ratio.state()),
            ("DownedRegenDelay", downed_regen_delay.state()),
            ("DamagedRegenDelay", damaged_regen_delay.state()),
            (
                "ElectricalChargeDamageResistance",
                electrical_charge_damage_resistance.state(),
            ),
            ("stunParams", stun_params.state()),
            ("ShieldResistance", shield_resistance.state()),
            ("ShieldAbsorption", shield_absorption.state()),
        ];
        let additional_fields =
            source.authored_members_except(projected.into_iter().chain(reserve.projected()))?;
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            max_health,
            max_regen,
            decay_ratio,
            downed_regen_delay,
            damaged_regen_delay,
            reserve,
            electrical_charge_damage_resistance,
            stun_params,
            shield_resistance,
            shield_absorption,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ShieldReserve {
    #[serde(rename = "reservePoolInitialHealthRatio")]
    initial_health: SourcedValue<ResourceNumber>,
    #[serde(rename = "reservePoolMaxHealthRatio")]
    max_health: SourcedValue<ResourceNumber>,
    #[serde(rename = "reservePoolRegenRateRatio")]
    regen_rate: SourcedValue<ResourceNumber>,
    #[serde(rename = "reservePoolDrainRateRatio")]
    drain_rate: SourcedValue<ResourceNumber>,
}

impl ShieldReserve {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let initial_health = source.authored_number("ReservePoolInitialHealthRatio", notes)?;
        let max_health = source.authored_number("ReservePoolMaxHealthRatio", notes)?;
        let regen_rate = source.authored_number("ReservePoolRegenRateRatio", notes)?;
        let drain_rate = source.authored_number("ReservePoolDrainRateRatio", notes)?;
        Ok(Self {
            initial_health,
            max_health,
            regen_rate,
            drain_rate,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 4] {
        [
            ("ReservePoolInitialHealthRatio", self.initial_health.state()),
            ("ReservePoolMaxHealthRatio", self.max_health.state()),
            ("ReservePoolRegenRateRatio", self.regen_rate.state()),
            ("ReservePoolDrainRateRatio", self.drain_rate.state()),
        ]
    }
}

/// The authored shield stun parameters and their original source type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ShieldStun<'source> {
    source_type: SourcedValue<&'source str>,
    min_alpha_damage_ratio: SourcedValue<ResourceNumber>,
    max_alpha_damage_ratio: SourcedValue<ResourceNumber>,
    min_stun_time: SourcedValue<ResourceNumber>,
    max_stun_time: SourcedValue<ResourceNumber>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ShieldStun<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let min_alpha_damage_ratio = source.authored_number("minAlphaDamageRatio", notes)?;
        let max_alpha_damage_ratio = source.authored_number("maxAlphaDamageRatio", notes)?;
        let min_stun_time = source.authored_number("minStunTime", notes)?;
        let max_stun_time = source.authored_number("maxStunTime", notes)?;
        let additional_fields = source.authored_members_except([
            ("minAlphaDamageRatio", min_alpha_damage_ratio.state()),
            ("maxAlphaDamageRatio", max_alpha_damage_ratio.state()),
            ("minStunTime", min_stun_time.state()),
            ("maxStunTime", max_stun_time.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            min_alpha_damage_ratio,
            max_alpha_damage_ratio,
            min_stun_time,
            max_stun_time,
            additional_fields,
        })
    }
}

/// One original resistance or absorption slot in authored array order.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ShieldRange<'source> {
    source_type: SourcedValue<&'source str>,
    min: SourcedValue<ResourceNumber>,
    max: SourcedValue<ResourceNumber>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ShieldRange<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let min = source.authored_number("Min", notes)?;
        let max = source.authored_number("Max", notes)?;
        let additional_fields =
            source.authored_members_except([("Min", min.state()), ("Max", max.state())])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            min,
            max,
            additional_fields,
        })
    }
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<ShieldGenerator<'source>>,
) -> CatalogTable<ShieldGenerator<'source>> {
    rows.sort_by_key(|row| row.owner.key());
    CatalogTable::new(forge, &[SOURCE_TYPE], rows)
}
