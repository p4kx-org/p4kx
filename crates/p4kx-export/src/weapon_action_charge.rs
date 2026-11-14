// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_nodes::AuthoredNodes;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

use super::super::read_child;

/// The original charged wrapper, its modifier and its separate child action.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ChargedAction<'source> {
    charge_time: SourcedValue<ResourceFloat32>,
    overcharge_time: SourcedValue<ResourceFloat32>,
    overcharged_time: SourcedValue<ResourceFloat32>,
    cooldown_time: SourcedValue<ResourceFloat32>,
    max_glow: SourcedValue<ResourceFloat32>,
    interpolate_charge_bonus: SourcedValue<bool>,
    fire_automatically_on_full_charge: SourcedValue<bool>,
    fire_only_on_full_charge: SourcedValue<bool>,
    charge_automatically: SourcedValue<bool>,
    max_charge_modifier: SourcedValue<SourceArenaOutput<'source>>,
    weapon_action: SourcedValue<u32>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> ChargedAction<'source> {
    pub(super) fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
        recoil: [(&'static str, SourceValueState); 3],
    ) -> Result<Self, CatalogError> {
        let charge_time = source.single("chargeTime", notes)?;
        let overcharge_input = source.single("overchargeTime", notes)?;
        let overcharged_time = source.single("overchargedTime", notes)?;
        let cooldown_time = source.single("cooldownTime", notes)?;
        let max_glow = source.single("maxGlow", notes)?;
        let interpolate_charge_bonus = source.boolean("interpolateChargeBonus", notes)?;
        let fire_automatically_on_full_charge =
            source.boolean("fireAutomaticallyOnFullCharge", notes)?;
        let fire_only_on_full_charge = source.boolean("fireOnlyOnFullCharge", notes)?;
        let charge_automatically = source.boolean("chargeAutomatically", notes)?;
        let max_charge_modifier =
            source.inline_class("maxChargeModifier", &["SWeaponStats"], notes, |stats, _| {
                stats.authored_members()
            })?;
        let weapon_action = read_child(source, "weaponAction", pending, notes)?;
        let authored = source.authored_members_except(
            [
                ("chargeTime", charge_time.state()),
                ("overchargeTime", overcharge_input.state()),
                ("overchargedTime", overcharged_time.state()),
                ("cooldownTime", cooldown_time.state()),
                ("maxGlow", max_glow.state()),
                ("interpolateChargeBonus", interpolate_charge_bonus.state()),
                (
                    "fireAutomaticallyOnFullCharge",
                    fire_automatically_on_full_charge.state(),
                ),
                ("fireOnlyOnFullCharge", fire_only_on_full_charge.state()),
                ("chargeAutomatically", charge_automatically.state()),
                ("maxChargeModifier", max_charge_modifier.state()),
                ("weaponAction", weapon_action.state()),
            ]
            .into_iter()
            .chain(recoil),
        )?;
        Ok(Self {
            charge_time,
            overcharge_time: overcharge_input,
            overcharged_time,
            cooldown_time,
            max_glow,
            interpolate_charge_bonus,
            fire_automatically_on_full_charge,
            fire_only_on_full_charge,
            charge_automatically,
            max_charge_modifier,
            weapon_action,
            authored,
        })
    }
}
