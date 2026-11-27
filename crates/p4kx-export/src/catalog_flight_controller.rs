// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_link::HealthLink;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

#[path = "flight_controller_limits.rs"]
mod limits;
pub use limits::{CollisionDetection, ShipRecall};

pub(in crate::catalog_v3) const COMPONENT: &str = "SCItemFlightControllerParams";

/// Physical flight-controller settings and input bindings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct FlightController<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    /// Original settings field.
    pub settings: OperatingSettings<'source>,
    #[serde(flatten)]
    /// Original interactions field.
    pub interactions: Interactions<'source>,
    /// Original collision detection field.
    pub collision_detection: SourcedValue<CollisionDetection<'source>>,
    #[serde(rename = "ShipRecall")]
    /// Original ship recall field.
    pub ship_recall: SourcedValue<ShipRecall<'source>>,
}

impl<'source> FlightController<'source> {
    /// Borrows the original component type and remaining fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the original physical Record and component position.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    pub(in crate::catalog_v3) fn read(
        owner: ComponentOwner,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let settings = OperatingSettings::read(fields, notes)?;
        let interactions = Interactions::read(fields, notes)?;
        let collision_detection =
            fields.declared_class("collisionDetection", notes, CollisionDetection::read)?;
        let ship_recall = fields.declared_pointer("ShipRecall", notes, ShipRecall::read)?;
        let source = UnitSource::read(
            fields,
            settings
                .projected()
                .into_iter()
                .chain(interactions.toggles())
                .chain(interactions.actions())
                .chain([
                    ("collisionDetection", collision_detection.state()),
                    ("ShipRecall", ship_recall.state()),
                ]),
        )?;
        Ok(Self {
            owner,
            source,
            settings,
            interactions,
            collision_detection,
            ship_recall,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored operating settings fields and their source evidence.
#[non_exhaustive]
pub struct OperatingSettings<'source> {
    /// Original fuel warning display time field.
    pub fuel_warning_display_time: SourcedValue<ResourceFloat32>,
    /// Original hover anim playback duration field.
    pub hover_anim_playback_duration: SourcedValue<ResourceFloat32>,
    /// Original hover anim play in landing mode field.
    pub hover_anim_play_in_landing_mode: SourcedValue<bool>,
    /// Original passive refuel warning threshold percentage field.
    pub passive_refuel_warning_threshold_percentage: SourcedValue<ResourceFloat32>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// Original hud params override field.
    pub hud_params_override: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// Original vehicle hud params override field.
    pub vehicle_hud_params_override: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[serde(rename = "PowerOffEffectTag")]
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// Original power off effect tag field.
    pub power_off_effect_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[serde(rename = "PowerOnEffectTag")]
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// Original power on effect tag field.
    pub power_on_effect_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
}

impl<'source> OperatingSettings<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            fuel_warning_display_time: fields.single("fuelWarningDisplayTime", notes)?,
            hover_anim_playback_duration: fields.single("hoverAnimPlaybackDuration", notes)?,
            hover_anim_play_in_landing_mode: fields.boolean("hoverAnimPlayInLandingMode", notes)?,
            passive_refuel_warning_threshold_percentage: fields
                .single("passiveRefuelWarningThresholdPercentage", notes)?,
            hud_params_override: fields.record_link("hudParamsOverride", notes)?,
            vehicle_hud_params_override: fields.record_link("vehicleHudParamsOverride", notes)?,
            power_off_effect_tag: fields.record_link("PowerOffEffectTag", notes)?,
            power_on_effect_tag: fields.record_link("PowerOnEffectTag", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 8] {
        [
            (
                "fuelWarningDisplayTime",
                self.fuel_warning_display_time.state(),
            ),
            (
                "hoverAnimPlaybackDuration",
                self.hover_anim_playback_duration.state(),
            ),
            (
                "hoverAnimPlayInLandingMode",
                self.hover_anim_play_in_landing_mode.state(),
            ),
            (
                "passiveRefuelWarningThresholdPercentage",
                self.passive_refuel_warning_threshold_percentage.state(),
            ),
            ("hudParamsOverride", self.hud_params_override.state()),
            (
                "vehicleHudParamsOverride",
                self.vehicle_hud_params_override.state(),
            ),
            ("PowerOffEffectTag", self.power_off_effect_tag.state()),
            ("PowerOnEffectTag", self.power_on_effect_tag.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored interactions fields and their source evidence.
#[non_exhaustive]
pub struct Interactions<'source> {
    #[serde(rename = "cycleTransformInteraction")]
    /// Original cycle transform field.
    pub cycle_transform: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "deployTransformInteraction")]
    /// Original deploy transform field.
    pub deploy_transform: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "retractTransformInteraction")]
    /// Original retract transform field.
    pub retract_transform: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "disableCoupledModeInteraction")]
    /// Original disable coupled mode field.
    pub disable_coupled_mode: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "enableCoupledModeInteraction")]
    /// Original enable coupled mode field.
    pub enable_coupled_mode: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "disableCruiseControlInteraction")]
    /// Original disable cruise control field.
    pub disable_cruise_control: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "enableCruiseControlInteraction")]
    /// Original enable cruise control field.
    pub enable_cruise_control: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "disableESPInteraction")]
    /// Original disable esp field.
    pub disable_esp: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "enableESPInteraction")]
    /// Original enable esp field.
    pub enable_esp: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "disableGSafetyInteraction")]
    /// Original disable g safety field.
    pub disable_g_safety: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "enableGSafetyInteraction")]
    /// Original enable g safety field.
    pub enable_g_safety: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "disableProximityAssistInteraction")]
    /// Original disable proximity assist field.
    pub disable_proximity_assist: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "enableProximityAssistInteraction")]
    /// Original enable proximity assist field.
    pub enable_proximity_assist: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "disableVTOLInteraction")]
    /// Original disable vtol field.
    pub disable_vtol: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "enableVTOLInteraction")]
    /// Original enable vtol field.
    pub enable_vtol: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "engineOffInteraction")]
    /// Original engine off field.
    pub engine_off: SourcedValue<HealthLink<'source>>,
    #[serde(rename = "engineOnInteraction")]
    /// Original engine on field.
    pub engine_on: SourcedValue<HealthLink<'source>>,
}

impl<'source> Interactions<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            cycle_transform: fields.link("cycleTransformInteraction", notes)?,
            deploy_transform: fields.link("deployTransformInteraction", notes)?,
            retract_transform: fields.link("retractTransformInteraction", notes)?,
            disable_coupled_mode: fields.link("disableCoupledModeInteraction", notes)?,
            enable_coupled_mode: fields.link("enableCoupledModeInteraction", notes)?,
            disable_cruise_control: fields.link("disableCruiseControlInteraction", notes)?,
            enable_cruise_control: fields.link("enableCruiseControlInteraction", notes)?,
            disable_esp: fields.link("disableESPInteraction", notes)?,
            enable_esp: fields.link("enableESPInteraction", notes)?,
            disable_g_safety: fields.link("disableGSafetyInteraction", notes)?,
            enable_g_safety: fields.link("enableGSafetyInteraction", notes)?,
            disable_proximity_assist: fields.link("disableProximityAssistInteraction", notes)?,
            enable_proximity_assist: fields.link("enableProximityAssistInteraction", notes)?,
            disable_vtol: fields.link("disableVTOLInteraction", notes)?,
            enable_vtol: fields.link("enableVTOLInteraction", notes)?,
            engine_off: fields.link("engineOffInteraction", notes)?,
            engine_on: fields.link("engineOnInteraction", notes)?,
        })
    }

    const fn toggles(&self) -> [(&'static str, SourceValueState); 12] {
        [
            (
                "disableCoupledModeInteraction",
                self.disable_coupled_mode.state(),
            ),
            (
                "enableCoupledModeInteraction",
                self.enable_coupled_mode.state(),
            ),
            (
                "disableCruiseControlInteraction",
                self.disable_cruise_control.state(),
            ),
            (
                "enableCruiseControlInteraction",
                self.enable_cruise_control.state(),
            ),
            ("disableESPInteraction", self.disable_esp.state()),
            ("enableESPInteraction", self.enable_esp.state()),
            ("disableGSafetyInteraction", self.disable_g_safety.state()),
            ("enableGSafetyInteraction", self.enable_g_safety.state()),
            (
                "disableProximityAssistInteraction",
                self.disable_proximity_assist.state(),
            ),
            (
                "enableProximityAssistInteraction",
                self.enable_proximity_assist.state(),
            ),
            ("disableVTOLInteraction", self.disable_vtol.state()),
            ("enableVTOLInteraction", self.enable_vtol.state()),
        ]
    }

    const fn actions(&self) -> [(&'static str, SourceValueState); 5] {
        [
            ("cycleTransformInteraction", self.cycle_transform.state()),
            ("deployTransformInteraction", self.deploy_transform.state()),
            (
                "retractTransformInteraction",
                self.retract_transform.state(),
            ),
            ("engineOffInteraction", self.engine_off.state()),
            ("engineOnInteraction", self.engine_on.state()),
        ]
    }
}

#[cfg(test)]
#[path = "flight_controller_tests.rs"]
mod tests;
