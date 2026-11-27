// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_item_components::source_getters;
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

pub use visual_graph::{
    QuantumInterdictionTransitionParameters, QuantumInterdictionTransitionSelector,
    QuantumInterdictionVisualAnimation, QuantumInterdictionVisualEffect,
    QuantumInterdictionVisualGraph, QuantumInterdictionVisualState,
    QuantumInterdictionVisualTransition,
};

pub(super) const SOURCE_TYPE: &str = "SCItemQuantumInterdictionGeneratorParams";
const JAMMER: &str = "SCItemQuantumJammerParams";
const PULSE: &str = "SCItemQuantumInterdictionPulseParams";
const INTERACTION: &str = "SSharedInteractionParams";

type InteractionLink<'source> = SourcedValue<Box<SourceWeakReference<'source>>>;

/// Authored settings of one item-side quantum interdiction generator.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct QuantumInterdictionGenerator<'source> {
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

/// Authored quantum jammer settings and interaction links.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct QuantumJammerSettings<'source> {
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

/// Authored quantum interdiction pulse settings and interaction links.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct QuantumInterdictionPulseSettings<'source> {
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

impl<'source> QuantumInterdictionGenerator<'source> {
    source_getters! {
        /// The physical Item Record and original Components slot.
        owner: ComponentOwner,
        /// The original concrete source type.
        source_type: SourcedValue<&'source str>,
        /// The authored base power-draw fraction.
        base_power_draw_fraction: SourcedValue<ResourceNumber>,
        /// The authored pulse power fraction.
        pulse_power_fraction: SourcedValue<ResourceNumber>,
        /// The authored jammer power fraction.
        jammer_power_fraction: SourcedValue<ResourceNumber>,
        /// The authored jammer settings.
        jammer_settings: SourcedValue<QuantumJammerSettings<'source>>,
        /// The authored pulse settings.
        pulse_settings: SourcedValue<QuantumInterdictionPulseSettings<'source>>,
        /// The authored visual state graph.
        visual_graph: SourcedValue<QuantumInterdictionVisualGraph<'source>>,
        /// The weak reference for switching the device on.
        main_device_switch_on: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// The weak reference for switching the device off.
        main_device_switch_off: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}

impl<'source> QuantumJammerSettings<'source> {
    source_getters! {
        /// The original concrete source type.
        source_type: SourcedValue<&'source str>,
        /// The authored jammer range.
        jammer_range: SourcedValue<ResourceNumber>,
        /// The authored maximum power draw.
        max_power_draw: SourcedValue<ResourceNumber>,
        /// The authored green-zone check range.
        green_zone_check_range: SourcedValue<ResourceNumber>,
        /// The weak reference for switching the jammer on.
        set_jammer_switch_on: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// The weak reference for switching the jammer off.
        set_jammer_switch_off: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}

impl<'source> QuantumInterdictionPulseSettings<'source> {
    source_getters! {
        /// The original concrete source type.
        source_type: SourcedValue<&'source str>,
        /// The authored charge time in seconds.
        charge_time_seconds: SourcedValue<ResourceNumber>,
        /// The authored discharge time in seconds.
        discharge_time_seconds: SourcedValue<ResourceNumber>,
        /// The authored cooldown time in seconds.
        cooldown_time_seconds: SourcedValue<ResourceNumber>,
        /// The authored radius in metres.
        radius_meters: SourcedValue<ResourceNumber>,
        /// The authored time for decreasing the charge rate.
        decrease_charge_rate_time_seconds: SourcedValue<ResourceNumber>,
        /// The authored time for increasing the charge rate.
        increase_charge_rate_time_seconds: SourcedValue<ResourceNumber>,
        /// The authored activation phase duration in seconds.
        activation_phase_duration_seconds: SourcedValue<ResourceNumber>,
        /// The authored charge dispersal time in seconds.
        disperse_charge_time_seconds: SourcedValue<ResourceNumber>,
        /// The authored maximum power draw.
        max_power_draw: SourcedValue<ResourceNumber>,
        /// The authored power fraction when charging stops.
        stop_charging_power_draw_fraction: SourcedValue<ResourceNumber>,
        /// The authored power fraction at maximum charge rate.
        max_charge_rate_power_draw_fraction: SourcedValue<ResourceNumber>,
        /// The authored active power-draw fraction.
        active_power_draw_fraction: SourcedValue<ResourceNumber>,
        /// The authored tethering power-draw fraction.
        tethering_power_draw_fraction: SourcedValue<ResourceNumber>,
        /// The authored green-zone check range.
        green_zone_check_range: SourcedValue<ResourceNumber>,
        /// The weak reference for starting to charge.
        start_charging_interaction: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// The weak reference for cancelling the charge.
        cancel_charging_interaction: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// The weak reference for dispersing the charge.
        disperse_charge_interaction: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// Original members outside the typed fields.
        additional_fields: SourceArenaOutput<'source>,
    }
}
