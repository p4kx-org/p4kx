// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Field, SourceScope, SourceUses, StructValue, Value};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{
    ExampleBodyCapture, ExampleBodyReceipt, FieldFailureKind, OriginalFlowFields,
};
use crate::record_observation::RecordObservation;
use crate::record_read::ReadError;
use crate::source_fields::{FieldCapture, FieldViolation};
use crate::{CatalogError, ExportAccountingError};

/// The same original frame receiving authored field positions and failed Template guards.
pub(crate) struct TemplateFieldCapture<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    input: &'scope OriginalCatalogInput<'scope, 'scope>,
    body: ExampleBodyCapture<'scope, 'read, 'source>,
}

impl<'fields, 'scope, 'read, 'source> TemplateFieldCapture<'fields, 'scope, 'read, 'source> {
    pub(crate) fn new(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        input: &'scope OriginalCatalogInput<'_, '_>,
        admission: &RecordObservation<'_, '_>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            fields,
            input,
            body: fields.body_capture(input, admission)?,
        })
    }

    pub(crate) const fn example_body(&self) -> &ExampleBodyCapture<'scope, 'read, 'source> {
        &self.body
    }
}

impl<'source> FieldCapture<'source> for TemplateFieldCapture<'_, '_, '_, 'source> {
    fn check_parent(&self, parent: &StructValue<'source>) -> Result<(), CatalogError> {
        self.fields.owner_of(parent)?;
        Ok(())
    }

    fn check_declared_target(
        &self,
        parent: &StructValue<'source>,
        path: &str,
        declaration: &Field<'source>,
        expected: &str,
    ) -> Result<(), ReadError> {
        let owner = self.fields.owner_of(parent)?;
        let (_, fact) = self
            .fields
            .field(owner, declaration.name())?
            .ok_or_else(|| CatalogError::from(ExportAccountingError::ConflictingSourceDatum))?;
        if !std::ptr::eq(fact.field(), declaration) {
            return Err(CatalogError::from(ExportAccountingError::ConflictingSourceDatum).into());
        }
        let (target, selectors) = fact.observe_declared_target(self.input)?;
        if target == Ok(expected) {
            return Ok(());
        }
        let violation =
            FieldViolation::rejected_declaration(parent, path, declaration, target.err());
        let ancestors = self.fields.ancestors(fact)?;
        let body = self.body.field_fact(
            parent,
            fact,
            &ancestors,
            &violation.field_text(),
            FieldFailureKind::NominalDeclaration(&selectors),
        )?;
        Err(Box::new(violation.into_error().with_body(body)))
    }
    fn check_context(
        &self,
        sources: &SourceUses,
        dataforge: &SourceScope,
        record_index: u32,
    ) -> Result<(), CatalogError> {
        self.fields.check_context(sources, dataforge, record_index)
    }

    fn unsupported_type(
        &self,
        source: &StructValue<'source>,
        path: &str,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        match self.fields.owner_of(source)? {
            Some(fact) => {
                if self.fields.logical_path(fact)? != path {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                let ancestors = self.fields.ancestors(fact)?;
                self.body.struct_fact(fact, &ancestors, path)
            }
            None if path.is_empty() => self.body.record_type(),
            None => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }

    fn field_failure(
        &self,
        violation: &FieldViolation<'_, 'source>,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        self.check_parent(violation.parent())?;
        let (value, kind) = match (violation.value(), violation.declaration()) {
            (Some(value), None) => (value, FieldFailureKind::Invalid),
            (None, Some(declaration)) => (declaration.value(), FieldFailureKind::Unsupported),
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        let fact = self.fields.fact(value)?;
        let ancestors = self.fields.ancestors(fact)?;
        self.body.field_fact(
            violation.parent(),
            fact,
            &ancestors,
            &violation.field_text(),
            kind,
        )
    }

    fn missing_field(
        &self,
        source: &StructValue<'source>,
        name: &'static str,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        let missing = self.fields.missing_field(source, name)?;
        self.body.missing_field(&missing)
    }

    fn record_field(
        &self,
        parent: &StructValue<'source>,
        value: &Value<'source>,
        location: &str,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        let fact = self.fields.fact(value)?;
        if self.fields.logical_path(fact)? != location {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let ancestors = self.fields.ancestors(fact)?;
        self.body
            .record_field_fact(parent, fact, &ancestors, location)
    }

    fn unknown_type(
        &self,
        source: &StructValue<'source>,
        operation: &'static str,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        let fact = self
            .fields
            .owner_of(source)?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let ancestors = self.fields.ancestors(fact)?;
        self.body.type_fact(fact, &ancestors, operation)
    }
}

#[cfg(test)]
#[path = "template_field_capture_tests.rs"]
mod tests;
