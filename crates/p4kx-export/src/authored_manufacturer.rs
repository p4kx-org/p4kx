// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

/// A manufacturer field with its original Reference or other declared storage.
#[derive(Debug, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
#[schemars(rename = "ItemManufacturer", transform = every_field_required)]
pub enum AuthoredManufacturer<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// The original manufacturer Record reference.
    Record(SourcedValue<Box<SourceRecordReference<'source>>>),
    /// An authored manufacturer declaration retained in its original value arena.
    Authored(SourcedValue<SourceArenaOutput<'source>>),
}

/// The value of a ship manufacturer's selected source member.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(untagged)]
#[schemars(transform = every_field_required)]
pub(crate) enum ManufacturerValue<'source> {
    #[schemars(with = "CatalogRecordReference")]
    Record(Box<SourceRecordReference<'source>>),
    Authored {
        authored: SourceArenaOutput<'source>,
    },
}

impl<'source> AuthoredManufacturer<'source> {
    pub(in crate::catalog_v3) fn into_value(self) -> SourcedValue<ManufacturerValue<'source>> {
        match self {
            Self::Record(cell) => cell.map(ManufacturerValue::Record),
            Self::Authored(cell) => cell.map(|authored| ManufacturerValue::Authored { authored }),
        }
    }

    pub(in crate::catalog_v3) fn read(
        source: Observed<'_, '_, '_, 'source>,
        field: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let is_scalar_reference =
            source
                .originals
                .field(source.owner, field)?
                .is_none_or(|(_, fact)| {
                    let storage = fact.field().storage();
                    storage.data_type() == DataType::Reference && !storage.is_array()
                });
        if is_scalar_reference {
            source.record_link(field, notes).map(Self::Record)
        } else {
            source
                .authored_member_with_errors(field)
                .map(Self::Authored)
        }
    }
}

#[cfg(test)]
#[path = "authored_manufacturer_cardinality_tests.rs"]
mod cardinality_tests;
