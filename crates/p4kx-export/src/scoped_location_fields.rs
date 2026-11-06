// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value, ValueError};

use crate::guid_text::CatalogGuid;
use crate::record_read::ReadError;
use crate::scoped_location_link::{DefinitionReference, DefinitionStatus, PropertyReference};
use crate::scoped_location_weak;
use crate::spawn_read::Reader;

#[derive(Debug, thiserror::Error)]
enum FieldError {
    #[error("ScopedLocation {owner}.{field} must hold {expected}")]
    Malformed {
        owner: String,
        field: &'static str,
        expected: &'static str,
    },
    #[error("ScopedLocation {owner}.{field} is unreadable: {source}")]
    Unreadable {
        owner: String,
        field: &'static str,
        source: ValueError,
    },
}

fn unexpected(
    owner: &StructValue<'_>,
    field: &'static str,
    value: &Value<'_>,
    expected: &'static str,
) -> ReadError {
    match value {
        Value::Error(source) => Box::new(FieldError::Unreadable {
            owner: owner.type_name().to_owned(),
            field,
            source: *source,
        }),
        _ => Box::new(FieldError::Malformed {
            owner: owner.type_name().to_owned(),
            field,
            expected,
        }),
    }
}

/// Returns an optional source struct without dropping a present invalid value.
pub(crate) fn structure<'t, 'f>(
    parent: &'t StructValue<'f>,
    field: &'static str,
) -> Result<Option<&'t StructValue<'f>>, ReadError> {
    match parent.field(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Struct(value)) => Ok(Some(value)),
        Some(value) => Err(unexpected(parent, field, value, "a nullable StrongPointer")),
    }
}

/// Returns a raw enum option, preserving unknown names and an empty stored option.
pub(crate) fn enum_name(
    parent: &StructValue<'_>,
    field: &'static str,
) -> Result<Option<String>, ReadError> {
    match parent.field(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Enum(name)) => Ok(Some((*name).to_owned())),
        Some(value) => Err(unexpected(parent, field, value, "an enum option")),
    }
}

/// Returns a source definition Reference, retaining a missing target's GUID.
pub(crate) fn definition(
    parent: &StructValue<'_>,
    field: &'static str,
) -> Result<Option<DefinitionReference>, ReadError> {
    parent
        .field(field)
        .map(|value| definition_value(parent, field, value))
        .transpose()
        .map(Option::flatten)
}

/// Returns the ordered source definition References, including null entries and repetitions.
pub(crate) fn definitions(
    parent: &StructValue<'_>,
    field: &'static str,
) -> Result<Option<Vec<Option<DefinitionReference>>>, ReadError> {
    array(parent, field, |value| {
        definition_value(parent, field, value)
    })
}

fn definition_value(
    parent: &StructValue<'_>,
    field: &'static str,
    value: &Value<'_>,
) -> Result<Option<DefinitionReference>, ReadError> {
    Ok(match value {
        Value::Null => None,
        Value::Reference(target) if target.location().is_none() => Some(DefinitionReference {
            guid: CatalogGuid(target.guid()),
            record_path: Some(target.record_path().to_owned()),
            target_type: Some(target.type_name().to_owned()),
            status: DefinitionStatus::Present,
        }),
        Value::Error(ValueError::DanglingReference { guid }) => Some(DefinitionReference {
            guid: CatalogGuid(*guid),
            record_path: None,
            target_type: None,
            status: DefinitionStatus::Dangling,
        }),
        _ => {
            return Err(unexpected(
                parent,
                field,
                value,
                "a nullable Record Reference",
            ));
        }
    })
}

/// Returns a weak property link with its available source identity and resolution status.
pub(crate) fn property(
    reader: &Reader<'_, '_>,
    parent: &StructValue<'_>,
    field: &'static str,
) -> Result<Option<PropertyReference>, ReadError> {
    parent
        .field(field)
        .map(|value| property_value(reader, parent, field, value))
        .transpose()
        .map(Option::flatten)
}

/// Returns the ordered weak links, retaining null entries and repetitions.
pub(crate) fn properties(
    reader: &Reader<'_, '_>,
    parent: &StructValue<'_>,
    field: &'static str,
) -> Result<Option<Vec<Option<PropertyReference>>>, ReadError> {
    array(parent, field, |value| {
        property_value(reader, parent, field, value)
    })
}

fn property_value(
    reader: &Reader<'_, '_>,
    parent: &StructValue<'_>,
    field: &'static str,
    value: &Value<'_>,
) -> Result<Option<PropertyReference>, ReadError> {
    scoped_location_weak::read(reader, value).map_err(|()| {
        unexpected(
            parent,
            field,
            value,
            "a nullable weak MissionProperty pointer",
        )
    })
}

fn array<T>(
    parent: &StructValue<'_>,
    field: &'static str,
    read: impl Fn(&Value<'_>) -> Result<T, ReadError>,
) -> Result<Option<Vec<T>>, ReadError> {
    match parent.field(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(values)) => values.iter().map(read).collect::<Result<_, _>>().map(Some),
        Some(value) => Err(unexpected(parent, field, value, "an array")),
    }
}
