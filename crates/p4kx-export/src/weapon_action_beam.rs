// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;

use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[path = "weapon_beam_damage.rs"]
mod damage;

use damage::BeamDamage;

/// Original beam damage, ranges, thermal inputs and activation timing.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BeamAction<'source> {
    damage_per_second: SourcedValue<BeamDamage<'source>>,
    full_damage_range: SourcedValue<ResourceFloat32>,
    zero_damage_range: SourcedValue<ResourceFloat32>,
    hit_type: SourcedValue<&'source str>,
    heat_cost_per_second: SourcedValue<ResourceFloat32>,
    heat_per_second: SourcedValue<ResourceFloat32>,
    wear_per_second: SourcedValue<ResourceFloat32>,
    min_energy_draw: SourcedValue<ResourceFloat32>,
    max_energy_draw: SourcedValue<ResourceFloat32>,
    charge_up_time: SourcedValue<ResourceFloat32>,
    charge_down_time: SourcedValue<ResourceFloat32>,
    fire_during_charge_up: SourcedValue<bool>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> BeamAction<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
        recoil: [(&'static str, SourceValueState); 3],
    ) -> Result<Self, CatalogError> {
        let damage_per_second = source.strong_pointer_family_to(
            "damagePerSecond",
            damage::BASE,
            notes,
            BeamDamage::read,
        )?;
        let full_damage_range = source.single("fullDamageRange", notes)?;
        let zero_damage_range = source.single("zeroDamageRange", notes)?;
        let hit_type = source.borrowed_string("hitType", notes)?;
        let heat_cost_per_second = source.single("heatCostPerSecond", notes)?;
        let heat_per_second = source.single("heatPerSecond", notes)?;
        let wear_per_second = source.single("wearPerSecond", notes)?;
        let min_energy_draw = source.single("minEnergyDraw", notes)?;
        let max_energy_draw = source.single("maxEnergyDraw", notes)?;
        let charge_up_time = source.single("chargeUpTime", notes)?;
        let charge_down_time = source.single("chargeDownTime", notes)?;
        let fire_during_charge_up = source.boolean("fireDuringChargeUp", notes)?;
        let authored = source.authored_members_except(
            [
                ("damagePerSecond", damage_per_second.state()),
                ("fullDamageRange", full_damage_range.state()),
                ("zeroDamageRange", zero_damage_range.state()),
                ("hitType", hit_type.state()),
                ("heatCostPerSecond", heat_cost_per_second.state()),
                ("heatPerSecond", heat_per_second.state()),
                ("wearPerSecond", wear_per_second.state()),
                ("minEnergyDraw", min_energy_draw.state()),
                ("maxEnergyDraw", max_energy_draw.state()),
                ("chargeUpTime", charge_up_time.state()),
                ("chargeDownTime", charge_down_time.state()),
                ("fireDuringChargeUp", fire_during_charge_up.state()),
            ]
            .into_iter()
            .chain(recoil),
        )?;
        Ok(Self {
            damage_per_second,
            full_damage_range,
            zero_damage_range,
            hit_type,
            heat_cost_per_second,
            heat_per_second,
            wear_per_second,
            min_energy_draw,
            max_energy_draw,
            charge_up_time,
            charge_down_time,
            fire_during_charge_up,
            authored,
        })
    }
}
