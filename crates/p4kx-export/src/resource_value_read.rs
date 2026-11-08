// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, SourcedValue, Value, ValueError};

use crate::CatalogError;
use crate::catalog_read::CatalogFields;
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;
use crate::resource_raw_policy::{self, ResourceNumberContract};
use crate::resource_reference::ResourceReference;
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
) -> Result<SourcedValue<ResourceReference>, CatalogError> {
    let path = fields.location(name);
    let value = match fields.source().field(name) {
        Some(Value::Reference(target)) if target.location().is_none() => {
            Some(ResourceReference::resolved(
                fields.context(),
                &path,
                target.record_index(),
                CatalogGuid(target.guid()),
                target.record_path(),
                target.type_name(),
            )?)
        }
        Some(Value::Error(ValueError::DanglingReference { guid })) => {
            Some(ResourceReference::unresolved(
                fields.context(),
                &path,
                CatalogGuid(*guid),
                DefinitionStatus::Dangling,
            )?)
        }
        Some(Value::Error(ValueError::UnexpectedReferenceType { guid, .. })) => {
            Some(ResourceReference::unresolved(
                fields.context(),
                &path,
                CatalogGuid(*guid),
                DefinitionStatus::UnexpectedTargetType,
            )?)
        }
        _ => None,
    };
    value.map_or_else(
        || fields.read(name, "a nullable Record Reference", notes, |_| None),
        |value| fields.context().value(&path, SourceValue::Present(value)),
    )
}
