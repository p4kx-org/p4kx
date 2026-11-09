// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{SourceValue, SourcedValue, StructValue, Value};

use super::CatalogRecordContext;
use crate::CatalogError;
use crate::source_fields::{FieldError, Fields};

/// Typed Catalog field reads retaining source state and authored locations.
pub(crate) struct CatalogFields<'s, 'f, 'c> {
    source: &'s StructValue<'f>,
    path: &'s str,
    context: &'s CatalogRecordContext<'c>,
}

impl<'s, 'f, 'c> CatalogFields<'s, 'f, 'c> {
    pub(crate) const fn new(
        source: &'s StructValue<'f>,
        path: &'s str,
        context: &'s CatalogRecordContext<'c>,
    ) -> Self {
        Self {
            source,
            path,
            context,
        }
    }

    pub(crate) fn check(&self, known: &[&str], notes: &mut Vec<FieldError>) {
        Fields::new(self.source, self.path).check(known, notes);
        let mut seen = HashSet::new();
        for field in self.source.fields() {
            if !seen.insert(field.name()) {
                notes.push(FieldError::Malformed {
                    source_type: self.source.type_name().to_owned(),
                    field: self.location(field.name()),
                    expected: "a unique declared name in the named canonical DTO",
                });
            }
        }
    }

    pub(crate) fn source_type(&self) -> Result<SourcedValue<&'f str>, CatalogError> {
        self.context.selector(
            &self.location("$type"),
            SourceValue::Present(self.source.type_name()),
        )
    }

    pub(crate) fn unsupported<T>(
        &self,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        notes.push(Fields::new(self.source, self.path).unsupported(""));
        self.context.value(self.path, SourceValue::Unsupported)
    }

    pub(crate) fn text(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'f str>, CatalogError> {
        self.read(name, "a nullable String", notes, |value| match value {
            Value::String(value) => Some(*value),
            _ => None,
        })
    }

    pub(crate) fn locale(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'f str>, CatalogError> {
        self.read(name, "a nullable Locale", notes, |value| match value {
            Value::Locale(value) => Some(*value),
            _ => None,
        })
    }

    pub(crate) fn boolean(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<bool>, CatalogError> {
        self.read(name, "a nullable Bool", notes, |value| match value {
            Value::Bool(value) => Some(*value),
            _ => None,
        })
    }

    pub(crate) fn structure(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'s StructValue<'f>>, CatalogError> {
        self.read(
            name,
            "a nullable owned struct",
            notes,
            |value| match value {
                Value::Struct(value) => Some(value),
                _ => None,
            },
        )
    }

    pub(crate) fn array(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'s [Value<'f>]>, CatalogError> {
        self.read(name, "a nullable array", notes, |value| match value {
            Value::Array(value) => Some(value.as_ref()),
            _ => None,
        })
    }

    /// Borrows the original owned source struct.
    pub(crate) const fn source(&self) -> &'s StructValue<'f> {
        self.source
    }

    /// Borrows the context of this original source field.
    pub(crate) const fn context(&self) -> &'s CatalogRecordContext<'c> {
        self.context
    }

    pub(crate) fn read<T>(
        &self,
        name: &str,
        expected: &'static str,
        notes: &mut Vec<FieldError>,
        project: impl FnOnce(&'s Value<'f>) -> Option<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.context.read(
            self.source.field(name),
            self.source.type_name(),
            &self.location(name),
            expected,
            notes,
            project,
        )
    }

    pub(crate) fn location(&self, name: &str) -> String {
        if self.path.is_empty() {
            name.to_owned()
        } else {
            format!("{}.{}", self.path, name)
        }
    }
}
