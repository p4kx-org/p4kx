// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value, ValueError};

use crate::catalog_read::original::ExampleBodyReceipt;
use crate::record_read::ReadError;
use crate::scoped_location_link::{DefinitionReference, PropertyReference};
use crate::scoped_location_weak;
use crate::source_fields::{FieldCapture, FieldDescription, FieldIssue, FieldViolation};
use crate::spawn_read::Reader;

#[path = "scoped_location_shape_fields.rs"]
mod shape_fields;
pub(crate) use shape_fields::ShapeFields;

#[path = "scoped_location_definition.rs"]
mod definition_shape;

#[derive(Debug, thiserror::Error)]
pub(crate) enum FieldError {
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
    #[error("{error}")]
    Captured {
        #[source]
        error: Box<Self>,
        body: ExampleBodyReceipt,
    },
}

impl FieldError {
    pub(crate) fn description(&self) -> FieldDescription<'_> {
        let mut current = self;
        loop {
            match current {
                Self::Captured { error, .. } => current = error,
                Self::Malformed { owner, field, .. } => {
                    return FieldDescription {
                        issue: FieldIssue::Malformed,
                        source_type: Some(owner),
                        field,
                    };
                }
                Self::Unreadable { owner, field, .. } => {
                    return FieldDescription {
                        issue: FieldIssue::Unreadable,
                        source_type: Some(owner),
                        field,
                    };
                }
            }
        }
    }
    pub(crate) const fn body(&self) -> Option<&ExampleBodyReceipt> {
        match self {
            Self::Captured { body, .. } => Some(body),
            _ => None,
        }
    }
}

fn unexpected_context<'source>(
    owner: &StructValue<'source>,
    field: &'static str,
    value: &Value<'source>,
    expected: &'static str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> ReadError {
    let body = match capture {
        None => None,
        Some(capture) => match capture
            .field_failure(&FieldViolation::invalid(owner, "", field, value, expected))
        {
            Ok(body) => Some(body),
            Err(error) => return Box::new(error),
        },
    };
    let error = match value {
        Value::Error(source) => FieldError::Unreadable {
            owner: owner.type_name().to_owned(),
            field,
            source: *source,
        },
        _ => FieldError::Malformed {
            owner: owner.type_name().to_owned(),
            field,
            expected,
        },
    };
    match body {
        None => Box::new(error),
        Some(body) => Box::new(FieldError::Captured {
            error: Box::new(error),
            body,
        }),
    }
}
/// Returns an optional source struct without dropping a present invalid value.
pub(crate) fn structure<'tree, 'source>(
    parent: &'tree StructValue<'source>,
    field: &'static str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<&'tree StructValue<'source>>, ReadError> {
    if let Some(capture) = capture {
        capture.check_parent(parent)?;
    }
    match parent.field(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Struct(value)) => Ok(Some(value)),
        Some(value) => Err(unexpected_context(
            parent,
            field,
            value,
            "a nullable StrongPointer",
            capture,
        )),
    }
}

/// Returns a raw enum option, preserving unknown names and an empty stored option.
pub(crate) fn enum_name<'source>(
    parent: &StructValue<'source>,
    field: &'static str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<String>, ReadError> {
    Ok(enum_context(parent, field, capture)?.map(str::to_owned))
}

fn enum_context<'source>(
    parent: &StructValue<'source>,
    field: &'static str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<&'source str>, ReadError> {
    if let Some(capture) = capture {
        capture.check_parent(parent)?;
    }
    match parent.field(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Enum(name)) => Ok(Some(*name)),
        Some(value) => Err(unexpected_context(
            parent,
            field,
            value,
            "an enum option",
            capture,
        )),
    }
}
/// Returns a source definition Reference, retaining a missing target's GUID.
pub(crate) fn definition<'source>(
    parent: &StructValue<'source>,
    field: &'static str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<DefinitionReference>, ReadError> {
    if let Some(capture) = capture {
        capture.check_parent(parent)?;
    }
    parent
        .field(field)
        .map(|value| definition_value(parent, field, value, capture))
        .transpose()
        .map(Option::flatten)
}

/// Returns the ordered source definition References, including null entries and repetitions.
pub(crate) fn definitions<'source>(
    parent: &StructValue<'source>,
    field: &'static str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<Vec<Option<DefinitionReference>>>, ReadError> {
    array(parent, field, capture, |value| {
        definition_value(parent, field, value, capture)
    })
}

fn definition_value<'source>(
    parent: &StructValue<'source>,
    field: &'static str,
    value: &Value<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<DefinitionReference>, ReadError> {
    Ok(
        definition_shape::read_context(parent, field, value, capture)?
            .map(definition_shape::DefinitionShape::into_owned),
    )
}

/// Returns a weak property link with its available source identity and resolution status.
pub(crate) fn property<'source>(
    reader: &Reader<'_, 'source>,
    parent: &StructValue<'source>,
    field: &'static str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<PropertyReference>, ReadError> {
    if let Some(capture) = capture {
        capture.check_parent(parent)?;
    }
    parent
        .field(field)
        .map(|value| property_value(reader, parent, field, value, capture))
        .transpose()
        .map(Option::flatten)
}

/// Returns the ordered weak links, retaining null entries and repetitions.
pub(crate) fn properties<'source>(
    reader: &Reader<'_, 'source>,
    parent: &StructValue<'source>,
    field: &'static str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<Vec<Option<PropertyReference>>>, ReadError> {
    array(parent, field, capture, |value| {
        property_value(reader, parent, field, value, capture)
    })
}

fn property_value<'source>(
    reader: &Reader<'_, 'source>,
    parent: &StructValue<'source>,
    field: &'static str,
    value: &Value<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<PropertyReference>, ReadError> {
    if let Some(capture) = capture {
        capture.check_parent(parent)?;
    }
    scoped_location_weak::read(reader, value).map_err(|()| {
        unexpected_context(
            parent,
            field,
            value,
            "a nullable weak MissionProperty pointer",
            capture,
        )
    })
}

fn array_context<'tree, 'source>(
    parent: &'tree StructValue<'source>,
    field: &'static str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<Option<&'tree [Value<'source>]>, ReadError> {
    if let Some(capture) = capture {
        capture.check_parent(parent)?;
    }
    match parent.field(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(values)) => Ok(Some(values)),
        Some(value) => Err(unexpected_context(
            parent, field, value, "an array", capture,
        )),
    }
}

fn array<'source, T>(
    parent: &StructValue<'source>,
    field: &'static str,
    capture: Option<&dyn FieldCapture<'source>>,
    read: impl Fn(&Value<'source>) -> Result<T, ReadError>,
) -> Result<Option<Vec<T>>, ReadError> {
    array_context(parent, field, capture)?
        .map(|values| values.iter().map(read).collect())
        .transpose()
}
