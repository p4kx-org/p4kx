// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const BASE: &str = "InventoryContainerGridCellSizeBase";

/// One stored grid-cell unit and its original integer amount.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) enum InventoryGridCellSize<'source> {
    Centimeters {
        #[serde(flatten)]
        source: UnitSource<'source>,
        centimeters: SourcedValue<i32>,
    },
    Meters {
        #[serde(flatten)]
        source: UnitSource<'source>,
        meters: SourcedValue<i32>,
    },
    Base {
        #[serde(flatten)]
        source: UnitSource<'source>,
    },
    UnknownUnit {
        #[serde(flatten)]
        source: UnitSource<'source>,
    },
}

impl<'source> InventoryGridCellSize<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "InventoryContainerGridCellSizeCentimeters" => {
                let centimeters = source.int32("centimeters", notes)?;
                let source = UnitSource::read(source, [("centimeters", centimeters.state())])?;
                Self::Centimeters {
                    source,
                    centimeters,
                }
            }
            "InventoryContainerGridCellSizeMeters" => {
                let meters = source.int32("meters", notes)?;
                let source = UnitSource::read(source, [("meters", meters.state())])?;
                Self::Meters { source, meters }
            }
            BASE => Self::Base {
                source: UnitSource::read(source, [])?,
            },
            _ => Self::UnknownUnit {
                source: UnitSource::read(source, [])?,
            },
        })
    }
}
