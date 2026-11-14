// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_effect_resource::Rtpc;
use super::authored_fields::Observed;
use super::prepared_definition::PreparedDefinition;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) const SOURCE_TYPE: &str = "ProceduralPlanetAudioData";
const ENTRIES: &str = "entries";
const DISTURBANCE_LISTS: &str = "disturbanceLists";
const EFFECT_TAG_MOVEMENT_RTPCS: &str = "effectTagMovementRtpcs";

/// An original DataForge planet audio record and its independently sourced controls.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ProceduralPlanetAudio<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    entries: SourcedValue<SourceArenaOutput<'source>>,
    disturbance_lists: SourcedValue<SourceArenaOutput<'source>>,
    effect_tag_movement_rtpcs: SourcedValue<SourceArenaOutput<'source>>,
    #[serde(flatten)]
    controls: PlanetAudioControls<'source>,
    additional_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
struct PlanetAudioRtpc<'source> {
    #[serde(flatten)]
    setting: Rtpc,
    #[serde(rename = "additionalFields")]
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> PlanetAudioRtpc<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let setting = Rtpc::read(fields, notes)?;
        let additional_fields = fields.authored_members_except([setting.projected_member()])?;
        Ok(Self {
            setting,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct PlanetAudioControls<'source> {
    #[serde(rename = "breathabilityRtpc")]
    breathability: SourcedValue<PlanetAudioRtpc<'source>>,
    #[serde(rename = "disturbanceVelocityRtpc")]
    disturbance_velocity: SourcedValue<PlanetAudioRtpc<'source>>,
    #[serde(rename = "firstRoomIsPlanetRoomRtpc")]
    first_room_is_planet_room: SourcedValue<PlanetAudioRtpc<'source>>,
    #[serde(rename = "firstRoomIsPlanetRoomNotIncludingShipsRtpc")]
    first_room_is_planet_room_not_including_ships: SourcedValue<PlanetAudioRtpc<'source>>,
    #[serde(rename = "firstRoomIsPlanetRoomOrVehicleRoomRtpc")]
    first_room_is_planet_room_or_vehicle_room: SourcedValue<PlanetAudioRtpc<'source>>,
    #[serde(rename = "humidityRtpc")]
    humidity: SourcedValue<PlanetAudioRtpc<'source>>,
    #[serde(rename = "pressureRtpc")]
    pressure: SourcedValue<PlanetAudioRtpc<'source>>,
    #[serde(rename = "temperatureRtpc")]
    temperature: SourcedValue<PlanetAudioRtpc<'source>>,
}

const BREATHABILITY: &str = "breathabilityRtpc";
const DISTURBANCE_VELOCITY: &str = "disturbanceVelocityRtpc";
const PLANET_ROOM: &str = "firstRoomIsPlanetRoomRtpc";
const PLANET_ROOM_EXCEPT_SHIPS: &str = "firstRoomIsPlanetRoomNotIncludingShipsRtpc";
const PLANET_OR_VEHICLE_ROOM: &str = "firstRoomIsPlanetRoomOrVehicleRoomRtpc";
const HUMIDITY: &str = "humidityRtpc";
const PRESSURE: &str = "pressureRtpc";
const TEMPERATURE: &str = "temperatureRtpc";

impl<'source> PlanetAudioControls<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut rtpc =
            |name| fields.declared_class_to(name, "AudioRtpc", notes, PlanetAudioRtpc::read);
        Ok(Self {
            breathability: rtpc(BREATHABILITY)?,
            disturbance_velocity: rtpc(DISTURBANCE_VELOCITY)?,
            first_room_is_planet_room: rtpc(PLANET_ROOM)?,
            first_room_is_planet_room_not_including_ships: rtpc(PLANET_ROOM_EXCEPT_SHIPS)?,
            first_room_is_planet_room_or_vehicle_room: rtpc(PLANET_OR_VEHICLE_ROOM)?,
            humidity: rtpc(HUMIDITY)?,
            pressure: rtpc(PRESSURE)?,
            temperature: rtpc(TEMPERATURE)?,
        })
    }

    const fn projected_members(&self) -> [(&'static str, p4kx_extraction::SourceValueState); 8] {
        [
            (BREATHABILITY, self.breathability.state()),
            (DISTURBANCE_VELOCITY, self.disturbance_velocity.state()),
            (PLANET_ROOM, self.first_room_is_planet_room.state()),
            (
                PLANET_ROOM_EXCEPT_SHIPS,
                self.first_room_is_planet_room_not_including_ships.state(),
            ),
            (
                PLANET_OR_VEHICLE_ROOM,
                self.first_room_is_planet_room_or_vehicle_room.state(),
            ),
            (HUMIDITY, self.humidity.state()),
            (PRESSURE, self.pressure.state()),
            (TEMPERATURE, self.temperature.state()),
        ]
    }
}

impl<'source> ProceduralPlanetAudio<'source> {
    fn project(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let entries = fields.authored_member(ENTRIES, notes)?;
        let disturbance_lists = fields.authored_member(DISTURBANCE_LISTS, notes)?;
        let effect_tag_movement_rtpcs = fields.authored_member(EFFECT_TAG_MOVEMENT_RTPCS, notes)?;
        let controls = PlanetAudioControls::read(fields, notes)?;
        let additional_fields = fields.authored_members_except(
            [
                (ENTRIES, entries.state()),
                (DISTURBANCE_LISTS, disturbance_lists.state()),
                (EFFECT_TAG_MOVEMENT_RTPCS, effect_tag_movement_rtpcs.state()),
            ]
            .into_iter()
            .chain(controls.projected_members()),
        )?;
        Ok(Self {
            source,
            entries,
            disturbance_lists,
            effect_tag_movement_rtpcs,
            controls,
            additional_fields,
        })
    }
}

pub(crate) type PreparedPlanetAudio<'source> =
    PreparedDefinition<'source, ProceduralPlanetAudio<'source>>;

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(PreparedPlanetAudio<'source>, Vec<FieldError>), CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        |source, fields, _, notes| ProceduralPlanetAudio::project(source, fields, notes),
    )
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<PreparedPlanetAudio<'source>>,
) -> CatalogTable<ProceduralPlanetAudio<'source>> {
    PreparedDefinition::order(&mut rows);
    PreparedDefinition::table(forge, SOURCE_TYPE, rows)
}

pub(crate) fn sources<'rows, 'source>(
    rows: &'rows [PreparedPlanetAudio<'source>],
) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
    PreparedDefinition::sources(Kind::ProceduralPlanetAudio, rows)
}
