// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{SourceEvidenceError, SourceValue, SourcedValue, StructValue, Value};

use super::CatalogRecordContext;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::source_fields::{FieldCapture, FieldError, FieldViolation, Fields};
use crate::{CatalogError, ExportAccountingError};

/// Typed Catalog field reads retaining source state and authored locations.
pub(crate) struct CatalogFields<'s, 'f, 'c> {
    source: &'s StructValue<'f>,
    path: &'s str,
    context: &'s CatalogRecordContext<'c>,
    capture: Option<&'s dyn FieldCapture<'f>>,
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
            capture: None,
        }
    }

    /// Binds the error receiver before reading this actual original Struct.
    pub(crate) fn observed(
        source: &'s StructValue<'f>,
        path: &'s str,
        context: &'s CatalogRecordContext<'c>,
        capture: &'s dyn FieldCapture<'f>,
    ) -> Result<Self, CatalogError> {
        capture.check_parent(source)?;
        let record = context
            .record_position()
            .ok_or(SourceEvidenceError::ForeignEvidence)?;
        capture.check_context(context.sources(), context.dataforge(), record)?;
        Ok(Self {
            source,
            path,
            context,
            capture: Some(capture),
        })
    }

    fn violations<'scan>(
        &'scan self,
        known: &'scan [&str],
    ) -> impl Iterator<Item = FieldViolation<'s, 'f>> + 'scan {
        let source = self.source;
        let path = self.path;
        let unknown = source
            .fields()
            .iter()
            .filter(move |field| !known.contains(&field.name()))
            .map(move |field| FieldViolation::unsupported(source, path, field));
        let mut seen = HashSet::new();
        let duplicate = source
            .fields()
            .iter()
            .filter(move |field| !seen.insert(field.name()))
            .map(move |field| {
                FieldViolation::invalid(
                    source,
                    path,
                    field.name(),
                    field.value(),
                    "a unique declared name in the named canonical DTO",
                )
            });
        unknown.chain(duplicate)
    }

    pub(crate) fn check_observed(
        &self,
        known: &[&str],
        notes: &mut Vec<FieldError>,
    ) -> Result<(), CatalogError> {
        for violation in self.violations(known) {
            notes.push(self.failure(violation)?);
        }
        Ok(())
    }

    fn failure(&self, violation: FieldViolation<'_, 'f>) -> Result<FieldError, CatalogError> {
        let body = self
            .capture
            .map(|capture| capture.field_failure(&violation))
            .transpose()?;
        let error = violation.into_error();
        Ok(match body {
            Some(body) => error.with_body(body),
            None => error,
        })
    }

    pub(crate) fn invalid(
        &self,
        name: &str,
        value: &Value<'f>,
        expected: &'static str,
    ) -> Result<FieldError, CatalogError> {
        self.failure(FieldViolation::invalid(
            self.source,
            self.path,
            name,
            value,
            expected,
        ))
    }

    pub(crate) fn read_value<'value, T>(
        &self,
        value: Option<&'value Value<'f>>,
        name: &str,
        expected: &'static str,
        notes: &mut Vec<FieldError>,
        project: impl FnOnce(&'value Value<'f>) -> Option<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.context
            .read_with(value, &self.location(name), notes, project, |value| {
                self.invalid(name, value, expected)
            })
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
        self.note_unsupported(notes)?;
        self.context.value(self.path, SourceValue::Unsupported)
    }

    /// Retains the rejected concrete source without inventing a payload cell.
    pub(crate) fn note_unsupported(&self, notes: &mut Vec<FieldError>) -> Result<(), CatalogError> {
        let body = self
            .capture
            .map(|capture| capture.unsupported_type(self.source, self.path))
            .transpose()?;
        let error = Fields::new(self.source, self.path).unsupported("");
        notes.push(match body {
            Some(body) => error.with_body(body),
            None => error,
        });
        Ok(())
    }

    /// Diagnoses a fixed required name from the complete original declaration survey.
    pub(crate) fn missing(
        &self,
        name: &'static str,
        expected: &'static str,
    ) -> Result<FieldError, CatalogError> {
        if self.source.field(name).is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let body = self
            .capture
            .map(|capture| capture.missing_field(self.source, name))
            .transpose()?;
        let error = FieldError::Malformed {
            source_type: self.source.type_name().to_owned(),
            field: self.location(name),
            expected,
        };
        Ok(match body {
            Some(body) => error.with_body(body),
            None => error,
        })
    }

    /// Reads a Record Reference with the same original receiver as scalar guards.
    pub(crate) fn reference(
        &self,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<CatalogRecordReference>, CatalogError> {
        self.reference_value(name, self.source.field(name), notes)
    }

    /// Reads one original Record Reference under the same parent field receiver.
    pub(crate) fn reference_value(
        &self,
        name: &str,
        value: Option<&'s Value<'f>>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<CatalogRecordReference>, CatalogError> {
        super::catalog_reference_read::reference_with_failure(
            self.context,
            value,
            &self.location(name),
            notes,
            |value| self.invalid(name, value, "a nullable Record Reference"),
        )
    }

    /// Reads the actual authored Array slot under this Struct's field receiver.
    pub(crate) fn structure_element<'value>(
        &self,
        name: &str,
        value: &'value Value<'f>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<&'value StructValue<'f>>, CatalogError> {
        self.read_value(
            Some(value),
            name,
            "a nullable owned struct",
            notes,
            |value| match value {
                Value::Struct(value) => Some(value),
                _ => None,
            },
        )
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
        self.read_value(self.source.field(name), name, expected, notes, project)
    }

    pub(crate) fn location(&self, name: &str) -> String {
        if self.path.is_empty() {
            name.to_owned()
        } else {
            format!("{}.{}", self.path, name)
        }
    }
}
