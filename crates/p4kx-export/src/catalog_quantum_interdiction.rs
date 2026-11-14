// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::item_component_owner::ComponentOwner;
use super::quantum_fields::QuantumFields;
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceWeakReference};

#[path = "quantum_interdiction_visual_graph.rs"]
mod visual_graph;

use visual_graph::QuantumInterdictionVisualGraph;

pub(super) const SOURCE_TYPE: &str = "SCItemQuantumInterdictionGeneratorParams";
const JAMMER: &str = "SCItemQuantumJammerParams";
const PULSE: &str = "SCItemQuantumInterdictionPulseParams";
const INTERACTION: &str = "SSharedInteractionParams";

type InteractionLink<'source> = SourcedValue<Box<SourceWeakReference<'source>>>;

/// Authored settings of one item-side quantum interdiction generator.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct QuantumInterdictionGenerator<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    base_power_draw_fraction: SourcedValue<ResourceNumber>,
    pulse_power_fraction: SourcedValue<ResourceNumber>,
    jammer_power_fraction: SourcedValue<ResourceNumber>,
    jammer_settings: SourcedValue<QuantumJammerSettings<'source>>,
    pulse_settings: SourcedValue<QuantumInterdictionPulseSettings<'source>>,
    visual_graph: SourcedValue<QuantumInterdictionVisualGraph<'source>>,
    main_device_switch_on: InteractionLink<'source>,
    main_device_switch_off: InteractionLink<'source>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumInterdictionGenerator<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            owner,
            source_type: fields.source_type()?,
            base_power_draw_fraction: fields.number("basePowerDrawFraction")?,
            pulse_power_fraction: fields.number("pulsePowerFraction")?,
            jammer_power_fraction: fields.number("jammerPowerFraction")?,
            jammer_settings: fields.project("jammerSettings", |source, name, notes| {
                source.strong_pointer_to(
                    name,
                    JAMMER,
                    &[JAMMER],
                    notes,
                    QuantumJammerSettings::read,
                )
            })?,
            pulse_settings: fields.project(
                "quantumInterdictionPulseSettings",
                |source, name, notes| {
                    source.strong_pointer_to(
                        name,
                        PULSE,
                        &[PULSE],
                        notes,
                        QuantumInterdictionPulseSettings::read,
                    )
                },
            )?,
            visual_graph: fields.project("visualGraphParams", |source, name, notes| {
                source.declared_class_to(
                    name,
                    visual_graph::SOURCE_TYPE,
                    notes,
                    QuantumInterdictionVisualGraph::read,
                )
            })?,
            main_device_switch_on: fields.weak_link_to("mainDeviceSwitchOn", INTERACTION)?,
            main_device_switch_off: fields.weak_link_to("mainDeviceSwitchOff", INTERACTION)?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

/// Returns the physical device occurrences in stable item-component order.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<QuantumInterdictionGenerator<'source>>,
) -> CatalogTable<QuantumInterdictionGenerator<'source>> {
    rows.sort_by_key(|row| row.owner.key());
    CatalogTable::new(forge, &[SOURCE_TYPE], rows)
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct QuantumJammerSettings<'source> {
    source_type: SourcedValue<&'source str>,
    jammer_range: SourcedValue<ResourceNumber>,
    max_power_draw: SourcedValue<ResourceNumber>,
    green_zone_check_range: SourcedValue<ResourceNumber>,
    set_jammer_switch_on: InteractionLink<'source>,
    set_jammer_switch_off: InteractionLink<'source>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumJammerSettings<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            source_type: fields.source_type()?,
            jammer_range: fields.number("jammerRange")?,
            max_power_draw: fields.number("maxPowerDraw")?,
            green_zone_check_range: fields.number("greenZoneCheckRange")?,
            set_jammer_switch_on: fields.weak_link_to("setJammerSwitchOn", INTERACTION)?,
            set_jammer_switch_off: fields.weak_link_to("setJammerSwitchOff", INTERACTION)?,
            additional_fields: fields.additional_fields()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct QuantumInterdictionPulseSettings<'source> {
    source_type: SourcedValue<&'source str>,
    charge_time_seconds: SourcedValue<ResourceNumber>,
    discharge_time_seconds: SourcedValue<ResourceNumber>,
    cooldown_time_seconds: SourcedValue<ResourceNumber>,
    radius_meters: SourcedValue<ResourceNumber>,
    decrease_charge_rate_time_seconds: SourcedValue<ResourceNumber>,
    increase_charge_rate_time_seconds: SourcedValue<ResourceNumber>,
    activation_phase_duration_seconds: SourcedValue<ResourceNumber>,
    disperse_charge_time_seconds: SourcedValue<ResourceNumber>,
    max_power_draw: SourcedValue<ResourceNumber>,
    stop_charging_power_draw_fraction: SourcedValue<ResourceNumber>,
    max_charge_rate_power_draw_fraction: SourcedValue<ResourceNumber>,
    active_power_draw_fraction: SourcedValue<ResourceNumber>,
    tethering_power_draw_fraction: SourcedValue<ResourceNumber>,
    green_zone_check_range: SourcedValue<ResourceNumber>,
    start_charging_interaction: InteractionLink<'source>,
    cancel_charging_interaction: InteractionLink<'source>,
    disperse_charge_interaction: InteractionLink<'source>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumInterdictionPulseSettings<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            source_type: fields.source_type()?,
            charge_time_seconds: fields.number("chargeTimeSecs")?,
            discharge_time_seconds: fields.number("dischargeTimeSecs")?,
            cooldown_time_seconds: fields.number("cooldownTimeSecs")?,
            radius_meters: fields.number("radiusMeters")?,
            decrease_charge_rate_time_seconds: fields.number("decreaseChargeRateTimeSeconds")?,
            increase_charge_rate_time_seconds: fields.number("increaseChargeRateTimeSeconds")?,
            activation_phase_duration_seconds: fields.number("activationPhaseDuration_seconds")?,
            disperse_charge_time_seconds: fields.number("disperseChargeTimeSeconds")?,
            max_power_draw: fields.number("maxPowerDraw")?,
            stop_charging_power_draw_fraction: fields.number("stopChargingPowerDrawFraction")?,
            max_charge_rate_power_draw_fraction: fields.number("maxChargeRatePowerDrawFraction")?,
            active_power_draw_fraction: fields.number("activePowerDrawFraction")?,
            tethering_power_draw_fraction: fields.number("tetheringPowerDrawFraction")?,
            green_zone_check_range: fields.number("greenZoneCheckRange")?,
            start_charging_interaction: fields.weak_link_to("startChargingIP", INTERACTION)?,
            cancel_charging_interaction: fields.weak_link_to("cancelChargingIP", INTERACTION)?,
            disperse_charge_interaction: fields.weak_link_to("disperseChargeIP", INTERACTION)?,
            additional_fields: fields.additional_fields()?,
        })
    }
}
