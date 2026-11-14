// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::ExportAccountingError;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

use crate::catalog_v3::cargo_source::{BASE, CENTI, CargoUnit, MICRO, STANDARD};

/// The authored conversion variant and its original quantity.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum AmmoConversion {
    Base,
    Standard {
        standard_cargo_units: SourcedValue<ResourceFloat32>,
    },
    Centi {
        centi_scu: SourcedValue<i32>,
    },
    Micro {
        micro_scu: SourcedValue<i32>,
    },
}

impl AmmoConversion {
    pub(super) const TYPES: &[&str] = &[BASE, STANDARD, CENTI, MICRO];

    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let name = source.source_type()?;
        if name == BASE {
            return Ok(Self::Base);
        }
        let unit =
            CargoUnit::from_source(name).ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let field = unit.amount_field();
        Ok(match unit {
            CargoUnit::Standard => Self::Standard {
                standard_cargo_units: source.single(field, notes)?,
            },
            CargoUnit::Centi => Self::Centi {
                centi_scu: source.int32(field, notes)?,
            },
            CargoUnit::Micro => Self::Micro {
                micro_scu: source.int32(field, notes)?,
            },
        })
    }
}
