// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashMap;

use p4kx_extraction::{CancelToken, SourceEvidenceError, StructValue, Value};

use super::{OriginalRecordScope, OriginalValueEdge, OriginalValueEvent, OriginalValueFact};
use crate::{CatalogError, ExportAccountingError};

/// One same-frame correspondence between original typed edges and sealed occurrences.
pub(crate) struct OriginalFlowFields<'scope, 'read, 'source> {
    scope: &'scope OriginalRecordScope<'read, 'source>,
    cancel: Option<&'scope CancelToken>,
    facts: Vec<OriginalValueFact<'scope, 'read, 'source>>,
    ends: Vec<Option<usize>>,
    edges: HashMap<(Option<u64>, OriginalValueEdge), usize>,
    values: HashMap<*const (), usize>,
    structures: HashMap<*const StructValue<'source>, usize>,
    first_fields: RefCell<HashMap<Option<u64>, HashMap<&'source str, usize>>>,
}

impl<'scope, 'read, 'source> OriginalFlowFields<'scope, 'read, 'source> {
    pub(crate) fn read(
        scope: &'scope OriginalRecordScope<'read, 'source>,
        cancel: Option<&'scope CancelToken>,
    ) -> Result<Self, CatalogError> {
        Self::read_observed(scope, cancel, |_| Ok(()))
    }

    /// Visits the same actual events while indexing their original facts.
    pub(crate) fn read_observed(
        scope: &'scope OriginalRecordScope<'read, 'source>,
        cancel: Option<&'scope CancelToken>,
        mut observe: impl FnMut(
            &OriginalValueEvent<'_, 'scope, 'read, 'source>,
        ) -> Result<(), CatalogError>,
    ) -> Result<Self, CatalogError> {
        let mut fields = Self {
            scope,
            cancel,
            facts: Vec::new(),
            ends: Vec::new(),
            edges: HashMap::new(),
            values: HashMap::new(),
            structures: HashMap::new(),
            first_fields: RefCell::new(HashMap::new()),
        };
        let mut walk = scope.walk(cancel);
        while let Some(event) = walk.next_event()? {
            match &event {
                OriginalValueEvent::Value(site) => fields.retain(site.fact())?,
                OriginalValueEvent::EndContainer { ordinal, kind } => {
                    fields.end(*ordinal, *kind)?;
                }
            }
            observe(&event)?;
        }
        for (index, end) in fields.ends.iter().enumerate() {
            fields.check_cancel()?;
            if end.is_none() {
                return Err(ExportAccountingError::UnfilledLootValue {
                    ordinal: u64::try_from(index)
                        .map_err(|_| ExportAccountingError::CounterOverflow)?,
                }
                .into());
            }
        }
        Ok(fields)
    }

    fn retain(
        &mut self,
        fact: OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<(), CatalogError> {
        if !std::ptr::eq(fact.scope(), self.scope) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        let index =
            usize::try_from(fact.ordinal()).map_err(|_| ExportAccountingError::CounterOverflow)?;
        let edge = (fact.parent_ordinal(), fact.edge());
        let value = std::ptr::from_ref(fact.value()).cast::<()>();
        if index != self.facts.len()
            || self.edges.contains_key(&edge)
            || self.values.contains_key(&value)
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.edges.insert(edge, index);
        self.values.insert(value, index);
        let end = if matches!(fact.value(), Value::Struct(_) | Value::Array(_)) {
            None
        } else {
            Some(
                index
                    .checked_add(1)
                    .ok_or(ExportAccountingError::CounterOverflow)?,
            )
        };
        self.retain_structure(&fact, index)?;
        self.facts.push(fact);
        self.ends.push(end);
        Ok(())
    }

    fn retain_structure(
        &mut self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        index: usize,
    ) -> Result<(), CatalogError> {
        if let Value::Struct(source) = fact.value()
            && self
                .structures
                .insert(std::ptr::from_ref(source), index)
                .is_some()
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(())
    }

    pub(crate) fn check_cancel(&self) -> Result<(), CatalogError> {
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        Ok(())
    }

    pub(crate) fn fact(
        &self,
        value: &Value<'_>,
    ) -> Result<&OriginalValueFact<'scope, 'read, 'source>, CatalogError> {
        self.check_cancel()?;
        let index = self
            .values
            .get(&std::ptr::from_ref(value).cast::<()>())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let fact = self
            .facts
            .get(*index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if !std::ptr::addr_eq(fact.value(), value) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(fact)
    }

    pub(crate) fn child(
        &self,
        parent: Option<u64>,
        edge: OriginalValueEdge,
    ) -> Result<&OriginalValueFact<'scope, 'read, 'source>, CatalogError> {
        self.check_cancel()?;
        let index = self
            .edges
            .get(&(parent, edge))
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let fact = self
            .facts
            .get(*index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if fact.parent_ordinal() != parent || fact.edge() != edge {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(fact)
    }
}

#[path = "original_flow_inventory.rs"]
mod inventory;

#[cfg(test)]
#[path = "original_flow_fields_tests.rs"]
mod tests;

#[path = "original_field_survey.rs"]
mod survey;
#[path = "original_flow_target.rs"]
mod target;

#[path = "original_flow_items.rs"]
mod access;

#[path = "original_flow_path.rs"]
mod path;

#[path = "original_flow_declarations.rs"]
mod declarations;

#[path = "original_flow_error_capture.rs"]
mod error_capture;

#[path = "original_missing_field.rs"]
mod missing_field;
pub(crate) use missing_field::OriginalMissingField;
