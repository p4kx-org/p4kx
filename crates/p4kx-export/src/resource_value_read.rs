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

/// Required presence of a numeric source field independently of its retained storage width.
#[derive(Clone, Copy)]
pub(crate) enum NumericPresence {
    Optional,
    Required,
}

/// Reads a source Single under its declared raw-number contract.
pub(crate) fn single(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<ResourceFloat32>, CatalogError> {
    fields.read(
        name,
        "a nullable Single",
        notes,
        |original| match original {
            Value::Single(value)
                if resource_raw_policy::field_contract(fields.source().type_name(), name)
                    == Some(ResourceNumberContract::Single) =>
            {
                fields.context().retain_number(
                    || fields.location(name),
                    fields.source().type_name(),
                    ResourceNumberContract::Single,
                    original,
                );
                Some(ResourceFloat32::new(*value))
            }
            _ => None,
        },
    )
}

/// Retains numeric storage while enforcing its independent Single field contract.
pub(crate) fn single_number(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
    let value = fields.read(name, "a nullable number", notes, |original| {
        let number = ResourceNumber::read(original)?;
        if matches!(original, Value::Single(_))
            && resource_raw_policy::field_contract(fields.source().type_name(), name)
                == Some(ResourceNumberContract::Single)
        {
            fields.context().retain_number(
                || fields.location(name),
                fields.source().type_name(),
                ResourceNumberContract::Single,
                original,
            );
        }
        Some(number)
    })?;
    if let Some(original) = fields.source().field(name)
        && value.value().is_some()
        && !matches!(original, Value::Single(_))
    {
        notes.push(FieldError::Malformed {
            source_type: fields.source().type_name().to_owned(),
            field: fields.location(name),
            expected: "a Single under the declared numeric field contract",
        });
    }
    Ok(value)
}

/// Reads a source number under its declared width contract.
pub(crate) fn number(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
    fields.read(name, "a nullable number", notes, |value| {
        let contract = resource_raw_policy::field_contract(fields.source().type_name(), name);
        let Some(contract @ (ResourceNumberContract::Number | ResourceNumberContract::Int32)) =
            contract
        else {
            return None;
        };
        let number = ResourceNumber::read(value)?;
        fields.context().retain_number(
            || fields.location(name),
            fields.source().type_name(),
            contract,
            value,
        );
        Some(number)
    })
}

/// Retains numeric storage while enforcing its independent Int32 field contract.
pub(crate) fn int32_number(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    presence: NumericPresence,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
    let value = number(fields, name, notes)?;
    let original = fields.source().field(name);
    let valid = matches!(original, Some(Value::Int32(_)))
        || (matches!(presence, NumericPresence::Optional)
            && matches!(original, None | Some(Value::Null)));
    if !valid && !matches!(original, Some(Value::Error(_))) {
        notes.push(FieldError::Malformed {
            source_type: fields.source().type_name().to_owned(),
            field: fields.location(name),
            expected: "an Int32 under the declared numeric field contract",
        });
    }
    Ok(value)
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
