// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    Guid, RecordTree, SourceUse, SourceUseRole, SourceValue, StructValue, Value, ValueError,
};

use crate::catalog_read::original::{
    OriginalDatum, OriginalErrorParameter, OriginalFlowFields, OriginalTargetShape,
    OriginalValueFact, OriginalValuePart,
};
use crate::catalog_v3::tag_catalog::TagPathLookup;
use crate::catalog_v3::tag_catalog::dependent_origins::TagInputs;
use crate::guid_text::CatalogGuid;
use crate::record_read::ReadError;
use crate::source_fields::{FieldCapture, FieldViolation};
use crate::spawn_read::ReadContext;
use crate::spawn_tag_text::SpawnTagText;
use crate::template_context_error::TemplateContextError;
use crate::template_reference_index::{TargetSelectionKind, TemplateReferences};
use crate::template_tag_cache::TemplateTagCache;
use crate::{CatalogError, ExportAccountingError};

/// A bound adapter for the existing spawn and location-query algorithms.
pub(crate) struct TemplateSpawnContext<'context, 'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    references: &'context RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
    tags: &'context TemplateTagCache<'context, 'fields, 'scope, 'read, 'source>,
    inputs: RefCell<TagInputs>,
    capture: &'context dyn FieldCapture<'source>,
}

impl<'context, 'fields, 'scope, 'read, 'source>
    TemplateSpawnContext<'context, 'fields, 'scope, 'read, 'source>
{
    pub(crate) fn new(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        references: &'context RefCell<TemplateReferences<'fields, 'scope, 'read, 'source>>,
        tags: &'context TemplateTagCache<'context, 'fields, 'scope, 'read, 'source>,
        root: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
        capture: &'context dyn FieldCapture<'source>,
    ) -> Result<Self, CatalogError> {
        let mut uses = Vec::new();
        if let Some(root) = root {
            fields.span(root)?;
            uses.push(root.observe(OriginalValuePart::Value, SourceUseRole::Conversion)?);
        }
        Ok(Self {
            fields,
            references,
            tags,
            capture,
            inputs: RefCell::new(TagInputs::local(uses, true)),
        })
    }

    pub(crate) fn finish(self) -> TagInputs {
        self.inputs.into_inner()
    }

    fn retain(
        &self,
        uses: impl IntoIterator<Item = SourceUse>,
        complete: bool,
    ) -> Result<(), CatalogError> {
        let mut inputs = self
            .inputs
            .try_borrow_mut()
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        inputs.retain(uses, complete);
        Ok(())
    }

    fn retain_datum<T>(&self, datum: &OriginalDatum<T>) -> Result<(), CatalogError> {
        let (uses, complete) = datum.contributions();
        self.retain(uses.iter().cloned(), complete)
    }

    fn unreadable(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        error: ValueError,
    ) -> Result<ReadError, CatalogError> {
        self.retain(
            [fact.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?],
            false,
        )?;
        if !matches!(fact.value(), Value::Error(actual) if *actual == error) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let parent = self.fields.guard_parent(fact)?;
        let path = self.fields.logical_path(fact)?;
        Ok(
            FieldViolation::invalid(parent, "", &path, fact.value(), "a readable source value")
                .captured_error(self.capture),
        )
    }

    fn read_value(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        structural: bool,
    ) -> Result<(), ReadError> {
        let role = match fact.value() {
            Value::Error(error) => return Err(self.unreadable(fact, *error)?),
            Value::Reference(_) | Value::Struct(_) | Value::Array(_) => SourceUseRole::Selector,
            _ if structural => SourceUseRole::Selector,
            _ => SourceUseRole::Payload,
        };
        self.retain([fact.observe(OriginalValuePart::Value, role)?], true)?;
        Ok(())
    }

    fn dangling(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        role: SourceUseRole,
    ) -> Result<bool, ReadError> {
        let Some(guid) = fact.dangling_guid(role)? else {
            return Ok(false);
        };
        self.retain_datum(&guid)?;
        self.retain(
            [fact.observe(
                OriginalValuePart::Error(OriginalErrorParameter::Guid),
                SourceUseRole::Conversion,
            )?],
            true,
        )?;
        Ok(true)
    }

    fn tag_failure(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<ReadError, CatalogError> {
        Ok(Box::new(TemplateContextError::TagLookupUnavailable {
            field: self.fields.logical_path(fact)?,
            source_type: fact.owner_type().to_owned(),
        }))
    }
}

impl<'source> ReadContext<'source> for TemplateSpawnContext<'_, '_, '_, '_, 'source> {
    fn capture(&self) -> &dyn FieldCapture<'source> {
        self.capture
    }

    fn check_tree(&self, tree: &RecordTree<'source>) -> Result<(), ReadError> {
        self.fields.check_tree(tree)?;
        Ok(())
    }

    fn field(&self, parent: &StructValue<'source>, name: &str) -> Result<(), ReadError> {
        let owner = self.fields.owner_of(parent)?;
        let Some((_, fact)) = self.fields.field(owner, name)? else {
            self.retain([self.fields.survey(owner)?], true)?;
            return Ok(());
        };
        self.retain(
            [fact.observe(OriginalValuePart::FieldName, SourceUseRole::Selector)?],
            true,
        )?;
        Ok(())
    }

    fn scalar(&self, value: &Value<'source>) -> Result<(), ReadError> {
        self.read_value(self.fields.fact(value)?, false)
    }

    fn element(&self, value: &Value<'source>) -> Result<(), ReadError> {
        self.read_value(self.fields.fact(value)?, true)
    }

    fn source_type(&self, source: &StructValue<'source>) -> Result<(), ReadError> {
        self.retain([self.fields.observe_type(source)?], true)?;
        Ok(())
    }

    fn tag(&self, value: &Value<'source>) -> Result<Option<SpawnTagText>, ReadError> {
        let fact = self.fields.fact(value)?;
        if !matches!(
            value,
            Value::Reference(_) | Value::Error(ValueError::DanglingReference { .. })
        ) {
            self.read_value(fact, true)?;
            return Ok(None);
        }
        let datum = match self.tags.lookup(self.references, fact)? {
            TagPathLookup::Ready { value, .. } | TagPathLookup::Unavailable { value } => value,
        };
        let (value, origins) = datum.split();
        let complete = origins.is_complete();
        self.inputs
            .try_borrow_mut()
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?
            .merge(origins)?;
        if !complete {
            return Err(self.tag_failure(fact)?);
        }
        match value {
            SourceValue::Present(path) => Ok(Some(SpawnTagText::shared(path))),
            SourceValue::Absent => Ok(None),
            SourceValue::Unreadable => Err(self.tag_failure(fact)?),
            _ => Err(Box::new(CatalogError::from(
                ExportAccountingError::ConflictingSourceDatum,
            ))),
        }
    }

    fn variable(&self, value: &Value<'source>) -> Result<Option<String>, ReadError> {
        let fact = self.fields.fact(value)?;
        if self.dangling(fact, SourceUseRole::Selector)? {
            return Ok(None);
        }
        self.read_value(fact, true)?;
        let mut references = self
            .references
            .try_borrow_mut()
            .map_err(|_| CatalogError::from(ExportAccountingError::ConflictingSourceDatum))?;
        let selected = references.select_fact(fact, "missionVariableName")?;
        let (uses, complete) = references.contributions(&selected)?;
        self.retain(uses, complete)?;
        match selected.kind() {
            TargetSelectionKind::Instance {
                field: Some(field), ..
            } => {
                self.read_value(field, false)?;
                Ok(match field.value() {
                    Value::String(value) | Value::Enum(value) if !value.is_empty() => {
                        Some((*value).to_owned())
                    }
                    _ => None,
                })
            }
            _ => Ok(None),
        }
    }

    fn authored_reference(&self, value: &Value<'source>) -> Result<(), ReadError> {
        let fact = self.fields.fact(value)?;
        if self.dangling(fact, SourceUseRole::Payload)? {
            return Ok(());
        }
        self.read_value(fact, true)?;
        if !matches!(value, Value::Reference(_)) {
            return Ok(());
        }
        let mut references = self
            .references
            .try_borrow_mut()
            .map_err(|_| CatalogError::from(ExportAccountingError::ConflictingSourceDatum))?;
        let captured = references.observation(fact)?.capture_view()?;
        self.retain_datum(&captured.guid)?;
        self.retain_datum(&captured.record_index)?;
        self.retain_datum(&captured.record_path)?;
        let OriginalTargetShape::Record { source_type } = captured.shape else {
            return Err(Box::new(CatalogError::from(
                ExportAccountingError::ConflictingSourceDatum,
            )));
        };
        self.retain_datum(&source_type)?;
        Ok(())
    }

    fn record_guid(&self, value: &Value<'source>) -> Result<Option<CatalogGuid>, ReadError> {
        let fact = self.fields.fact(value)?;
        if self.dangling(fact, SourceUseRole::Selector)? {
            return Ok(None);
        }
        self.read_value(fact, true)?;
        if !matches!(value, Value::Reference(reference) if reference.location().is_none()) {
            return Ok(None);
        }
        let mut references = self
            .references
            .try_borrow_mut()
            .map_err(|_| CatalogError::from(ExportAccountingError::ConflictingSourceDatum))?;
        let datum: OriginalDatum<Guid> = references.observation(fact)?.guid()?;
        self.retain_datum(&datum)?;
        match datum.value() {
            SourceValue::Present(guid) => Ok(Some(CatalogGuid(*guid))),
            _ => Err(Box::new(CatalogError::from(
                ExportAccountingError::ConflictingSourceDatum,
            ))),
        }
    }
}
