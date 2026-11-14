// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// Authored thermal connection parameters of an original component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ResourceHeatConnection<'source> {
    #[serde(flatten)]
    thermal: HeatTransfer,
    #[serde(flatten)]
    performance: HeatPerformance,
    #[serde(flatten)]
    cooling: HeatCooling,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourceHeatConnection<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let thermal = HeatTransfer::read(source, notes)?;
        let performance = HeatPerformance::read(source, notes)?;
        let cooling = HeatCooling::read(source, notes)?;
        let additional_fields = source.authored_members_except(
            thermal
                .projected()
                .into_iter()
                .chain(performance.projected())
                .chain(cooling.projected()),
        )?;
        Ok(Self {
            thermal,
            performance,
            cooling,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
#[schemars(transform = every_field_required)]
struct HeatTransfer {
    thermal_energy_base: SourcedValue<ResourceFloat32>,
    thermal_energy_draw: SourcedValue<ResourceFloat32>,
    thermal_conductivity: SourcedValue<ResourceFloat32>,
    specific_heat_capacity: SourcedValue<ResourceFloat32>,
    mass: SourcedValue<ResourceFloat32>,
    surface_area: SourcedValue<ResourceFloat32>,
}

impl HeatTransfer {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            thermal_energy_base: source.single("ThermalEnergyBase", notes)?,
            thermal_energy_draw: source.single("ThermalEnergyDraw", notes)?,
            thermal_conductivity: source.single("ThermalConductivity", notes)?,
            specific_heat_capacity: source.single("SpecificHeatCapacity", notes)?,
            mass: source.single("Mass", notes)?,
            surface_area: source.single("SurfaceArea", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 6] {
        [
            ("ThermalEnergyBase", self.thermal_energy_base.state()),
            ("ThermalEnergyDraw", self.thermal_energy_draw.state()),
            ("ThermalConductivity", self.thermal_conductivity.state()),
            ("SpecificHeatCapacity", self.specific_heat_capacity.state()),
            ("Mass", self.mass.state()),
            ("SurfaceArea", self.surface_area.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
#[schemars(transform = every_field_required)]
struct HeatPerformance {
    #[serde(rename = "TemperatureToIR")]
    temperature_to_ir: SourcedValue<ResourceFloat32>,
    #[serde(rename = "StartIRTemperature")]
    start_ir_temperature: SourcedValue<ResourceFloat32>,
    overpower_heat: SourcedValue<ResourceFloat32>,
    overclock_threshold_min_heat: SourcedValue<ResourceFloat32>,
    overclock_threshold_max_heat: SourcedValue<ResourceFloat32>,
}

impl HeatPerformance {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            temperature_to_ir: source.single("TemperatureToIR", notes)?,
            start_ir_temperature: source.single("StartIRTemperature", notes)?,
            overpower_heat: source.single("OverpowerHeat", notes)?,
            overclock_threshold_min_heat: source.single("OverclockThresholdMinHeat", notes)?,
            overclock_threshold_max_heat: source.single("OverclockThresholdMaxHeat", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 5] {
        [
            ("TemperatureToIR", self.temperature_to_ir.state()),
            ("StartIRTemperature", self.start_ir_temperature.state()),
            ("OverpowerHeat", self.overpower_heat.state()),
            (
                "OverclockThresholdMinHeat",
                self.overclock_threshold_min_heat.state(),
            ),
            (
                "OverclockThresholdMaxHeat",
                self.overclock_threshold_max_heat.state(),
            ),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
#[schemars(transform = every_field_required)]
struct HeatCooling {
    start_cooling_temperature: SourcedValue<ResourceFloat32>,
    max_cooling_rate: SourcedValue<ResourceFloat32>,
    max_temperature: SourcedValue<ResourceFloat32>,
    overheat_temperature: SourcedValue<ResourceFloat32>,
    recovery_temperature: SourcedValue<ResourceFloat32>,
    min_temperature: SourcedValue<ResourceFloat32>,
    misfire_min_temperature: SourcedValue<ResourceFloat32>,
    misfire_max_temperature: SourcedValue<ResourceFloat32>,
}

impl HeatCooling {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            start_cooling_temperature: source.single("StartCoolingTemperature", notes)?,
            max_cooling_rate: source.single("MaxCoolingRate", notes)?,
            max_temperature: source.single("MaxTemperature", notes)?,
            overheat_temperature: source.single("OverheatTemperature", notes)?,
            recovery_temperature: source.single("RecoveryTemperature", notes)?,
            min_temperature: source.single("MinTemperature", notes)?,
            misfire_min_temperature: source.single("MisfireMinTemperature", notes)?,
            misfire_max_temperature: source.single("MisfireMaxTemperature", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 8] {
        [
            (
                "StartCoolingTemperature",
                self.start_cooling_temperature.state(),
            ),
            ("MaxCoolingRate", self.max_cooling_rate.state()),
            ("MaxTemperature", self.max_temperature.state()),
            ("OverheatTemperature", self.overheat_temperature.state()),
            ("RecoveryTemperature", self.recovery_temperature.state()),
            ("MinTemperature", self.min_temperature.state()),
            (
                "MisfireMinTemperature",
                self.misfire_min_temperature.state(),
            ),
            (
                "MisfireMaxTemperature",
                self.misfire_max_temperature.state(),
            ),
        ]
    }
}
