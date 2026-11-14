// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{
    Guid, InputScope, SourceUse, SourceUseRole, SourceValue, StructValue, Value,
};

use super::health_example_body::ExampleBodyReceipt;
use super::health_reference_receipt::{ReferenceDatum, ReferenceFrame, ReferenceUse};
use super::{OriginalRecordScope, OriginalValueEdge, OriginalValueFact, OriginalValuePart};
use crate::build::original_input::OriginalCatalogInput;
use crate::health_diagnostic_observation::ExampleObservation;
use crate::record_header_field::RecordHeaderField;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, HealthText};

/// The actual guard operation producing a field diagnostic.
#[derive(Debug, Clone, Copy)]
pub(crate) enum FieldFailureKind<'selection> {
    Invalid,
    Unsupported,
    NominalDeclaration(&'selection [SourceUse; 5]),
}

/// A same-read source frame borrowed by observed guards before errors become owned text.
pub(crate) struct ExampleBodyCapture<'scope, 'read, 'source> {
    original_scope: &'scope OriginalRecordScope<'read, 'source>,
    frame: Arc<ReferenceFrame>,
    texts: &'scope super::HealthTextCapture,
}

impl<'scope, 'read, 'source> ExampleBodyCapture<'scope, 'read, 'source> {
    pub(crate) fn new(
        input: &'scope OriginalCatalogInput<'_, '_>,
        admission: &RecordObservation<'_, '_>,
        scope: &'scope OriginalRecordScope<'read, 'source>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            original_scope: scope,
            frame: Arc::new(ReferenceFrame::read(input, admission, scope)?),
            texts: input.origins().health_texts(),
        })
    }

    fn check_fact(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<(), ExportAccountingError> {
        if !std::ptr::eq(fact.scope(), self.original_scope) {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        self.frame.check_scope(fact.scope())
    }

    pub(crate) fn vocabulary_example(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        ancestors: &[&OriginalValueFact<'scope, 'read, 'source>],
        path: &str,
        part: OriginalValuePart,
    ) -> Result<ExampleObservation, CatalogError> {
        self.check_fact(fact)?;
        super::health_value_chain::check(fact, ancestors)?;
        let (source_type, location) = match (part, fact.value()) {
            (OriginalValuePart::Value, Value::String(value) | Value::Enum(value))
                if !value.is_empty() =>
            {
                (
                    self.record_type_datum()?,
                    fact.location(OriginalValuePart::Value),
                )
            }
            (OriginalValuePart::ConcreteType, Value::Struct(value)) => (
                ReferenceDatum::captured(
                    SourceValue::Present(HealthText::new(value.type_name())),
                    vec![ReferenceUse::captured(
                        fact.location(OriginalValuePart::ConcreteType),
                        SourceUseRole::Selector,
                    )],
                ),
                fact.location(OriginalValuePart::ConcreteType),
            ),
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        let mut inputs = path_inputs(fact, ancestors);
        inputs.push(ReferenceUse::captured(location, SourceUseRole::Selector));
        let body = ExampleBodyReceipt::captured(
            Arc::clone(&self.frame),
            ReferenceDatum::captured(SourceValue::Present(HealthText::new(path)), inputs),
            source_type,
            ReferenceDatum::absent(),
        );
        let (field, source_type, _) = body.description()?;
        Ok(ExampleObservation::captured(
            self.frame.subject().clone(),
            field.as_deref(),
            source_type.as_deref(),
            None,
            body,
        )?)
    }

    pub(crate) fn type_fact<'fact>(
        &self,
        fact: &'fact OriginalValueFact<'scope, 'read, 'source>,
        ancestors: &'fact [&'fact OriginalValueFact<'scope, 'read, 'source>],
        field_text: &'static str,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        self.check_fact(fact)?;
        super::health_value_chain::check(fact, ancestors)?;
        let Value::Struct(value) = fact.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        Ok(ExampleBodyReceipt::captured(
            Arc::clone(&self.frame),
            ReferenceDatum::captured(
                SourceValue::Present(self.texts.retain_text(field_text)?.text().clone()),
                Vec::new(),
            ),
            ReferenceDatum::captured(
                SourceValue::Present(self.texts.retain_text(value.type_name())?.text().clone()),
                vec![ReferenceUse::captured(
                    fact.location(OriginalValuePart::ConcreteType),
                    SourceUseRole::Selector,
                )],
            ),
            ReferenceDatum::absent(),
        ))
    }

    pub(crate) fn struct_fact<'fact>(
        &self,
        fact: &'fact OriginalValueFact<'scope, 'read, 'source>,
        ancestors: &'fact [&'fact OriginalValueFact<'scope, 'read, 'source>],
        field_text: &str,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        self.check_fact(fact)?;
        super::health_value_chain::check(fact, ancestors)?;
        let Value::Struct(value) = fact.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        Ok(ExampleBodyReceipt::captured(
            Arc::clone(&self.frame),
            self.path_datum(fact, ancestors, field_text)?,
            ReferenceDatum::captured(
                SourceValue::Present(self.texts.retain_text(value.type_name())?.text().clone()),
                vec![ReferenceUse::captured(
                    fact.location(OriginalValuePart::ConcreteType),
                    SourceUseRole::Selector,
                )],
            ),
            ReferenceDatum::absent(),
        ))
    }

    pub(crate) fn record_type(&self) -> Result<ExampleBodyReceipt, CatalogError> {
        self.frame.check_scope(self.original_scope)?;
        Ok(ExampleBodyReceipt::captured(
            Arc::clone(&self.frame),
            ReferenceDatum::captured(
                SourceValue::Present(self.texts.retain_text("")?.text().clone()),
                Vec::new(),
            ),
            self.record_type_datum()?,
            ReferenceDatum::absent(),
        ))
    }

    pub(crate) fn record_field_fact<'fact>(
        &self,
        parent: &StructValue<'source>,
        fact: &'fact OriginalValueFact<'scope, 'read, 'source>,
        ancestors: &'fact [&'fact OriginalValueFact<'scope, 'read, 'source>],
        field_text: &str,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        self.check_fact(fact)?;
        super::health_value_chain::parent(parent, fact, ancestors)?;
        if !matches!(fact.edge(), OriginalValueEdge::Field { .. }) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(ExampleBodyReceipt::captured(
            Arc::clone(&self.frame),
            self.path_datum(fact, ancestors, field_text)?,
            self.record_type_datum()?,
            ReferenceDatum::absent(),
        ))
    }

    pub(crate) fn field_fact<'fact>(
        &self,
        parent: &StructValue<'source>,
        fact: &'fact OriginalValueFact<'scope, 'read, 'source>,
        ancestors: &'fact [&'fact OriginalValueFact<'scope, 'read, 'source>],
        field_text: &str,
        kind: FieldFailureKind<'_>,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        self.field_body(parent, fact, ancestors, field_text, kind, || {
            Ok(ReferenceDatum::absent())
        })
    }

    pub(crate) fn reference_fact<'fact>(
        &self,
        parent: &StructValue<'source>,
        fact: &'fact OriginalValueFact<'scope, 'read, 'source>,
        ancestors: &'fact [&'fact OriginalValueFact<'scope, 'read, 'source>],
        field_text: &str,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        self.field_body(
            parent,
            fact,
            ancestors,
            field_text,
            FieldFailureKind::Invalid,
            || super::health_reference_target::guid_for_fact(fact),
        )
    }

    fn field_body<'fact>(
        &self,
        parent: &StructValue<'source>,
        fact: &'fact OriginalValueFact<'scope, 'read, 'source>,
        ancestors: &'fact [&'fact OriginalValueFact<'scope, 'read, 'source>],
        field_text: &str,
        kind: FieldFailureKind<'_>,
        target: impl FnOnce() -> Result<ReferenceDatum<Guid>, CatalogError>,
    ) -> Result<ExampleBodyReceipt, CatalogError> {
        self.check_fact(fact)?;
        let parent_site = super::health_value_chain::parent(parent, fact, ancestors)?;
        if !matches!(kind, FieldFailureKind::Invalid)
            && !matches!(fact.edge(), OriginalValueEdge::Field { .. })
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let owner = parent_site.map_or_else(
            || RecordHeaderField::SourceType.location(fact.scope().record().index()),
            |parent| parent.location(OriginalValuePart::ConcreteType),
        );
        let target = target()?;
        let mut field = self.path_datum(fact, ancestors, field_text)?;
        if let FieldFailureKind::NominalDeclaration(selectors) = kind {
            self.retain_declaration_selectors(selectors, &mut field)?;
        }
        Ok(ExampleBodyReceipt::captured(
            Arc::clone(&self.frame),
            field,
            ReferenceDatum::captured(
                SourceValue::Present(self.texts.retain_text(parent.type_name())?.text().clone()),
                vec![ReferenceUse::captured(owner, SourceUseRole::Selector)],
            ),
            target,
        ))
    }
    fn retain_declaration_selectors(
        &self,
        selectors: &[SourceUse; 5],
        field: &mut ReferenceDatum<HealthText>,
    ) -> Result<(), ExportAccountingError> {
        if !self
            .original_scope
            .sources()
            .origin(selectors, true)
            .source_set_complete()
        {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        for selector in selectors {
            let original = selector.description();
            if original.source() != self.original_scope.dataforge().source_ref()
                || original.scope() != InputScope::Payload
            {
                return Err(ExportAccountingError::ConflictingSourceDatum);
            }
            match original.role() {
                SourceUseRole::Selector => field.contribute(ReferenceUse::captured(
                    original.location().clone(),
                    original.role(),
                )),
                SourceUseRole::Attempted => {}
                _ => return Err(ExportAccountingError::ConflictingSourceDatum),
            }
        }
        Ok(())
    }

    fn path_datum(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        ancestors: &[&OriginalValueFact<'scope, 'read, 'source>],
        field_text: &str,
    ) -> Result<ReferenceDatum<HealthText>, CatalogError> {
        Ok(ReferenceDatum::captured(
            SourceValue::Present(self.texts.retain_text(field_text)?.text().clone()),
            path_inputs(fact, ancestors),
        ))
    }

    fn record_type_datum(&self) -> Result<ReferenceDatum<HealthText>, CatalogError> {
        let value = self.original_scope.tree().root().type_name();
        Ok(ReferenceDatum::captured(
            SourceValue::Present(self.texts.retain_text(value)?.text().clone()),
            vec![ReferenceUse::captured(
                RecordHeaderField::SourceType.location(self.original_scope.record().index()),
                SourceUseRole::Selector,
            )],
        ))
    }
}

fn path_inputs(
    fact: &OriginalValueFact<'_, '_, '_>,
    ancestors: &[&OriginalValueFact<'_, '_, '_>],
) -> Vec<ReferenceUse> {
    let mut inputs = Vec::new();
    for ancestor in ancestors {
        add_path(&mut inputs, ancestor);
    }
    add_path(&mut inputs, fact);
    inputs
}

fn add_path(inputs: &mut Vec<ReferenceUse>, fact: &OriginalValueFact<'_, '_, '_>) {
    if matches!(fact.edge(), OriginalValueEdge::Field { .. }) {
        inputs.push(ReferenceUse::captured(
            fact.location(OriginalValuePart::FieldName),
            SourceUseRole::Conversion,
        ));
    }
    if matches!(fact.value(), Value::Array(_)) {
        inputs.push(ReferenceUse::captured(
            fact.location(OriginalValuePart::Value),
            SourceUseRole::Selector,
        ));
    }
}

#[cfg(test)]
#[path = "health_example_record_field_tests.rs"]
mod record_field_tests;

#[cfg(test)]
#[path = "health_example_capture_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "health_example_struct_tests.rs"]
mod struct_tests;

#[path = "health_example_missing_field.rs"]
mod missing_field;
