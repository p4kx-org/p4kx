// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_dataforge::{
    DataType, FieldStatisticsFingerprint, FieldStatisticsObservation, FieldStatisticsOperand,
    FieldStatisticsReducedRow, FieldStatisticsUse, FieldStatisticsValue,
};

use super::FieldStatisticsReportError;

use crate::field_statistics_evidence::{FieldStatisticsSourceUses, storage};
use crate::{FieldOrigin, SharedFieldOrigin, SourceScope, SourceUse, SourceUseRef, SourceUses};

/// Original contributors to one emitted statistic row.
#[derive(Debug)]
#[non_exhaustive]
pub struct FieldStatisticsRowProof {
    /// Original output position before any display filter.
    pub ordinal: usize,
    /// Local traversal evidence for the field path, counts and storage kinds.
    pub structure: FieldOrigin,
    /// Actual empty and nonempty inputs of the reported empty count.
    pub empty: FieldOrigin,
    /// Actual comparison cohort for the distinct count and capped status.
    pub distinct: FieldOrigin,
    /// Actual numeric inputs considered for both extrema.
    pub extrema: FieldOrigin,
    /// Exact winning minimum operand within the numeric cohort.
    pub minimum_uses: Vec<SourceUseRef>,
    /// Exact winning maximum operand within the numeric cohort.
    pub maximum_uses: Vec<SourceUseRef>,
    /// Failed original uses at this field or its enclosing traversal path.
    pub attempts: Vec<SourceUseRef>,
}

#[derive(Default)]
pub(super) struct Uses(BTreeMap<SourceUseRef, SourceUse>);

impl Uses {
    pub(super) fn insert(&mut self, source: &SourceUse) {
        if !self.0.contains_key(source.reference()) {
            self.0.insert(source.reference().clone(), source.clone());
        }
    }

    fn extend(&mut self, sources: &[SourceUse]) {
        for source in sources {
            self.insert(source);
        }
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &SourceUse> {
        self.0.values()
    }

    pub(super) fn values(&self) -> Vec<SourceUse> {
        self.0.values().cloned().collect()
    }

    pub(super) fn refs(&self) -> Vec<SourceUseRef> {
        self.0.keys().cloned().collect()
    }
}

#[derive(Default)]
struct FieldUses {
    selectors: Uses,
    empty: Uses,
    distinct: Uses,
    numeric: Uses,
    failed_decodes: Uses,
    attempts: Uses,
}

impl FieldUses {
    fn observe(
        &mut self,
        event: &FieldStatisticsObservation<'_>,
        bound: &FieldStatisticsSourceUses,
    ) {
        self.selectors.extend(bound.selectors());
        self.attempts.extend(bound.attempts());
        let FieldStatisticsUse::Counted { value, .. } = event.usage() else {
            return;
        };
        let kind = storage(event.input());
        if matches!(value, FieldStatisticsValue::Unreadable) {
            self.failed_decodes.extend(bound.decoder_uses());
        } else {
            if may_be_empty(kind) {
                self.empty.extend(bound.operands());
                self.empty.extend(bound.decoder_uses());
            }
            if matches!(value, FieldStatisticsValue::Nonempty { .. })
                || (matches!(value, FieldStatisticsValue::Empty(_)) && may_be_distinct(kind))
            {
                self.distinct.extend(bound.operands());
                self.distinct.extend(bound.decoder_uses());
            }
        }

        if is_numeric(kind) {
            self.numeric.extend(bound.operands());
        }
    }
}

const fn may_be_empty(kind: Option<DataType>) -> bool {
    matches!(
        kind,
        Some(
            DataType::String
                | DataType::Locale
                | DataType::Enum
                | DataType::Guid
                | DataType::Reference
                | DataType::StrongPointer
                | DataType::WeakPointer
        )
    )
}

const fn may_be_distinct(kind: Option<DataType>) -> bool {
    !matches!(
        kind,
        Some(DataType::StrongPointer | DataType::WeakPointer | DataType::Class)
    )
}

const fn is_numeric(kind: Option<DataType>) -> bool {
    matches!(
        kind,
        Some(
            DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64
                | DataType::Single
                | DataType::Double
        )
    )
}

#[derive(Default)]
struct Node {
    parents: BTreeMap<String, usize>,
    proof: FieldUses,
}

#[derive(Default)]
pub(super) struct Rows {
    nodes: Vec<Node>,
}

impl Rows {
    pub(super) fn observe(
        &mut self,
        event: &FieldStatisticsObservation<'_>,
        bound: &FieldStatisticsSourceUses,
    ) {
        let branches = &mut self.nodes;
        if branches.is_empty() {
            branches.push(Node::default());
        }
        let mut index = 0;
        for segment in event.leaf_to_root() {
            let Some(node) = branches.get_mut(index) else {
                return;
            };
            if let Some(&next) = node.parents.get(segment) {
                index = next;
            } else {
                let next = branches.len();
                let Some(node) = branches.get_mut(index) else {
                    return;
                };
                node.parents.insert(segment.to_owned(), next);
                branches.push(Node::default());
                index = next;
            }
        }
        if let Some(node) = branches.get_mut(index) {
            node.proof.observe(event, bound);
        }
    }

    fn get(&self, segments: &[&str]) -> Option<&FieldUses> {
        let mut position = 0;
        for segment in segments {
            position = *self.nodes.get(position)?.parents.get(*segment)?;
        }
        self.nodes.get(position).map(|node| &node.proof)
    }

    pub(super) fn bind(
        &self,
        sources: &SourceUses,
        scope: &SourceScope,
        row: &FieldStatisticsReducedRow<'_, '_>,
        ordinal: usize,
        selection: &SharedFieldOrigin,
    ) -> Result<Option<FieldStatisticsRowProof>, FieldStatisticsReportError> {
        let segments: Vec<_> = row.leaf_to_root().collect();
        let Some(local) = self.get(&segments) else {
            return Ok(None);
        };
        let coverage = row.metric_coverage();
        let (basis, attempts) = self.enclosing(&segments);
        let distinct_uses = if row.statistic().distinct_capped() {
            bind_fingerprints(scope, row.distinct())?
        } else {
            local.distinct.values()
        };
        Ok(Some(FieldStatisticsRowProof {
            ordinal,
            structure: sources.origin_with_shared_uses(
                selection,
                basis.iter(),
                coverage.structure.is_covered(),
            )?,
            empty: sources.origin_with_shared_uses(
                selection,
                basis.iter().chain(local.empty.iter()),
                coverage.empty.is_covered(),
            )?,
            distinct: sources.origin_with_shared_uses(
                selection,
                basis.iter().chain(&distinct_uses),
                coverage.distinct.is_covered(),
            )?,
            extrema: sources.origin_with_shared_uses(
                selection,
                basis.iter().chain(local.numeric.iter()),
                coverage.extrema.is_covered(),
            )?,
            minimum_uses: operand_refs(scope, row.minimum(), row.statistic().min().is_some())?,
            maximum_uses: operand_refs(scope, row.maximum(), row.statistic().max().is_some())?,
            attempts,
        }))
    }

    pub(super) fn shape_uses(&self) -> impl Iterator<Item = &SourceUse> {
        self.nodes
            .iter()
            .skip(1)
            .flat_map(|node| node.proof.selectors.iter())
    }

    pub(super) fn checked_values(&self) -> impl Iterator<Item = &SourceUse> {
        self.shape_uses()
            .chain(self.nodes.iter().skip(1).flat_map(|node| {
                node.proof
                    .empty
                    .iter()
                    .chain(node.proof.distinct.iter())
                    .chain(node.proof.failed_decodes.iter())
            }))
    }

    pub(super) fn value_attempts(&self) -> impl Iterator<Item = &SourceUse> {
        self.nodes
            .iter()
            .skip(1)
            .flat_map(|node| node.proof.attempts.iter())
    }

    pub(super) fn root_uses(&self) -> impl Iterator<Item = &SourceUse> {
        self.get(&[])
            .into_iter()
            .flat_map(|root| root.selectors.0.values())
    }

    pub(super) fn root_attempts(&self) -> impl Iterator<Item = &SourceUse> {
        self.get(&[])
            .into_iter()
            .flat_map(|root| root.attempts.0.values())
    }

    fn enclosing(&self, segments: &[&str]) -> (Vec<SourceUse>, Vec<SourceUseRef>) {
        let mut selectors = Vec::new();
        let mut attempts = BTreeSet::new();
        for start in 0..segments.len() {
            let Some(proof) = segments.get(start..).and_then(|path| self.get(path)) else {
                continue;
            };
            selectors.extend(proof.selectors.0.values().cloned());
            attempts.extend(proof.attempts.0.keys().cloned());
        }
        (selectors, attempts.into_iter().collect())
    }
}

fn bind_fingerprints(
    scope: &SourceScope,
    fingerprints: &[FieldStatisticsFingerprint],
) -> Result<Vec<SourceUse>, FieldStatisticsReportError> {
    let mut retained = Uses::default();
    for fingerprint in fingerprints {
        let operand = fingerprint
            .operand()
            .ok_or(FieldStatisticsReportError::UnboundField)?;
        let use_sites = FieldStatisticsSourceUses::bind_operand(scope, operand)?;
        retained.extend(use_sites.operands());
        retained.extend(use_sites.decoder_uses());
    }
    Ok(retained.values())
}

fn operand_refs(
    scope: &SourceScope,
    operand: Option<&FieldStatisticsOperand>,
    expected: bool,
) -> Result<Vec<SourceUseRef>, FieldStatisticsReportError> {
    let Some(operand) = operand else {
        if expected {
            return Err(FieldStatisticsReportError::UnboundField);
        }
        return Ok(Vec::new());
    };
    let bound = FieldStatisticsSourceUses::bind_operand(scope, operand)?;
    let mut retained = Uses::default();
    retained.extend(bound.operands());
    Ok(retained.refs())
}
