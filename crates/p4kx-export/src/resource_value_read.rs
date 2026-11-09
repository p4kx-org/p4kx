// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, Value};

use crate::CatalogError;
use crate::catalog_read::CatalogFields;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;
use crate::resource_raw_policy::{self, ResourceNumberContract};
use crate::source_fields::FieldError;

/// Reads a source Single under its declared raw-number contract.
pub(crate) fn single(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<ResourceFloat32>, CatalogError> {
    fields.read(name, "a nullable Single", notes, |value| match value {
        Value::Single(value)
            if resource_raw_policy::field_contract(fields.source().type_name(), name)
                == Some(ResourceNumberContract::Single) =>
        {
            Some(ResourceFloat32::new(*value))
        }
        _ => None,
    })
}

/// Reads a source number under its declared width contract.
pub(crate) fn number(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
    fields.read(name, "a nullable number", notes, |value| {
        (resource_raw_policy::field_contract(fields.source().type_name(), name)
            == Some(ResourceNumberContract::Number))
        .then(|| ResourceNumber::read(value))
        .flatten()
    })
}

/// Reads a Record Reference with independent target-header evidence.
pub(crate) fn reference(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<CatalogRecordReference>, CatalogError> {
    crate::catalog_read::reference_value(
        fields.context(),
        fields.source().field(name),
        fields.source().type_name(),
        &fields.location(name),
        notes,
    )
}
