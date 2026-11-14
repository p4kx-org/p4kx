// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::authored_link::HealthLink;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const TANK: &str = "SCItemFuelTankParams";
pub(super) const INTAKE: &str = "SCItemFuelIntakeParams";

/// Authored flow, interaction and audio parameters of a physical fuel tank.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct FuelTank<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    hydrogen_max_flow_multiplier: SourcedValue<ResourceFloat32>,
    quantum_max_flow_multiplier: SourcedValue<ResourceFloat32>,
    open_state: SourcedValue<HealthLink<'source>>,
    closed_state: SourcedValue<HealthLink<'source>>,
    pumping_state: SourcedValue<HealthLink<'source>>,
    fuel_flow_loop_start_audio_trigger: SourcedValue<FuelAudioTrigger<'source>>,
    fuel_flow_loop_stop_audio_trigger: SourcedValue<FuelAudioTrigger<'source>>,
    fuel_fill_drain_rate_audio_rtpc: SourcedValue<FuelAudioRtpc<'source>>,
    fuel_fill_level_audio_rtpc: SourcedValue<FuelAudioRtpc<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> FuelTank<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let hydrogen_max_flow_multiplier = source.single("hydrogenMaxFlowMultiplier", notes)?;
        let quantum_max_flow_multiplier = source.single("quantumMaxFlowMultiplier", notes)?;
        let open_state = source.link("openState", notes)?;
        let closed_state = source.link("closedState", notes)?;
        let pumping_state = source.link("pumpingState", notes)?;
        let fuel_flow_loop_start_audio_trigger = source.declared_class(
            "fuelFlowLoopStartAudioTrigger",
            notes,
            FuelAudioTrigger::read,
        )?;
        let fuel_flow_loop_stop_audio_trigger = source.declared_class(
            "fuelFlowLoopStopAudioTrigger",
            notes,
            FuelAudioTrigger::read,
        )?;
        let fuel_fill_drain_rate_audio_rtpc =
            source.declared_class("fuelFillDrainRateAudioRtpc", notes, FuelAudioRtpc::read)?;
        let fuel_fill_level_audio_rtpc =
            source.declared_class("fuelFillLevelAudioRtpc", notes, FuelAudioRtpc::read)?;
        let additional_fields = source.authored_members_except([
            (
                "hydrogenMaxFlowMultiplier",
                hydrogen_max_flow_multiplier.state(),
            ),
            (
                "quantumMaxFlowMultiplier",
                quantum_max_flow_multiplier.state(),
            ),
            ("openState", open_state.state()),
            ("closedState", closed_state.state()),
            ("pumpingState", pumping_state.state()),
            (
                "fuelFlowLoopStartAudioTrigger",
                fuel_flow_loop_start_audio_trigger.state(),
            ),
            (
                "fuelFlowLoopStopAudioTrigger",
                fuel_flow_loop_stop_audio_trigger.state(),
            ),
            (
                "fuelFillDrainRateAudioRtpc",
                fuel_fill_drain_rate_audio_rtpc.state(),
            ),
            ("fuelFillLevelAudioRtpc", fuel_fill_level_audio_rtpc.state()),
        ])?;
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            hydrogen_max_flow_multiplier,
            quantum_max_flow_multiplier,
            open_state,
            closed_state,
            pumping_state,
            fuel_flow_loop_start_audio_trigger,
            fuel_flow_loop_stop_audio_trigger,
            fuel_fill_drain_rate_audio_rtpc,
            fuel_fill_level_audio_rtpc,
            additional_fields,
        })
    }

    pub(super) const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct FuelAudioTrigger<'source> {
    source_type: SourcedValue<&'source str>,
    audio_trigger: SourcedValue<&'source str>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> FuelAudioTrigger<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let audio_trigger = source.borrowed_string("audioTrigger", notes)?;
        let additional_fields =
            source.authored_members_except([("audioTrigger", audio_trigger.state())])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            audio_trigger,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct FuelAudioRtpc<'source> {
    source_type: SourcedValue<&'source str>,
    rtpc: SourcedValue<&'source str>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> FuelAudioRtpc<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let rtpc = source.borrowed_string("rtpc", notes)?;
        let additional_fields = source.authored_members_except([("rtpc", rtpc.state())])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            rtpc,
            additional_fields,
        })
    }
}

/// Authored rate parameters of a physical fuel intake.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct FuelIntake<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    fuel_push_rate: SourcedValue<ResourceFloat32>,
    minimum_rate: SourcedValue<ResourceFloat32>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> FuelIntake<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let fuel_push_rate = source.single("fuelPushRate", notes)?;
        let minimum_rate = source.single("minimumRate", notes)?;
        let additional_fields = source.authored_members_except([
            ("fuelPushRate", fuel_push_rate.state()),
            ("minimumRate", minimum_rate.state()),
        ])?;
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            fuel_push_rate,
            minimum_rate,
            additional_fields,
        })
    }

    pub(super) const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }
}
