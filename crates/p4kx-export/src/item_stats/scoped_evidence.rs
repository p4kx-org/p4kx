// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForgeError, Record, RecordTree, SourceEvidenceError, SourceUse, SourceUseRole,
    SourceUses, Value,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{
    OriginalFlowFields, OriginalReferenceObservation, OriginalValueEvent, OriginalValueFact,
    OriginalValuePart,
};
use crate::record_header_field::RecordHeaderField;
use crate::variant_evidence::VariantEvidence;
use crate::{CatalogError, ExportAccountingError};

#[derive(Default)]
pub(super) struct Evidence {
    uses: Vec<SourceUse>,
    incomplete: bool,
}

impl Evidence {
    pub(super) fn owner(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
    ) -> Result<(), CatalogError> {
        self.uses.push(
            input
                .record_context(record)?
                .observe("", SourceUseRole::Payload)?,
        );
        Ok(())
    }

    pub(super) fn select(
        &mut self,
        fields: &OriginalFlowFields<'_, '_, '_>,
        occurrence: Option<&Value<'_>>,
    ) -> Result<(), CatalogError> {
        let selection = fields.component_selection(occurrence)?;
        self.uses.extend(selection.uses);
        self.incomplete |= !selection.complete;
        Ok(())
    }

    pub(super) fn named_value(
        &mut self,
        fields: &OriginalFlowFields<'_, '_, '_>,
        parent: &Value<'_>,
        name: &str,
    ) -> Result<(), CatalogError> {
        let parent = fields.fact(parent)?;
        if let Some((_, fact)) = fields.field(Some(parent), name)? {
            self.value(fact, SourceUseRole::Payload)
        } else {
            self.uses.push(fields.survey(Some(parent))?);
            Ok(())
        }
    }

    pub(super) fn selected_link(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        fields: &OriginalFlowFields<'_, '_, '_>,
        selector: &Value<'_>,
        selected: &Record<'_>,
    ) -> Result<(), CatalogError> {
        let fact = fields.fact(selector)?;
        let original = fact.reference_observation()?;
        if original.record_index() != selected.index()
            || !original
                .reference()
                .original_target()
                .shares_source_with(selected)
        {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        self.capture_reference(input, &original, selected)
            .map(|_| ())
    }

    pub(super) fn link_field(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        fields: &OriginalFlowFields<'_, '_, '_>,
        parent: &Value<'_>,
        name: &str,
        rejected_type: Option<&str>,
    ) -> Result<(), CatalogError> {
        let parent = fields.fact(parent)?;
        let Some((_, fact)) = fields.field(Some(parent), name)? else {
            self.uses.push(fields.survey(Some(parent))?);
            return Ok(());
        };
        if !matches!(fact.value(), Value::Reference(_)) {
            if fact.observe_dangling_reference(&mut self.uses)? {
                return Ok(());
            }
            return self.value(fact, SourceUseRole::Selector);
        }
        let original = fact.reference_observation()?;
        match original.record() {
            Ok(target) => {
                let actual = self.capture_reference(input, &original, &target)?;
                self.incomplete |= rejected_type.is_some_and(|expected| actual == Some(expected));
            }
            Err(CatalogError::DataForge(_)) => {
                self.incomplete = true;
                self.uses
                    .push(fact.observe(OriginalValuePart::Value, SourceUseRole::Attempted)?);
            }
            Err(error) => return Err(error),
        }
        Ok(())
    }

    fn capture_reference<'source>(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        original: &OriginalReferenceObservation<'_, '_, '_, 'source>,
        target: &Record<'source>,
    ) -> Result<Option<&'source str>, CatalogError> {
        let origin = original.guid_selector()?;
        let (uses, complete) = origin.contributions();
        self.uses.extend_from_slice(uses);
        self.incomplete |= !complete;
        let context = input.record_context(target)?;
        self.uses
            .push(context.observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Selector)?);
        let source_type = target.type_name().ok();
        let role = if source_type.is_some() {
            SourceUseRole::Selector
        } else {
            SourceUseRole::Attempted
        };
        self.uses
            .push(context.observe_header(RecordHeaderField::SourceType, role)?);
        self.incomplete |= source_type.is_none();
        Ok(source_type)
    }

    pub(super) fn body(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
        body: Result<&RecordTree<'_>, &DataForgeError>,
        root_field: Option<&str>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        match body {
            Ok(tree) => {
                let scope = input.record_scope(record, tree)?;
                if let Some(name) = root_field {
                    let fields = OriginalFlowFields::read(&scope, cancel)?;
                    self.named_body(&fields, name)?;
                } else {
                    self.owner(input, record)?;
                    let mut walk = scope.walk(cancel);
                    while let Some(event) = walk.next_event()? {
                        if let OriginalValueEvent::Value(site) = event
                            && matches!(site.value(), Value::Error(_))
                        {
                            self.value(&site.fact(), SourceUseRole::Payload)?;
                        }
                    }
                }
                Ok(())
            }
            Err(DataForgeError::ObservationStopped) => Err(CatalogError::Cancelled),
            Err(_) => self.failed_body(input, record),
        }
    }

    fn named_body(
        &mut self,
        fields: &OriginalFlowFields<'_, '_, '_>,
        name: &str,
    ) -> Result<(), CatalogError> {
        let Some((_, root)) = fields.field(None, name)? else {
            self.uses.push(fields.survey(None)?);
            return Ok(());
        };
        self.value(root, SourceUseRole::Payload)?;
        let span = fields.span(root)?;
        let facts = fields
            .facts()
            .get(span)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        for fact in facts {
            if matches!(fact.value(), Value::Error(_)) {
                self.value(fact, SourceUseRole::Payload)?;
            }
        }
        Ok(())
    }

    pub(super) fn failed_body(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
    ) -> Result<(), CatalogError> {
        self.incomplete = true;
        self.uses.push(
            input
                .record_context(record)?
                .observe("", SourceUseRole::Attempted)?,
        );
        Ok(())
    }

    fn value(
        &mut self,
        fact: &OriginalValueFact<'_, '_, '_>,
        role: SourceUseRole,
    ) -> Result<(), CatalogError> {
        let readable = !matches!(fact.value(), Value::Error(_));
        self.incomplete |= !readable;
        self.uses.push(fact.observe(
            OriginalValuePart::Value,
            if readable {
                role
            } else {
                SourceUseRole::Attempted
            },
        )?);
        Ok(())
    }

    pub(super) fn extend(&mut self, other: Self) {
        self.uses.extend(other.uses);
        self.incomplete |= other.incomplete;
    }

    pub(super) fn finish(self, sources: &SourceUses) -> VariantEvidence {
        VariantEvidence::from_uses(sources, &self.uses, !self.incomplete)
    }
}
