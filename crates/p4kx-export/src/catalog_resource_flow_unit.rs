// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use crate::CatalogError;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// The original unit of a resource amount per second.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(super) enum ResourceFlowUnit<'source> {
    Base {
        #[serde(flatten)]
        source: UnitSource<'source>,
    },
    CentiResourceUnit {
        #[serde(flatten)]
        source: UnitSource<'source>,
        centi_resource_units: SourcedValue<ResourceNumber>,
    },
    MicroResourceUnit {
        #[serde(flatten)]
        source: UnitSource<'source>,
        micro_resource_units: SourcedValue<ResourceNumber>,
    },
    StandardResourceUnit {
        #[serde(flatten)]
        source: UnitSource<'source>,
        standard_resource_units: SourcedValue<ResourceNumber>,
    },
    PowerSegmentResourceUnit {
        #[serde(flatten)]
        source: UnitSource<'source>,
        units: SourcedValue<ResourceNumber>,
    },
    UnknownUnit {
        #[serde(flatten)]
        source: UnitSource<'source>,
    },
}

impl<'source> ResourceFlowUnit<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "SBaseResourceUnit" => Self::Base {
                source: UnitSource::read(source, [])?,
            },
            "SCentiResourceUnit" => {
                let (source, centi_resource_units) = member(source, "centiResourceUnits", notes)?;
                Self::CentiResourceUnit {
                    source,
                    centi_resource_units,
                }
            }
            "SMicroResourceUnit" => {
                let (source, micro_resource_units) = member(source, "microResourceUnits", notes)?;
                Self::MicroResourceUnit {
                    source,
                    micro_resource_units,
                }
            }
            "SStandardResourceUnit" => {
                let (source, standard_resource_units) =
                    member(source, "standardResourceUnits", notes)?;
                Self::StandardResourceUnit {
                    source,
                    standard_resource_units,
                }
            }
            "SPowerSegmentResourceUnit" => {
                let (source, units) = member(source, "units", notes)?;
                Self::PowerSegmentResourceUnit { source, units }
            }
            _ => Self::UnknownUnit {
                source: UnitSource::read(source, [])?,
            },
        })
    }
}

fn member<'source>(
    source: Observed<'_, '_, '_, 'source>,
    field: &'static str,
    notes: &mut Vec<FieldError>,
) -> Result<(UnitSource<'source>, SourcedValue<ResourceNumber>), CatalogError> {
    let amount = source.authored_number(field, notes)?;
    let identity = UnitSource::read(source, [(field, amount.state())])?;
    Ok((identity, amount))
}
