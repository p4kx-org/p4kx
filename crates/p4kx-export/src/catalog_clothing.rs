// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use crate::CatalogError;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const COMPONENT: &str = "SCItemClothingParams";

/// One original clothing component with its temperature, radiation and flight fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Clothing<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    temperature_resistance: SourcedValue<TemperatureResistance<'source>>,
    radiation_resistance: SourcedValue<RadiationResistance<'source>>,
    flight: SourcedValue<Flight<'source>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> Clothing<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let temperature_resistance = source.declared_class_to(
            "TemperatureResistance",
            "SCItemClothingTemperatureResistanceParams",
            notes,
            TemperatureResistance::read,
        )?;
        let radiation_resistance = source.declared_class_to(
            "RadiationResistance",
            "SCItemClothingRadiationResistanceParams",
            notes,
            RadiationResistance::read,
        )?;
        let flight = source.declared_class_to(
            "Flight",
            "SCItemClothingFlightParams",
            notes,
            Flight::read,
        )?;
        let original = UnitSource::read(
            source,
            [
                ("TemperatureResistance", temperature_resistance.state()),
                ("RadiationResistance", radiation_resistance.state()),
                ("Flight", flight.state()),
            ],
        )?;
        Ok(Self {
            owner,
            temperature_resistance,
            radiation_resistance,
            flight,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct TemperatureResistance<'source> {
    min_resistance: SourcedValue<ResourceFloat32>,
    max_resistance: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> TemperatureResistance<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let min_resistance = source.single("MinResistance", notes)?;
        let max_resistance = source.single("MaxResistance", notes)?;
        let original = UnitSource::read(
            source,
            [
                ("MinResistance", min_resistance.state()),
                ("MaxResistance", max_resistance.state()),
            ],
        )?;
        Ok(Self {
            min_resistance,
            max_resistance,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct RadiationResistance<'source> {
    maximum_radiation_capacity: SourcedValue<ResourceFloat32>,
    radiation_dissipation_rate: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> RadiationResistance<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let maximum_radiation_capacity = source.single("MaximumRadiationCapacity", notes)?;
        let radiation_dissipation_rate = source.single("RadiationDissipationRate", notes)?;
        let original = UnitSource::read(
            source,
            [
                (
                    "MaximumRadiationCapacity",
                    maximum_radiation_capacity.state(),
                ),
                (
                    "RadiationDissipationRate",
                    radiation_dissipation_rate.state(),
                ),
            ],
        )?;
        Ok(Self {
            maximum_radiation_capacity,
            radiation_dissipation_rate,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct Flight<'source> {
    g_force_resistance: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> Flight<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let g_force_resistance = source.single("gForceResistance", notes)?;
        let original =
            UnitSource::read(source, [("gForceResistance", g_force_resistance.state())])?;
        Ok(Self {
            g_force_resistance,
            original,
        })
    }
}
