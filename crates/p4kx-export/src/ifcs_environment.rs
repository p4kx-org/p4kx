// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::OrderedCurve;
use super::ifcs_aero_surface::IfcsAeroSurface;
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
/// Original authored ifcs environment fields and their source evidence.
#[non_exhaustive]
pub struct IfcsEnvironment<'source> {
    /// Original aero surfaces field.
    pub aero_surfaces: SourcedValue<Vec<SourcedValue<IfcsAeroSurface<'source>>>>,
    /// Original atmospheric flight params field.
    pub atmospheric_flight_params: SourcedValue<IfcsAtmosphericFlight<'source>>,
    /// Original drag multiplier field.
    pub drag_multiplier: SourcedValue<ResourceFloat32>,
    /// Original lift multiplier field.
    pub lift_multiplier: SourcedValue<ResourceFloat32>,
    /// Original scm max drag multiplier field.
    pub scm_max_drag_multiplier: SourcedValue<ResourceFloat32>,
    /// Original turbulence params field.
    pub turbulence_params: SourcedValue<IfcsTurbulence<'source>>,
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
/// Original authored ifcs atmospheric flight fields and their source evidence.
#[non_exhaustive]
pub struct IfcsAtmosphericFlight<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    /// Original aerodynamic fx params field.
    pub aerodynamic_fx_params: SourcedValue<SourceArenaOutput<'source>>,
    /// Original aerodynamics params field.
    pub aerodynamics_params: SourcedValue<SourceArenaOutput<'source>>,
    /// Original command control params field.
    pub command_control_params: SourcedValue<SourceArenaOutput<'source>>,
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
/// Original authored ifcs turbulence fields and their source evidence.
#[non_exhaustive]
pub struct IfcsTurbulence<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    /// Original cloud density mapping field.
    pub cloud_density_mapping: SourcedValue<OrderedCurve<'source>>,
    /// Original cloud density turbulence amplitude field.
    pub cloud_density_turbulence_amplitude: SourcedValue<ResourceFloat32>,
    /// Original ground turbulence altitude max field.
    pub ground_turbulence_altitude_max: SourcedValue<ResourceFloat32>,
    /// Original ground turbulence altitude min field.
    pub ground_turbulence_altitude_min: SourcedValue<ResourceFloat32>,
    /// Original ground turbulence enabled field.
    pub ground_turbulence_enabled: SourcedValue<bool>,
    /// Original ground turbulence max ang acceleration field.
    pub ground_turbulence_max_ang_acceleration: SourcedValue<ResourceFloat32>,
    /// Original ground turbulence noise field.
    pub ground_turbulence_noise: SourcedValue<SourceArenaOutput<'source>>,
    /// Original ground turbulence speed max field.
    pub ground_turbulence_speed_max: SourcedValue<ResourceFloat32>,
    /// Original ground turbulence speed min field.
    pub ground_turbulence_speed_min: SourcedValue<ResourceFloat32>,
    /// Original wind speed mapping field.
    pub wind_speed_mapping: SourcedValue<OrderedCurve<'source>>,
    /// Original wind speed max field.
    pub wind_speed_max: SourcedValue<ResourceFloat32>,
    /// Original wind speed min field.
    pub wind_speed_min: SourcedValue<ResourceFloat32>,
    /// Original wind speed turbulence amplitude field.
    pub wind_speed_turbulence_amplitude: SourcedValue<ResourceFloat32>,
    /// Original wind speed turbulence enabled field.
    pub wind_speed_turbulence_enabled: SourcedValue<bool>,
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
            fields.declared_class("cloudDensityMapping", notes, OrderedCurve::read)?;
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
            fields.declared_class("windSpeedMapping", notes, OrderedCurve::read)?;
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
