// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{DataType, FieldOrigin, SourceUse, SourceUseRole, Value, ValueError};

use crate::CatalogError;
use crate::catalog_read::original::{
    OriginalFlowFields, OriginalReferenceObservation, OriginalValueFact, OriginalValuePart,
};
use crate::source_value_catalog::{self, SourceNode};

#[path = "template_target_selection.rs"]
mod selection;

#[path = "template_reference_navigation.rs"]
mod navigation;

#[path = "template_reference_failure.rs"]
mod failure;

pub(crate) use selection::{TargetSelection, TargetSelectionKind};

/// One real reference admission reused by interpretation and original raw serialization.
pub(crate) struct TemplateReferences<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    observations: HashMap<u64, OriginalReferenceObservation<'fields, 'scope, 'read, 'source>>,
}

impl<'fields, 'scope, 'read, 'source> TemplateReferences<'fields, 'scope, 'read, 'source> {
    pub(crate) fn new(fields: &'fields OriginalFlowFields<'scope, 'read, 'source>) -> Self {
        Self {
            fields,
            observations: HashMap::new(),
        }
    }

    pub(crate) fn select(
        &mut self,
        parent: &'fields OriginalValueFact<'scope, 'read, 'source>,
        caller_name: &str,
        target_name: &str,
    ) -> Result<TargetSelection<'fields, 'scope, 'read, 'source>, CatalogError> {
        let Some((_, caller)) = self.fields.field(Some(parent), caller_name)? else {
            return Ok(TargetSelection {
                parent: Some(parent),
                caller: None,
                kind: TargetSelectionKind::Absent,
                resolved: false,
                selectors: Vec::new(),
                retained_root: None,
            });
        };
        let mut selected = self.select_fact(caller, target_name)?;
        selected.parent = Some(parent);
        Ok(selected)
    }

    pub(crate) fn select_fact(
        &mut self,
        caller: &'fields OriginalValueFact<'scope, 'read, 'source>,
        target_name: &str,
    ) -> Result<TargetSelection<'fields, 'scope, 'read, 'source>, CatalogError> {
        self.select_slot(caller, target_name, true)
    }

    pub(crate) fn select_owned_fact(
        &mut self,
        caller: &'fields OriginalValueFact<'scope, 'read, 'source>,
        target_name: &str,
    ) -> Result<TargetSelection<'fields, 'scope, 'read, 'source>, CatalogError> {
        self.select_slot(caller, target_name, false)
    }

    fn select_slot(
        &mut self,
        caller: &'fields OriginalValueFact<'scope, 'read, 'source>,
        target_name: &str,
        weak_storage: bool,
    ) -> Result<TargetSelection<'fields, 'scope, 'read, 'source>, CatalogError> {
        self.fields.span(caller)?;
        let kind = self.target(caller, target_name, weak_storage)?;
        Ok(TargetSelection {
            parent: None,
            caller: Some(caller),
            resolved: matches!(kind, TargetSelectionKind::Instance { .. }),
            kind,
            selectors: Vec::new(),
            retained_root: None,
        })
    }

    pub(crate) fn select_field(
        &self,
        selected: &TargetSelection<'fields, 'scope, 'read, 'source>,
        target_name: &str,
    ) -> Result<TargetSelection<'fields, 'scope, 'read, 'source>, CatalogError> {
        if let Some(caller) = selected.caller {
            self.fields.span(caller)?;
        }
        let kind = match selected.kind {
            TargetSelectionKind::Instance { owner, .. } => {
                let field = self
                    .fields
                    .field(Some(owner), target_name)?
                    .map(|(_, fact)| fact);
                TargetSelectionKind::Instance { owner, field }
            }
            kind => kind,
        };
        Ok(TargetSelection {
            parent: selected.parent,
            caller: selected.caller,
            kind,
            resolved: selected.resolved,
            selectors: selected.selectors.clone(),
            retained_root: selected.retained_root,
        })
    }

    fn target(
        &mut self,
        caller: &'fields OriginalValueFact<'scope, 'read, 'source>,
        target_name: &str,
        weak_storage: bool,
    ) -> Result<TargetSelectionKind<'fields, 'scope, 'read, 'source>, CatalogError> {
        match caller.value() {
            Value::Null => return Ok(TargetSelectionKind::Null),
            Value::Error(ValueError::OutsideRecord { .. }) => {
                return Ok(TargetSelectionKind::OutsideOwner);
            }
            Value::Error(
                ValueError::InvalidPointer { .. } | ValueError::DanglingReference { .. },
            ) => return Ok(TargetSelectionKind::Unresolved),
            Value::Error(_) => return Ok(TargetSelectionKind::Unreadable),
            Value::Reference(_) => {}
            _ => return Ok(TargetSelectionKind::WrongShape),
        }
        if weak_storage && caller.field().storage().data_type() != DataType::WeakPointer {
            return Ok(TargetSelectionKind::WrongStorage);
        }
        let fields = self.fields;
        let observed = self.observation(caller)?;
        if observed.record_index() != fields.record_index() {
            return Ok(TargetSelectionKind::OutsideOwner);
        }
        if !observed.is_instance() {
            return Ok(TargetSelectionKind::RecordTarget);
        }
        let Some(owner) = fields.target_fact(observed)? else {
            return Ok(TargetSelectionKind::Unresolved);
        };
        let field = fields
            .field(Some(owner), target_name)?
            .map(|(_, fact)| fact);
        Ok(TargetSelectionKind::Instance { owner, field })
    }

    pub(crate) fn observation(
        &mut self,
        caller: &'fields OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<&OriginalReferenceObservation<'fields, 'scope, 'read, 'source>, CatalogError> {
        use std::collections::hash_map::Entry;
        self.fields.span(caller)?;
        Ok(match self.observations.entry(caller.ordinal()) {
            Entry::Vacant(entry) => entry.insert(caller.reference_observation()?),
            Entry::Occupied(entry) => entry.into_mut(),
        })
    }

    pub(crate) fn origin(
        &self,
        selected: &TargetSelection<'fields, 'scope, 'read, 'source>,
    ) -> Result<FieldOrigin, CatalogError> {
        let (inputs, complete) = self.contributions(selected)?;
        Ok(self.fields.sources().origin(&inputs, complete))
    }

    pub(crate) fn contributions(
        &self,
        selected: &TargetSelection<'fields, 'scope, 'read, 'source>,
    ) -> Result<(Vec<SourceUse>, bool), CatalogError> {
        let Some(caller) = selected.caller else {
            return Ok((vec![self.fields.survey(selected.parent)?], true));
        };
        self.fields.span(caller)?;
        let mut inputs = Vec::new();
        let mut complete = true;
        for (fact, part) in &selected.selectors {
            self.fields.span(fact)?;
            let readable = !matches!(fact.value(), Value::Error(_));
            inputs.push(fact.observe(
                *part,
                if readable {
                    SourceUseRole::Selector
                } else {
                    SourceUseRole::Attempted
                },
            )?);
            complete &= readable;
        }
        if selected.resolved {
            inputs.push(caller.observe(OriginalValuePart::Value, SourceUseRole::Selector)?);
            inputs.push(caller.observe(OriginalValuePart::Value, SourceUseRole::Conversion)?);
            if let TargetSelectionKind::Instance { owner, field } = selected.kind {
                self.fields.span(owner)?;
                if let Some(field) = field {
                    self.fields.span(field)?;
                    let readable = !matches!(field.value(), Value::Error(_));
                    inputs.push(field.observe(
                        OriginalValuePart::Value,
                        if readable {
                            SourceUseRole::Payload
                        } else {
                            SourceUseRole::Attempted
                        },
                    )?);
                    complete &= readable;
                } else {
                    inputs.push(self.fields.survey(Some(owner))?);
                }
            }
        } else if let Some(failure) = failure::inputs(caller)? {
            inputs.extend(failure);
        } else {
            let readable = !matches!(caller.value(), Value::Error(_));
            inputs.push(caller.observe(
                OriginalValuePart::Value,
                if readable {
                    SourceUseRole::Payload
                } else {
                    SourceUseRole::Attempted
                },
            )?);
            complete &= readable;
        }
        Ok((inputs, complete))
    }

    pub(crate) fn raw(
        &mut self,
        fact: &'fields OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<SourceNode<'source>, CatalogError> {
        self.fields.span(fact)?;
        match self.observations.remove(&fact.ordinal()) {
            Some(observed) => source_value_catalog::read_reference(observed, self.fields.sources()),
            None => source_value_catalog::read_scalar(fact, self.fields.sources()),
        }
    }
}
