// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::ops::Range;

use p4kx_extraction::{DecimalU64, Value};

use crate::catalog_read::original::{
    ExampleBodyCapture, OriginalFlowFields, OriginalValueEdge, OriginalValueFact, OriginalValuePart,
};
use crate::health_diagnostic_observation::ExampleObservation;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::source_value_catalog::{SourceValueRef, retained_facts_complete};
use crate::template_issues::Unknowns;
use crate::template_source_refs::{PhysicalTemplateRef, SourceFieldRef};
use crate::{CatalogError, ExportAccountingError};

/// The union of actual same-frame source subtrees in canonical original DFS order.
pub(crate) struct TemplateMembers<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    spans: Vec<MemberSpan<'fields, 'scope, 'read, 'source>>,
    value_count: usize,
    owner: FlowOwnerKind,
    warning: Option<&'fields ExampleBodyCapture<'scope, 'read, 'source>>,
}

struct MemberSpan<'fields, 'scope, 'read, 'source> {
    original: Range<usize>,
    root: &'fields OriginalValueFact<'scope, 'read, 'source>,
    values: &'fields [OriginalValueFact<'scope, 'read, 'source>],
    value_start: usize,
}

impl<'fields, 'scope, 'read, 'source> TemplateMembers<'fields, 'scope, 'read, 'source> {
    pub(crate) fn new(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        wanted: impl IntoIterator<Item = &'fields OriginalValueFact<'scope, 'read, 'source>>,
        owner: FlowOwnerKind,
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
            spans: Vec::new(),
            value_count: 0,
            owner,
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
            let value_start = result.value_count;
            result.value_count = value_start
                .checked_add(span.len())
                .ok_or(ExportAccountingError::CounterOverflow)?;
            let values = fields
                .facts()
                .get(span.clone())
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            result.spans.push(MemberSpan {
                original: span,
                root: fact,
                values,
                value_start,
            });
        }
        Ok(result)
    }

    pub(crate) const fn with_warning_capture(
        mut self,
        body: &'fields ExampleBodyCapture<'scope, 'read, 'source>,
    ) -> Self {
        self.warning = Some(body);
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
        unknowns.parse(self.owner, category, value, parse, || {
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
        unknowns.parse_type(self.owner, category, value.type_name(), parse, || {
            self.warning_example(fact, OriginalValuePart::ConcreteType)
        })
    }

    fn warning_example(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        part: OriginalValuePart,
    ) -> Result<Option<ExampleObservation>, CatalogError> {
        let Some(capture) = self.warning else {
            return Ok(None);
        };
        let ancestors = self.fields.ancestors(fact)?;
        let path = self.fields.logical_path(fact)?;
        Ok(Some(
            capture.vocabulary_example(fact, &ancestors, &path, part)?,
        ))
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

    pub(crate) fn roots(
        &self,
    ) -> impl Iterator<Item = &'fields OriginalValueFact<'scope, 'read, 'source>> + '_ {
        self.spans.iter().map(|span| span.root)
    }

    pub(crate) const fn value_count(&self) -> usize {
        self.value_count
    }

    pub(crate) fn values(
        &self,
    ) -> impl Iterator<Item = &'fields OriginalValueFact<'scope, 'read, 'source>> + '_ {
        self.spans.iter().flat_map(|span| span.values.iter())
    }

    pub(crate) fn retained_source_complete(&self) -> bool {
        self.spans
            .iter()
            .all(|span| retained_facts_complete(span.values))
    }

    fn containing_span(
        &self,
        ordinal: usize,
    ) -> Result<(usize, &MemberSpan<'fields, 'scope, 'read, 'source>), CatalogError> {
        let index = self
            .spans
            .partition_point(|span| span.original.start <= ordinal)
            .checked_sub(1)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        self.spans
            .get(index)
            .filter(|span| span.original.contains(&ordinal))
            .map(|span| (index, span))
            .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
    }

    pub(crate) fn value_ref(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<SourceValueRef, CatalogError> {
        let ordinal = self.fields.span(fact)?.start;
        let (_, span) = self.containing_span(ordinal)?;
        let index = span.value_start + (ordinal - span.original.start);
        Ok(SourceValueRef {
            value_index: DecimalU64::new(
                u64::try_from(index).map_err(|_| ExportAccountingError::CounterOverflow)?,
            ),
        })
    }

    pub(crate) fn field_ref(
        &self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
    ) -> Result<SourceFieldRef, CatalogError> {
        let start = self.fields.span(fact)?.start;
        let (root_index, span) = self.containing_span(start)?;
        let OriginalValueEdge::Field { ordinal } = fact.edge() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        if start == span.original.start {
            return Ok(SourceFieldRef::Root {
                root_field_index: DecimalU64::new(
                    u64::try_from(root_index)
                        .map_err(|_| ExportAccountingError::CounterOverflow)?,
                ),
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

#[cfg(test)]
#[path = "template_source_members_tests.rs"]
mod tests;
