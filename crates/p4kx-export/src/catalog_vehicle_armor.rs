// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use super::vehicle_armor_damage::{self, VehicleArmorDamage};
use super::vehicle_armor_resistance::{self, ArmorDeflection, ArmorPenetrationResistance};
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const COMPONENT: &str = "SCItemVehicleArmorParams";

/// The independently owned tuning of a physical vehicle armour component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct VehicleArmor<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    signal_infrared: SourcedValue<ResourceFloat32>,
    signal_electromagnetic: SourcedValue<ResourceFloat32>,
    signal_cross_section: SourcedValue<ResourceFloat32>,
    damage_multiplier: SourcedValue<VehicleArmorDamage<'source>>,
    armor_penetration_resistance: SourcedValue<ArmorPenetrationResistance<'source>>,
    armor_deflection: SourcedValue<ArmorDeflection<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> VehicleArmor<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let signal_infrared = source.single("signalInfrared", notes)?;
        let signal_electromagnetic = source.single("signalElectromagnetic", notes)?;
        let signal_cross_section = source.single("signalCrossSection", notes)?;
        let damage_multiplier = source.strong_pointer_family_to(
            "damageMultiplier",
            vehicle_armor_damage::BASE,
            notes,
            VehicleArmorDamage::read,
        )?;
        let armor_penetration_resistance = source.declared_class_to(
            "armorPenetrationResistance",
            vehicle_armor_resistance::PENETRATION,
            notes,
            ArmorPenetrationResistance::read,
        )?;
        let armor_deflection = source.declared_class_to(
            "armorDeflection",
            vehicle_armor_resistance::DEFLECTION,
            notes,
            ArmorDeflection::read,
        )?;
        let additional_fields = source.authored_members_except([
            ("signalInfrared", signal_infrared.state()),
            ("signalElectromagnetic", signal_electromagnetic.state()),
            ("signalCrossSection", signal_cross_section.state()),
            ("damageMultiplier", damage_multiplier.state()),
            (
                "armorPenetrationResistance",
                armor_penetration_resistance.state(),
            ),
            ("armorDeflection", armor_deflection.state()),
        ])?;
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            signal_infrared,
            signal_electromagnetic,
            signal_cross_section,
            damage_multiplier,
            armor_penetration_resistance,
            armor_deflection,
            additional_fields,
        })
    }

    pub(super) const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }
}
