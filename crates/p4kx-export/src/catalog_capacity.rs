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

pub(super) const BASE: &str = "SBaseCargoUnit";

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
        Ok(match source.source_type()? {
            BASE => Self::Base {
                source: UnitSource::read(source, [])?,
            },
            "SCentiCargoUnit" => {
                let centi_scu = source.int32("centiSCU", notes)?;
                let source = UnitSource::read(source, [("centiSCU", centi_scu.state())])?;
                Self::CentiCargoUnit { source, centi_scu }
            }
            "SMicroCargoUnit" => {
                let micro_scu = source.int32("microSCU", notes)?;
                let source = UnitSource::read(source, [("microSCU", micro_scu.state())])?;
                Self::MicroCargoUnit { source, micro_scu }
            }
            "SStandardCargoUnit" => {
                let standard_cargo_units = source.single("standardCargoUnits", notes)?;
                let source = UnitSource::read(
                    source,
                    [("standardCargoUnits", standard_cargo_units.state())],
                )?;
                Self::StandardCargoUnit {
                    source,
                    standard_cargo_units,
                }
            }
            _ => Self::UnknownUnit {
                source: UnitSource::read(source, [])?,
            },
        })
    }
}
