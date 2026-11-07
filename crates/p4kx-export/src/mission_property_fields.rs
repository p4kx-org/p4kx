// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value, ValueError};

use crate::record_read::ReadError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum PropertyFieldError {
    #[error("mission property {field} must hold {expected}")]
    Malformed {
        field: &'static str,
        expected: &'static str,
    },
    #[error("mission property {field} is unreadable: {source}")]
    Unreadable {
        field: &'static str,
        source: ValueError,
    },
}

pub(crate) fn malformed(
    field: &'static str,
    value: &Value<'_>,
    expected: &'static str,
) -> ReadError {
    match value {
        Value::Error(source) => Box::new(PropertyFieldError::Unreadable {
            field,
            source: *source,
        }),
        _ => Box::new(PropertyFieldError::Malformed { field, expected }),
    }
}

pub(crate) fn structure<'t, 'f>(
    value: &'t Value<'f>,
    field: &'static str,
) -> Result<Option<&'t StructValue<'f>>, ReadError> {
    match value {
        Value::Null => Ok(None),
        Value::Struct(value) => Ok(Some(value)),
        _ => Err(malformed(field, value, "a nullable source struct")),
    }
}

pub(crate) fn parameter<'t, 'f>(
    parent: &'t StructValue<'f>,
    field: &'static str,
) -> Result<Option<&'t StructValue<'f>>, ReadError> {
    parent
        .field(field)
        .map(|value| structure(value, field))
        .transpose()
        .map(Option::flatten)
}

pub(crate) fn text(
    parent: &StructValue<'_>,
    field: &'static str,
) -> Result<Option<String>, ReadError> {
    match parent.field(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some((*value).to_owned())),
        Some(value) => Err(malformed(field, value, "a nullable source string")),
    }
}
