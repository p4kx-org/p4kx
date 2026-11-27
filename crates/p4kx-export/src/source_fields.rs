// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value, ValueError};

use crate::definition_reference::DefinitionReference;

use crate::catalog_read::original::ExampleBodyReceipt;

#[path = "source_checked_fields.rs"]
mod checked;
#[path = "source_field_description.rs"]
mod description;
#[path = "source_field_reads.rs"]
mod reads;
#[path = "source_reference_shape.rs"]
mod reference_shape;
#[path = "source_field_violation.rs"]
mod violation;
pub(crate) use checked::{FieldCapture, FieldChecks, unknown_struct};
pub(crate) use description::{FieldDescription, FieldIssue};
pub(crate) use violation::FieldViolation;

pub(crate) use reference_shape::{ReferenceShape, ReferenceSlot};

/// A typed projection failure with its complete owning source field location.
#[derive(Debug, thiserror::Error)]
pub(crate) enum FieldError {
    #[error("{source_type}.{field} must hold {expected}")]
    Malformed {
        source_type: String,
        field: String,
        expected: &'static str,
    },
    #[error("{source_type}.{field} is unreadable: {source}")]
    Unreadable {
        source_type: String,
        field: String,
        source: ValueError,
    },
    #[error("{source_type}.{field} has an unsupported source declaration")]
    Unsupported { source_type: String, field: String },
    #[error("{source_type}.{field} has a concrete variant retained as authored fields")]
    RetainedVariant { source_type: String, field: String },
    #[error(transparent)]
    Captured(Box<CapturedFieldError>),
}

/// One allocation owning a field failure and its deferred original receipt.
#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub(crate) struct CapturedFieldError {
    #[source]
    error: FieldError,
    body: ExampleBodyReceipt,
}

impl CapturedFieldError {
    pub(crate) const fn error(&self) -> &FieldError {
        &self.error
    }
}

impl FieldError {
    pub(crate) fn with_body(self, body: ExampleBodyReceipt) -> Self {
        Self::Captured(Box::new(CapturedFieldError { error: self, body }))
    }

    pub(crate) fn body(&self) -> Option<&ExampleBodyReceipt> {
        match self {
            Self::Captured(captured) => Some(&captured.body),
            _ => None,
        }
    }
}
/// Strict reads of fields owned by one source struct, without graph following.
pub(crate) struct Fields<'scope, 'source, 'path> {
    source: &'scope StructValue<'source>,
    path: &'path str,
}

impl<'scope, 'source, 'path> Fields<'scope, 'source, 'path> {
    /// Creates field reads at a canonical source location.
    pub(crate) const fn new(source: &'scope StructValue<'source>, path: &'path str) -> Self {
        Self { source, path }
    }

    /// Records every additional declared field rather than silently omitting it.
    pub(crate) fn check(&self, known: &[&str], notes: &mut Vec<FieldError>) {
        for field in self.source.fields() {
            if !known.contains(&field.name()) {
                notes.push(self.unsupported(field.name()));
            }
        }
    }

    /// Retains an unsupported concrete source at its authored location.
    pub(crate) fn unsupported(&self, field: &str) -> FieldError {
        FieldError::Unsupported {
            source_type: self.source.type_name().to_owned(),
            field: self.location(field),
        }
    }

    /// Reads an exact source `Int32`, without conversion from another numeric kind.
    pub(crate) fn int32(&self, name: &str) -> Result<Option<i32>, FieldError> {
        reads::FieldReads::new(self.source, self.path)
            .int32(name)
            .map_err(FieldViolation::into_error)
    }

    /// Reads a finite number representable by the catalog's 32-bit float payload.
    pub(crate) fn number(&self, name: &str) -> Result<Option<f32>, FieldError> {
        reads::FieldReads::new(self.source, self.path)
            .number(name)
            .map_err(FieldViolation::into_error)
    }

    /// Reads an owned struct without turning an unreadable pointer into null.
    pub(crate) fn structure(
        &self,
        name: &str,
    ) -> Result<Option<&'scope StructValue<'source>>, FieldError> {
        reads::FieldReads::new(self.source, self.path)
            .structure(name)
            .map_err(FieldViolation::into_error)
    }

    /// Reads an array while retaining the difference between null and empty.
    pub(crate) fn array(&self, name: &str) -> Result<Option<&'scope [Value<'source>]>, FieldError> {
        reads::FieldReads::new(self.source, self.path)
            .array(name)
            .map_err(FieldViolation::into_error)
    }

    /// Borrows the checked Record Reference shape without owning target text.
    pub(crate) fn reference_view(
        &self,
        name: &str,
    ) -> Result<Option<ReferenceShape<'scope, 'source>>, FieldError> {
        reads::FieldReads::new(self.source, self.path)
            .reference(name)
            .map_err(FieldViolation::into_error)
    }

    /// Borrows the original value selected by the checked Record Reference read.
    pub(crate) fn reference_slot(
        &self,
        name: &str,
    ) -> Result<Option<ReferenceSlot<'scope, 'source>>, FieldError> {
        reads::FieldReads::new(self.source, self.path)
            .reference_slot(name)
            .map_err(FieldViolation::into_error)
    }

    /// Reads a Record Reference with its physical target and unresolved identity.
    pub(crate) fn reference(&self, name: &str) -> Result<Option<DefinitionReference>, FieldError> {
        Ok(self.reference_view(name)?.map(ReferenceShape::into_owned))
    }
    /// Describes an invalid stored value without retaining its arbitrary contents.
    pub(crate) fn invalid(
        &self,
        name: &str,
        value: &Value<'_>,
        expected: &'static str,
    ) -> FieldError {
        match value {
            Value::Error(source) => FieldError::Unreadable {
                source_type: self.source.type_name().to_owned(),
                field: self.location(name),
                source: *source,
            },
            _ => FieldError::Malformed {
                source_type: self.source.type_name().to_owned(),
                field: self.location(name),
                expected,
            },
        }
    }

    fn location(&self, name: &str) -> String {
        match (self.path.is_empty(), name.is_empty()) {
            (true, _) => name.to_owned(),
            (_, true) => self.path.to_owned(),
            _ => format!("{}.{}", self.path, name),
        }
    }
}

fn reference(value: &Value<'_>) -> Result<Option<DefinitionReference>, &'static str> {
    reference_shape::read(value).map(|target| target.map(ReferenceShape::into_owned))
}
/// Reads a nullable array entry as an owned struct at its indexed source position.
pub(crate) fn entry<'scope, 'source>(
    value: &'scope Value<'source>,
    owner: &str,
    path: &str,
) -> Result<Option<&'scope StructValue<'source>>, FieldError> {
    match value {
        Value::Null => Ok(None),
        Value::Struct(value) => Ok(Some(value)),
        Value::Error(source) => Err(FieldError::Unreadable {
            source_type: owner.to_owned(),
            field: path.to_owned(),
            source: *source,
        }),
        _ => Err(FieldError::Malformed {
            source_type: owner.to_owned(),
            field: path.to_owned(),
            expected: "a nullable owned struct",
        }),
    }
}
