// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::quantum_fields::QuantumFields;
use crate::CatalogError;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// The authored thermal energy inputs of one quantum drive.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct QuantumHeat<'source> {
    source_type: SourcedValue<&'source str>,
    pre_ramp_up_thermal_energy_draw: SourcedValue<ResourceNumber>,
    ramp_up_thermal_energy_draw: SourcedValue<ResourceNumber>,
    in_flight_thermal_energy_draw: SourcedValue<ResourceNumber>,
    ramp_down_thermal_energy_draw: SourcedValue<ResourceNumber>,
    post_ramp_down_thermal_energy_draw: SourcedValue<ResourceNumber>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> QuantumHeat<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = QuantumFields::new(source, notes);
        Ok(Self {
            source_type: fields.source_type()?,
            pre_ramp_up_thermal_energy_draw: fields.number("preRampUpThermalEnergyDraw")?,
            ramp_up_thermal_energy_draw: fields.number("rampUpThermalEnergyDraw")?,
            in_flight_thermal_energy_draw: fields.number("inFlightThermalEnergyDraw")?,
            ramp_down_thermal_energy_draw: fields.number("rampDownThermalEnergyDraw")?,
            post_ramp_down_thermal_energy_draw: fields.number("postRampDownThermalEnergyDraw")?,
            additional_fields: fields.additional_fields()?,
        })
    }
}
