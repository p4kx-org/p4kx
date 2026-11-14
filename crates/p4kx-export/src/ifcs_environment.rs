// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::ifcs_aero_surface::IfcsAeroSurface;
use super::ifcs_curve::IfcsCurve;
use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct IfcsEnvironment<'source> {
    aero_surfaces: SourcedValue<Vec<SourcedValue<IfcsAeroSurface<'source>>>>,
    atmospheric_flight_params: SourcedValue<IfcsAtmosphericFlight<'source>>,
    drag_multiplier: SourcedValue<ResourceFloat32>,
    lift_multiplier: SourcedValue<ResourceFloat32>,
    scm_max_drag_multiplier: SourcedValue<ResourceFloat32>,
    turbulence_params: SourcedValue<IfcsTurbulence<'source>>,
}

impl<'source> IfcsEnvironment<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            aero_surfaces: fields.declared_class_array(
                "aeroSurfaces",
                notes,
                IfcsAeroSurface::read,
            )?,
            atmospheric_flight_params: fields.declared_class(
                "atmosphericFlightParams",
                notes,
                IfcsAtmosphericFlight::read,
            )?,
            drag_multiplier: fields.single("dragMultiplier", notes)?,
            lift_multiplier: fields.single("liftMultiplier", notes)?,
            scm_max_drag_multiplier: fields.single("scmMaxDragMultiplier", notes)?,
            turbulence_params: fields.declared_class(
                "turbulenceParams",
                notes,
                IfcsTurbulence::read,
            )?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 6] {
        [
            ("aeroSurfaces", self.aero_surfaces.state()),
            (
                "atmosphericFlightParams",
                self.atmospheric_flight_params.state(),
            ),
            ("dragMultiplier", self.drag_multiplier.state()),
            ("liftMultiplier", self.lift_multiplier.state()),
            ("scmMaxDragMultiplier", self.scm_max_drag_multiplier.state()),
            ("turbulenceParams", self.turbulence_params.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct IfcsAtmosphericFlight<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    aerodynamic_fx_params: SourcedValue<SourceArenaOutput<'source>>,
    aerodynamics_params: SourcedValue<SourceArenaOutput<'source>>,
    command_control_params: SourcedValue<SourceArenaOutput<'source>>,
}

impl<'source> IfcsAtmosphericFlight<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let aerodynamic_fx_params = fields.authored_member("aerodynamicFXParams", notes)?;
        let aerodynamics_params = fields.authored_member("aerodynamicsParams", notes)?;
        let command_control_params = fields.authored_member("commandControlParams", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("aerodynamicFXParams", aerodynamic_fx_params.state()),
                ("aerodynamicsParams", aerodynamics_params.state()),
                ("commandControlParams", command_control_params.state()),
            ],
        )?;
        Ok(Self {
            source,
            aerodynamic_fx_params,
            aerodynamics_params,
            command_control_params,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct IfcsTurbulence<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    cloud_density_mapping: SourcedValue<IfcsCurve<'source>>,
    cloud_density_turbulence_amplitude: SourcedValue<ResourceFloat32>,
    ground_turbulence_altitude_max: SourcedValue<ResourceFloat32>,
    ground_turbulence_altitude_min: SourcedValue<ResourceFloat32>,
    ground_turbulence_enabled: SourcedValue<bool>,
    ground_turbulence_max_ang_acceleration: SourcedValue<ResourceFloat32>,
    ground_turbulence_noise: SourcedValue<SourceArenaOutput<'source>>,
    ground_turbulence_speed_max: SourcedValue<ResourceFloat32>,
    ground_turbulence_speed_min: SourcedValue<ResourceFloat32>,
    wind_speed_mapping: SourcedValue<IfcsCurve<'source>>,
    wind_speed_max: SourcedValue<ResourceFloat32>,
    wind_speed_min: SourcedValue<ResourceFloat32>,
    wind_speed_turbulence_amplitude: SourcedValue<ResourceFloat32>,
    wind_speed_turbulence_enabled: SourcedValue<bool>,
}

impl<'source> IfcsTurbulence<'source> {
    #[expect(
        clippy::too_many_lines,
        reason = "one read and source-state entry per original field"
    )]
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let cloud_density_mapping =
            fields.declared_class("cloudDensityMapping", notes, IfcsCurve::read)?;
        let cloud_density_turbulence_amplitude =
            fields.single("cloudDensityTurbulenceAmplitude", notes)?;
        let ground_turbulence_altitude_max = fields.single("groundTurbulenceAltitudeMax", notes)?;
        let ground_turbulence_altitude_min = fields.single("groundTurbulenceAltitudeMin", notes)?;
        let ground_turbulence_enabled = fields.boolean("groundTurbulenceEnabled", notes)?;
        let ground_turbulence_max_ang_acceleration =
            fields.single("groundTurbulenceMaxAngAcceleration", notes)?;
        let ground_turbulence_noise = fields.authored_member("groundTurbulenceNoise", notes)?;
        let ground_turbulence_speed_max = fields.single("groundTurbulenceSpeedMax", notes)?;
        let ground_turbulence_speed_min = fields.single("groundTurbulenceSpeedMin", notes)?;
        let wind_speed_mapping =
            fields.declared_class("windSpeedMapping", notes, IfcsCurve::read)?;
        let wind_speed_max = fields.single("windSpeedMax", notes)?;
        let wind_speed_min = fields.single("windSpeedMin", notes)?;
        let wind_speed_turbulence_amplitude =
            fields.single("windSpeedTurbulenceAmplitude", notes)?;
        let wind_speed_turbulence_enabled = fields.boolean("windSpeedTurbulenceEnabled", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("cloudDensityMapping", cloud_density_mapping.state()),
                (
                    "cloudDensityTurbulenceAmplitude",
                    cloud_density_turbulence_amplitude.state(),
                ),
                (
                    "groundTurbulenceAltitudeMax",
                    ground_turbulence_altitude_max.state(),
                ),
                (
                    "groundTurbulenceAltitudeMin",
                    ground_turbulence_altitude_min.state(),
                ),
                ("groundTurbulenceEnabled", ground_turbulence_enabled.state()),
                (
                    "groundTurbulenceMaxAngAcceleration",
                    ground_turbulence_max_ang_acceleration.state(),
                ),
                ("groundTurbulenceNoise", ground_turbulence_noise.state()),
                (
                    "groundTurbulenceSpeedMax",
                    ground_turbulence_speed_max.state(),
                ),
                (
                    "groundTurbulenceSpeedMin",
                    ground_turbulence_speed_min.state(),
                ),
                ("windSpeedMapping", wind_speed_mapping.state()),
                ("windSpeedMax", wind_speed_max.state()),
                ("windSpeedMin", wind_speed_min.state()),
                (
                    "windSpeedTurbulenceAmplitude",
                    wind_speed_turbulence_amplitude.state(),
                ),
                (
                    "windSpeedTurbulenceEnabled",
                    wind_speed_turbulence_enabled.state(),
                ),
            ],
        )?;
        Ok(Self {
            source,
            cloud_density_mapping,
            cloud_density_turbulence_amplitude,
            ground_turbulence_altitude_max,
            ground_turbulence_altitude_min,
            ground_turbulence_enabled,
            ground_turbulence_max_ang_acceleration,
            ground_turbulence_noise,
            ground_turbulence_speed_max,
            ground_turbulence_speed_min,
            wind_speed_mapping,
            wind_speed_max,
            wind_speed_min,
            wind_speed_turbulence_amplitude,
            wind_speed_turbulence_enabled,
        })
    }
}
