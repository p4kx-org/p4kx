// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value, ValueError};

use crate::catalog_read::original::ExampleBodyReceipt;
use crate::record_read::ReadError;
use crate::source_fields::{FieldCapture, FieldDescription, FieldIssue, FieldViolation};

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
    #[error("{error}")]
    Captured {
        #[source]
        error: Box<Self>,
        body: ExampleBodyReceipt,
    },
}

impl PropertyFieldError {
    pub(crate) fn description(&self) -> FieldDescription<'_> {
        let mut current = self;
        loop {
            match current {
                Self::Captured { error, .. } => current = error,
                Self::Malformed { field, .. } => {
                    return FieldDescription {
                        issue: FieldIssue::Malformed,
                        source_type: None,
                        field,
                    };
                }
                Self::Unreadable { field, .. } => {
                    return FieldDescription {
                        issue: FieldIssue::Unreadable,
                        source_type: None,
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

pub(crate) fn malformed<'source>(
    parent: &StructValue<'source>,
    field: &'static str,
    value: &Value<'source>,
    expected: &'static str,
    capture: &dyn FieldCapture<'source>,
) -> ReadError {
    let body =
        match capture.field_failure(&FieldViolation::invalid(parent, "", field, value, expected)) {
            Ok(body) => body,
            Err(error) => return Box::new(error),
        };
    let error = match value {
        Value::Error(source) => PropertyFieldError::Unreadable {
            field,
            source: *source,
        },
        _ => PropertyFieldError::Malformed { field, expected },
    };
    Box::new(PropertyFieldError::Captured {
        error: Box::new(error),
        body,
    })
}

pub(crate) fn structure<'tree, 'source>(
    parent: &StructValue<'source>,
    value: &'tree Value<'source>,
    field: &'static str,
    capture: &dyn FieldCapture<'source>,
) -> Result<Option<&'tree StructValue<'source>>, ReadError> {
    capture.check_parent(parent)?;
    match value {
        Value::Null => Ok(None),
        Value::Struct(value) => Ok(Some(value)),
        _ => Err(malformed(
            parent,
            field,
            value,
            "a nullable source struct",
            capture,
        )),
    }
}

pub(crate) fn parameter<'tree, 'source>(
    parent: &'tree StructValue<'source>,
    field: &'static str,
    capture: &dyn FieldCapture<'source>,
) -> Result<Option<&'tree StructValue<'source>>, ReadError> {
    capture.check_parent(parent)?;
    parent
        .field(field)
        .map(|value| structure(parent, value, field, capture))
        .transpose()
        .map(Option::flatten)
}

pub(crate) fn text<'source>(
    parent: &StructValue<'source>,
    field: &'static str,
    capture: &dyn FieldCapture<'source>,
) -> Result<Option<String>, ReadError> {
    capture.check_parent(parent)?;
    match parent.field(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some((*value).to_owned())),
        Some(value) => Err(malformed(
            parent,
            field,
            value,
            "a nullable source string",
            capture,
        )),
    }
}
