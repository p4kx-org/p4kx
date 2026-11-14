// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

use super::cargo_source::{BASE, CargoUnit};

/// A capacity retains the concrete unit and its own stored numeric member.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(super) enum ResourceCapacity<'source> {
    Base {
        #[serde(flatten)]
        source: UnitSource<'source>,
    },
    CentiCargoUnit {
        #[serde(flatten)]
        source: UnitSource<'source>,
        #[serde(rename = "centiSCU")]
        centi_scu: SourcedValue<i32>,
    },
    MicroCargoUnit {
        #[serde(flatten)]
        source: UnitSource<'source>,
        #[serde(rename = "microSCU")]
        micro_scu: SourcedValue<i32>,
    },
    StandardCargoUnit {
        #[serde(flatten)]
        source: UnitSource<'source>,
        standard_cargo_units: SourcedValue<ResourceFloat32>,
    },
    UnknownUnit {
        #[serde(flatten)]
        source: UnitSource<'source>,
    },
}

impl<'source> ResourceCapacity<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let name = source.source_type()?;
        let Some(unit) = CargoUnit::from_source(name) else {
            let source = UnitSource::read(source, [])?;
            return Ok(if name == BASE {
                Self::Base { source }
            } else {
                Self::UnknownUnit { source }
            });
        };
        let field = unit.amount_field();
        Ok(match unit {
            CargoUnit::Centi => {
                let centi_scu = source.int32(field, notes)?;
                let source = UnitSource::read(source, [(field, centi_scu.state())])?;
                Self::CentiCargoUnit { source, centi_scu }
            }
            CargoUnit::Micro => {
                let micro_scu = source.int32(field, notes)?;
                let source = UnitSource::read(source, [(field, micro_scu.state())])?;
                Self::MicroCargoUnit { source, micro_scu }
            }
            CargoUnit::Standard => {
                let standard_cargo_units = source.single(field, notes)?;
                let source = UnitSource::read(source, [(field, standard_cargo_units.state())])?;
                Self::StandardCargoUnit {
                    source,
                    standard_cargo_units,
                }
            }
        })
    }
}
