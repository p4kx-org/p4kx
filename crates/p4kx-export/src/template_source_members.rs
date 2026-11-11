// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashMap};

use p4kx_extraction::{DecimalU64, Value};

use crate::catalog_read::original::{
    ExampleBodyCapture, OriginalFlowFields, OriginalValueEdge, OriginalValueFact, OriginalValuePart,
};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::source_value_catalog::SourceValueRef;
use crate::template_issues::TemplateWarningExample;
use crate::template_issues::Unknowns;
use crate::template_source_refs::{PhysicalTemplateRef, SourceFieldRef};
use crate::{CatalogError, ExportAccountingError};

/// The union of actual same-frame source subtrees in canonical original DFS order.
pub(crate) struct TemplateMembers<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    roots: Vec<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    values: Vec<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    indices: HashMap<u64, SourceValueRef>,
    root_indices: HashMap<u64, DecimalU64>,
    warning: Option<WarningCapture<'fields, 'scope, 'read, 'source>>,
}

struct WarningCapture<'fields, 'scope, 'read, 'source> {
    body: &'fields ExampleBodyCapture<'scope, 'read, 'source>,
    owner: FlowOwnerKind,
}

impl<'fields, 'scope, 'read, 'source> TemplateMembers<'fields, 'scope, 'read, 'source> {
    pub(crate) fn new(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        wanted: impl IntoIterator<Item = &'fields OriginalValueFact<'scope, 'read, 'source>>,
    ) -> Result<Self, CatalogError> {
        let mut starts = BTreeSet::new();
        for fact in wanted {
            if !matches!(fact.edge(), OriginalValueEdge::Field { .. }) {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            starts.insert(fields.span(fact)?.start);
        }
        let mut result = Self {
            fields,
            roots: Vec::new(),
            values: Vec::new(),
            indices: HashMap::new(),
            root_indices: HashMap::new(),
            warning: None,
        };
        let mut covered_until = 0;
        for start in starts {
            let fact = fields
                .facts()
                .get(start)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            let span = fields.span(fact)?;
            if start < covered_until {
                if span.end > covered_until {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                continue;
            }
            covered_until = span.end;
            let root_index = u64::try_from(result.roots.len())
                .map_err(|_| ExportAccountingError::CounterOverflow)?;
            result
                .root_indices
                .insert(fact.ordinal(), DecimalU64::new(root_index));
            result.roots.push(fact);
            for fact in fields
                .facts()
                .get(span)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?
            {
                fields.check_cancel()?;
                let index = u64::try_from(result.values.len())
                    .map_err(|_| ExportAccountingError::CounterOverflow)?;
                if result
                    .indices
                    .insert(
                        fact.ordinal(),
                        SourceValueRef {
                            value_index: DecimalU64::new(index),
                        },
                    )
                    .is_some()
                {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                result.values.push(fact);
            }
        }
        Ok(result)
    }

    pub(crate) const fn with_warning_capture(
        mut self,
        body: &'fields ExampleBodyCapture<'scope, 'read, 'source>,
        owner: FlowOwnerKind,
    ) -> Self {
        self.warning = Some(WarningCapture { body, owner });
        self
    }

    pub(crate) fn parse_warning<T>(
        &self,
        unknowns: &Unknowns,
        category: &'static str,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        parse: impl FnOnce(&str) -> Option<T>,
    ) -> Result<Option<T>, CatalogError> {
        let (Value::String(value) | Value::Enum(value)) = fact.value() else {
            return Ok(None);
        };
        if value.is_empty() {
            return Ok(None);
        }
        unknowns.parse(category, value, parse, || {
            self.warning_example(fact, OriginalValuePart::Value)
        })
    }

    pub(crate) fn parse_type_warning<T>(
        &self,
        unknowns: &Unknowns,
        category: &'static str,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        parse: impl FnOnce(&str) -> Option<T>,
    ) -> Result<Option<T>, CatalogError> {
        let Value::Struct(value) = fact.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        unknowns.parse_type(category, value.type_name(), parse, || {
            self.warning_example(fact, OriginalValuePart::ConcreteType)
        })
    }

    fn warning_example(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        part: OriginalValuePart,
    ) -> Result<Option<TemplateWarningExample>, CatalogError> {
        let Some(capture) = &self.warning else {
            return Ok(None);
        };
        let ancestors = self.fields.ancestors(fact)?;
        let path = self.fields.logical_path(fact)?;
        Ok(Some(TemplateWarningExample {
            owner: capture.owner,
            example: capture
                .body
                .vocabulary_example(fact, &ancestors, &path, part)?,
        }))
    }

    pub(crate) fn physical(&self) -> PhysicalTemplateRef {
        PhysicalTemplateRef {
            input_ref: self.fields.input_ref().clone(),
            record_index: self.fields.record_index(),
        }
    }
    pub(crate) const fn fields(&self) -> &'fields OriginalFlowFields<'scope, 'read, 'source> {
        self.fields
    }

    pub(crate) fn roots(&self) -> &[&'fields OriginalValueFact<'scope, 'read, 'source>] {
        &self.roots
    }

    pub(crate) fn values(&self) -> &[&'fields OriginalValueFact<'scope, 'read, 'source>] {
        &self.values
    }

    pub(crate) fn value_ref(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<SourceValueRef, CatalogError> {
        self.fields.span(fact)?;
        self.indices
            .get(&fact.ordinal())
            .copied()
            .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
    }

    pub(crate) fn field_ref(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<SourceFieldRef, CatalogError> {
        self.value_ref(fact)?;
        let OriginalValueEdge::Field { ordinal } = fact.edge() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        if let Some(index) = self.root_indices.get(&fact.ordinal()) {
            return Ok(SourceFieldRef::Root {
                root_field_index: *index,
            });
        }
        let parent = fact
            .parent_ordinal()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let parent = self
            .fields
            .facts()
            .get(usize::try_from(parent).map_err(|_| ExportAccountingError::CounterOverflow)?)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        Ok(SourceFieldRef::Field {
            parent: self.value_ref(parent)?,
            field_index: DecimalU64::new(ordinal),
        })
    }
}
