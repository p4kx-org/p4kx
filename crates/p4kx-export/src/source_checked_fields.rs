// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Field, SourceScope, SourceUses, StructValue, Value};

use super::reads::FieldReads;
use super::reference_shape::{self, ReferenceShape};
use super::violation::FieldViolation;
use crate::CatalogError;
use crate::catalog_read::original::ExampleBodyReceipt;
use crate::definition_reference::DefinitionReference;
use crate::record_read::ReadError;

/// The existing original frame receiving actual borrowed fields and guard failures.
pub(crate) trait FieldCapture<'source> {
    fn check_parent(&self, parent: &StructValue<'source>) -> Result<(), CatalogError>;
    fn check_context(
        &self,
        sources: &SourceUses,
        dataforge: &SourceScope,
        record_index: u32,
    ) -> Result<(), CatalogError>;
    fn unsupported_type(
        &self,
        source: &StructValue<'source>,
        path: &str,
    ) -> Result<ExampleBodyReceipt, CatalogError>;
    fn unknown_type(
        &self,
        source: &StructValue<'source>,
        operation: &'static str,
    ) -> Result<ExampleBodyReceipt, CatalogError>;
    fn field_failure(
        &self,
        violation: &FieldViolation<'_, 'source>,
    ) -> Result<ExampleBodyReceipt, CatalogError>;
    fn missing_field(
        &self,
        source: &StructValue<'source>,
        name: &'static str,
    ) -> Result<ExampleBodyReceipt, CatalogError>;
    fn record_field(
        &self,
        parent: &StructValue<'source>,
        value: &Value<'source>,
        location: &str,
    ) -> Result<ExampleBodyReceipt, CatalogError>;
}

/// Shared guards with an explicit standalone or original-capture error boundary.
pub(crate) struct FieldChecks<'capture, 'scope, 'source> {
    source: &'scope StructValue<'source>,
    path: &'scope str,
    capture: Option<&'capture dyn FieldCapture<'source>>,
}

impl<'capture, 'scope, 'source> FieldChecks<'capture, 'scope, 'source> {
    pub(crate) fn for_context(
        source: &'scope StructValue<'source>,
        path: &'scope str,
        capture: Option<&'capture dyn FieldCapture<'source>>,
    ) -> Result<Self, ReadError> {
        if let Some(capture) = capture {
            capture.check_parent(source)?;
        }
        Ok(Self {
            source,
            path,
            capture,
        })
    }

    const fn reads(&self) -> FieldReads<'scope, 'source, 'scope> {
        FieldReads::new(self.source, self.path)
    }

    fn failure(&self, violation: FieldViolation<'_, 'source>) -> ReadError {
        match self.capture {
            None => Box::new(violation.into_error()),
            Some(capture) => violation.captured_error(capture),
        }
    }

    pub(crate) fn text(&self, name: &str) -> Result<Option<&'source str>, ReadError> {
        self.reads().text(name).map_err(|error| self.failure(error))
    }

    pub(crate) fn locale(&self, name: &str) -> Result<Option<&'source str>, ReadError> {
        self.reads()
            .locale(name)
            .map_err(|error| self.failure(error))
    }

    pub(crate) fn boolean(&self, name: &str) -> Result<Option<bool>, ReadError> {
        self.reads()
            .boolean(name)
            .map_err(|error| self.failure(error))
    }

    pub(crate) fn int32(&self, name: &str) -> Result<Option<i32>, ReadError> {
        self.reads()
            .int32(name)
            .map_err(|error| self.failure(error))
    }

    pub(crate) fn single(&self, name: &str) -> Result<Option<f32>, ReadError> {
        self.reads()
            .single(name)
            .map_err(|error| self.failure(error))
    }

    pub(crate) fn number(&self, name: &str) -> Result<Option<f32>, ReadError> {
        self.reads()
            .number(name)
            .map_err(|error| self.failure(error))
    }

    pub(crate) fn structure(
        &self,
        name: &str,
    ) -> Result<Option<&'scope StructValue<'source>>, ReadError> {
        self.reads()
            .structure(name)
            .map_err(|error| self.failure(error))
    }

    pub(crate) fn array(&self, name: &str) -> Result<Option<&'scope [Value<'source>]>, ReadError> {
        self.reads()
            .array(name)
            .map_err(|error| self.failure(error))
    }

    pub(crate) fn reference_view(
        &self,
        name: &str,
    ) -> Result<Option<ReferenceShape<'scope, 'source>>, ReadError> {
        self.reads()
            .reference(name)
            .map_err(|error| self.failure(error))
    }

    /// Collects authored Reference slots using the shared nullable classifier.
    pub(crate) fn references(
        &self,
        name: &str,
    ) -> Result<Option<Vec<Option<DefinitionReference>>>, ReadError> {
        self.array(name)?
            .map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        super::reference(value).map_err(|expected| {
                            self.invalid(&format!("{name}[{index}]"), value, expected)
                        })
                    })
                    .collect()
            })
            .transpose()
    }

    pub(crate) fn reference_shape(&self, name: &str) -> Result<(), ReadError> {
        self.reference_view(name)?;
        Ok(())
    }

    pub(crate) fn reference_shapes(&self, name: &str) -> Result<(), ReadError> {
        if let Some(values) = self.array(name)? {
            for (index, value) in values.iter().enumerate() {
                if let Err(expected) = reference_shape::read(value) {
                    return Err(self.invalid(&format!("{name}[{index}]"), value, expected));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn invalid(
        &self,
        name: &str,
        value: &Value<'source>,
        expected: &'static str,
    ) -> ReadError {
        self.failure(FieldViolation::invalid(
            self.source,
            self.path,
            name,
            value,
            expected,
        ))
    }

    pub(crate) fn unsupported(&self, declaration: &Field<'source>) -> ReadError {
        self.failure(FieldViolation::unsupported(
            self.source,
            self.path,
            declaration,
        ))
    }

    pub(crate) fn check(&self, known: &[&str]) -> Result<(), ReadError> {
        for declaration in self.source.fields() {
            if !known.contains(&declaration.name()) {
                return Err(self.unsupported(declaration));
            }
        }
        Ok(())
    }
}

/// Retains an unknown concrete type at the same guard that rejects its interpretation.
pub(crate) fn unknown_struct<'source>(
    source: &StructValue<'source>,
    operation: &'static str,
    capture: Option<&dyn FieldCapture<'source>>,
) -> ReadError {
    let body = match capture {
        None => None,
        Some(capture) => match capture.unknown_type(source, operation) {
            Ok(body) => Some(body),
            Err(error) => return Box::new(error),
        },
    };
    let error = crate::template_issues::TemplateError::unknown(operation, source);
    match body {
        None => Box::new(error),
        Some(body) => Box::new(error.with_body(body)),
    }
}
