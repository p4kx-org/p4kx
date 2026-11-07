// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value, ValueError};

use crate::definition_reference::{DefinitionReference, DefinitionStatus};
use crate::guid_text::CatalogGuid;
use crate::record_fields;

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
}

/// Strict reads of fields owned by one source struct, without graph following.
pub(crate) struct Fields<'scope, 'source> {
    source: &'scope StructValue<'source>,
    path: &'scope str,
}

impl<'scope, 'source> Fields<'scope, 'source> {
    /// Creates field reads at a canonical source location.
    pub(crate) const fn new(source: &'scope StructValue<'source>, path: &'scope str) -> Self {
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

    /// Reads a raw source string, retaining an empty value.
    pub(crate) fn text(&self, name: &str) -> Result<Option<&'source str>, FieldError> {
        self.optional(name, "a nullable String", |value| match value {
            Value::String(value) => Some(*value),
            _ => None,
        })
    }

    /// Reads a localisation key, retaining an empty value.
    pub(crate) fn locale(&self, name: &str) -> Result<Option<&'source str>, FieldError> {
        self.optional(name, "a nullable Locale", |value| match value {
            Value::Locale(value) => Some(*value),
            _ => None,
        })
    }

    /// Reads a nullable source boolean without replacing false.
    pub(crate) fn boolean(&self, name: &str) -> Result<Option<bool>, FieldError> {
        self.optional(name, "a nullable Bool", |value| match value {
            Value::Bool(value) => Some(*value),
            _ => None,
        })
    }

    /// Reads an exact source `Int32`, without conversion from another numeric kind.
    pub(crate) fn int32(&self, name: &str) -> Result<Option<i32>, FieldError> {
        self.optional(name, "a nullable Int32", |value| match value {
            Value::Int32(value) => Some(*value),
            _ => None,
        })
    }

    /// Reads a finite source `Single`, without clamping or changing its unit.
    pub(crate) fn single(&self, name: &str) -> Result<Option<f32>, FieldError> {
        self.optional(name, "a nullable finite Single", |value| match value {
            Value::Single(value) if value.is_finite() => Some(*value),
            _ => None,
        })
    }

    /// Reads a finite number representable by the catalog's 32-bit float payload.
    pub(crate) fn number(&self, name: &str) -> Result<Option<f32>, FieldError> {
        self.optional(
            name,
            "a finite number representable as a 32-bit float",
            |_| record_fields::number(Some(self.source), name),
        )
    }

    /// Reads an owned struct without turning an unreadable pointer into null.
    pub(crate) fn structure(
        &self,
        name: &str,
    ) -> Result<Option<&'scope StructValue<'source>>, FieldError> {
        self.optional(name, "a nullable owned struct", |value| match value {
            Value::Struct(value) => Some(value),
            _ => None,
        })
    }

    /// Reads an array while retaining the difference between null and empty.
    pub(crate) fn array(&self, name: &str) -> Result<Option<&'scope [Value<'source>]>, FieldError> {
        self.optional(name, "a nullable array", |value| match value {
            Value::Array(value) => Some(value.as_ref()),
            _ => None,
        })
    }

    /// Reads a Record Reference with its physical target and unresolved identity.
    pub(crate) fn reference(&self, name: &str) -> Result<Option<DefinitionReference>, FieldError> {
        self.source
            .field(name)
            .map(|value| reference(value).map_err(|expected| self.invalid(name, value, expected)))
            .transpose()
            .map(Option::flatten)
    }

    /// Reads an authored Reference list, retaining null entries and repetitions.
    pub(crate) fn references(
        &self,
        name: &str,
    ) -> Result<Option<Vec<Option<DefinitionReference>>>, FieldError> {
        self.array(name)?
            .map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        reference(value).map_err(|expected| {
                            self.invalid(&format!("{name}[{index}]"), value, expected)
                        })
                    })
                    .collect()
            })
            .transpose()
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

    fn optional<T>(
        &self,
        name: &str,
        expected: &'static str,
        read: impl FnOnce(&'scope Value<'source>) -> Option<T>,
    ) -> Result<Option<T>, FieldError> {
        match self.source.field(name) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => read(value)
                .map(Some)
                .ok_or_else(|| self.invalid(name, value, expected)),
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
    let (guid, status) = match value {
        Value::Null => return Ok(None),
        Value::Reference(target) if target.location().is_none() => {
            return Ok(Some(DefinitionReference {
                target_record_index: Some(target.record_index()),
                guid: CatalogGuid(target.guid()),
                record_path: Some(target.record_path().to_owned()),
                target_type: Some(target.type_name().to_owned()),
                status: DefinitionStatus::Present,
            }));
        }
        Value::Error(ValueError::DanglingReference { guid }) => (*guid, DefinitionStatus::Dangling),
        Value::Error(ValueError::UnexpectedReferenceType { guid, .. }) => {
            (*guid, DefinitionStatus::UnexpectedTargetType)
        }
        _ => return Err("a nullable Record Reference"),
    };
    Ok(Some(DefinitionReference {
        target_record_index: None,
        guid: CatalogGuid(guid),
        record_path: None,
        target_type: None,
        status,
    }))
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
