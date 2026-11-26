// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use crate::resource_density_source::{
    ResourceDensityPayload, ResourceDensitySource, ResourceDensityUnit, ResourceDensityUnitPayload,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Density<'source> {
    source_type: Cell<&'source str>,
    payload: Cell<DensityPayload<'source>>,
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum DensityPayload<'source> {
    Density { density_unit: Cell<Unit<'source>> },
    MolarMass { molar_mass: Cell<Number<'source>> },
    EmptyBase,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Unit<'source> {
    source_type: Cell<&'source str>,
    payload: Cell<UnitPayload<'source>>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum UnitPayload<'source> {
    GramsPerCubicCentimeter { value: Cell<Number<'source>> },
    KilogramsPerCubicMeter { value: Cell<Number<'source>> },
    EmptyBase,
}

pub(super) fn read<'source>(
    path: &'static str,
    density: &'source SourcedValue<ResourceDensitySource<'_>>,
) -> Result<Cell<Density<'source>>, ReducedError> {
    completeness::value(path, density)?;
    Cell::from(density).try_map(|density| {
        completeness::value(path, density.source_type())?;
        completeness::value(path, density.payload())?;
        let payload = Cell::from(density.payload()).try_map(|payload| {
            Ok::<_, ReducedError>(match payload {
                ResourceDensityPayload::Density(density) => DensityPayload::Density {
                    density_unit: unit(path, density.density_unit())?,
                },
                ResourceDensityPayload::MolarMass(mass) => {
                    completeness::value(path, mass.molar_mass())?;
                    DensityPayload::MolarMass {
                        molar_mass: Cell::from(mass.molar_mass()).map(Number::from),
                    }
                }
                ResourceDensityPayload::EmptyBase => DensityPayload::EmptyBase,
            })
        })?;
        Ok(Density {
            source_type: Cell::from(density.source_type()).map(|name| *name),
            payload,
        })
    })
}

fn unit<'source>(
    path: &'static str,
    unit: &'source SourcedValue<ResourceDensityUnit<'_>>,
) -> Result<Cell<Unit<'source>>, ReducedError> {
    completeness::value(path, unit)?;
    Cell::from(unit).try_map(|unit| {
        completeness::value(path, unit.source_type())?;
        completeness::value(path, unit.payload())?;
        let payload = Cell::from(unit.payload()).try_map(|payload| {
            Ok::<_, ReducedError>(match payload {
                ResourceDensityUnitPayload::GramsPerCubicCentimeter(quantity) => {
                    completeness::value(path, quantity.value())?;
                    UnitPayload::GramsPerCubicCentimeter {
                        value: Cell::from(quantity.value()).map(Number::from),
                    }
                }
                ResourceDensityUnitPayload::KilogramsPerCubicMeter(quantity) => {
                    completeness::value(path, quantity.value())?;
                    UnitPayload::KilogramsPerCubicMeter {
                        value: Cell::from(quantity.value()).map(Number::from),
                    }
                }
                ResourceDensityUnitPayload::EmptyBase => UnitPayload::EmptyBase,
            })
        })?;
        Ok(Unit {
            source_type: Cell::from(unit.source_type()).map(|name| *name),
            payload,
        })
    })
}
