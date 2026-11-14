// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::starmap_navigation::{
    LOCATION_TYPE, StarmapLocation, StarmapQuantumTravel, TRAVEL_TYPE,
};
use super::starmap_radar::RadarProperties;
use crate::CatalogError;
use crate::resource_float::{ResourceFloat32, ResourceFloat64};
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct StarmapObjectOwned<'source> {
    radar_properties: SourcedValue<RadarProperties<'source>>,
    quantum_travel_data: SourcedValue<StarmapQuantumTravel<'source>>,
    location_params: SourcedValue<StarmapLocation<'source>>,
    asteroid_rings: SourcedValue<Vec<SourcedValue<StarmapAsteroidRing<'source>>>>,
    location_audio_play_trigger: SourcedValue<SourceArenaOutput<'source>>,
    location_audio_stop_trigger: SourcedValue<SourceArenaOutput<'source>>,
}

impl<'source> StarmapObjectOwned<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            radar_properties: RadarProperties::read_field(fields, notes)?,
            quantum_travel_data: fields.strong_pointer_family_to(
                "quantumTravelData",
                TRAVEL_TYPE,
                notes,
                StarmapQuantumTravel::read,
            )?,
            location_params: fields.strong_pointer_family_to(
                "locationParams",
                LOCATION_TYPE,
                notes,
                StarmapLocation::read,
            )?,
            asteroid_rings: fields.class_array_to(
                "asteroidRings",
                "StarMapAsteroidRing",
                notes,
                StarmapAsteroidRing::read,
            )?,
            location_audio_play_trigger: audio_trigger(fields, "locationAudioPlayTrigger", notes)?,
            location_audio_stop_trigger: audio_trigger(fields, "locationAudioStopTrigger", notes)?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 6] {
        [
            ("radarProperties", self.radar_properties.state()),
            ("quantumTravelData", self.quantum_travel_data.state()),
            ("locationParams", self.location_params.state()),
            ("asteroidRings", self.asteroid_rings.state()),
            (
                "locationAudioPlayTrigger",
                self.location_audio_play_trigger.state(),
            ),
            (
                "locationAudioStopTrigger",
                self.location_audio_stop_trigger.state(),
            ),
        ]
    }
}

fn audio_trigger<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    field: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<SourceArenaOutput<'source>>, CatalogError> {
    fields.declared_class_to(field, "GlobalResourceAudio", notes, |fields, _| {
        fields.authored_members()
    })
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct StarmapAsteroidRing<'source> {
    source_type: SourcedValue<&'source str>,
    density_scale: SourcedValue<ResourceFloat32>,
    size_scale: SourcedValue<ResourceFloat32>,
    inner_radius: SourcedValue<ResourceFloat64>,
    outer_radius: SourcedValue<ResourceFloat64>,
    depth: SourcedValue<ResourceFloat32>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> StarmapAsteroidRing<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let density_scale = fields.single("densityScale", notes)?;
        let size_scale = fields.single("sizeScale", notes)?;
        let inner_radius = fields.double("innerRadius", notes)?;
        let outer_radius = fields.double("outerRadius", notes)?;
        let depth = fields.single("depth", notes)?;
        let authored_fields = fields.authored_members_except([
            ("densityScale", density_scale.state()),
            ("sizeScale", size_scale.state()),
            ("innerRadius", inner_radius.state()),
            ("outerRadius", outer_radius.state()),
            ("depth", depth.state()),
        ])?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            density_scale,
            size_scale,
            inner_radius,
            outer_radius,
            depth,
            authored_fields,
        })
    }
}
