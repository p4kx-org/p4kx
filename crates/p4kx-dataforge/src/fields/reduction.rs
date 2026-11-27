// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter;
use std::ops::Range;

use super::trie::{Node, ROOT};
use super::{
    FieldStat, FieldStatistics, FieldStatisticsInput, FieldStatisticsMetricCoverage,
    FieldStatisticsScalarRead,
};

/// The physical input used by a selected numeric or distinct operand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldStatisticsOperand {
    input: FieldStatisticsInput,
    text: Option<Range<u64>>,
    scalar_read: FieldStatisticsScalarRead,
}

impl FieldStatisticsOperand {
    pub(super) const fn new(
        input: FieldStatisticsInput,
        text: Option<Range<u64>>,
        scalar_read: FieldStatisticsScalarRead,
    ) -> Self {
        Self {
            input,
            text,
            scalar_read,
        }
    }

    /// Borrows the original slot or array position.
    #[must_use]
    pub const fn input(&self) -> &FieldStatisticsInput {
        &self.input
    }

    /// Returns the same decoder inputs used by this selected comparison operand.
    #[must_use]
    pub const fn scalar_read(&self) -> FieldStatisticsScalarRead {
        self.scalar_read
    }

    /// Borrows the exact original text range for a consulted textual operand.
    #[must_use]
    pub const fn text_span(&self) -> Option<&Range<u64>> {
        self.text.as_ref()
    }
}

/// One fingerprint with the physical operand retained by the actual distinct reducer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldStatisticsFingerprint {
    fingerprint: u64,
    operand: Option<FieldStatisticsOperand>,
}

impl FieldStatisticsFingerprint {
    pub(super) const fn new(fingerprint: u64, operand: Option<FieldStatisticsOperand>) -> Self {
        Self {
            fingerprint,
            operand,
        }
    }

    pub(super) const fn into_operand(self) -> Option<FieldStatisticsOperand> {
        self.operand
    }

    /// Returns the fingerprint used by the existing distinct reducer.
    #[must_use]
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Borrows the original representative if its physical coordinates were observed.
    #[must_use]
    pub const fn operand(&self) -> Option<&FieldStatisticsOperand> {
        self.operand.as_ref()
    }
}

/// The remaining source coverage of one reduced field-statistics scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldStatisticsSourceScope {
    /// The caller has not bound the record selection and enclosing paths to physical sources.
    SelectionAndPathsUnbound,
    /// The physical header census behind the complete all-Record path order is observed.
    #[non_exhaustive]
    AllRecordPathsObserved {
        /// Physical Records omitted by the canonical path order.
        unreadable_paths: u64,
        /// Admitted Record paths whose ordering name could not be decoded.
        unreadable_names: u64,
    },
}

/// Ordinary field statistics with exact final comparison representatives.
#[derive(Debug)]
pub struct FieldStatisticsReduction<'source> {
    statistics: FieldStatistics,
    rows: Vec<usize>,
    nodes: Vec<ReducedNode<'source>>,
    source_scope: FieldStatisticsSourceScope,
}

impl<'source> FieldStatisticsReduction<'source> {
    pub(super) fn empty() -> Self {
        Self::new(FieldStatistics::default(), Vec::new(), Vec::new())
    }
    pub(super) const fn new(
        statistics: FieldStatistics,
        rows: Vec<usize>,
        nodes: Vec<ReducedNode<'source>>,
    ) -> Self {
        Self {
            statistics,
            rows,
            nodes,
            source_scope: FieldStatisticsSourceScope::SelectionAndPathsUnbound,
        }
    }

    /// Borrows the original aggregate and its unchanged values.
    #[must_use]
    pub const fn statistics(&self) -> &FieldStatistics {
        &self.statistics
    }

    /// Returns coverage of the physical input selection for these results.
    #[must_use]
    pub const fn source_scope(&self) -> FieldStatisticsSourceScope {
        self.source_scope
    }

    pub(super) const fn observe_all_paths(&mut self, unreadable_paths: u64, unreadable_names: u64) {
        self.source_scope = FieldStatisticsSourceScope::AllRecordPathsObserved {
            unreadable_paths,
            unreadable_names,
        };
    }

    /// Returns the comparison evidence at the corresponding statistic's output ordinal.
    #[must_use]
    pub fn row(&self, ordinal: usize) -> Option<FieldStatisticsReducedRow<'_, 'source>> {
        let index = *self.rows.get(ordinal)?;
        let node = self.nodes.get(index)?;
        let statistic = self.statistics.fields().get(ordinal)?;
        Some(FieldStatisticsReducedRow {
            index,
            node,
            nodes: &self.nodes,
            statistic,
        })
    }
}

#[derive(Debug)]
pub(super) struct ReducedNode<'source> {
    parent: usize,
    segment: &'source str,
    minimum: Option<FieldStatisticsOperand>,
    maximum: Option<FieldStatisticsOperand>,
    distinct: Vec<FieldStatisticsFingerprint>,
    coverage: FieldStatisticsMetricCoverage,
    descendants_incomplete: bool,
}

impl<'source> ReducedNode<'source> {
    pub(super) fn take(node: &mut Node<'source>, parents: &[Self]) -> Self {
        let inherited = parents
            .get(node.parent as usize)
            .is_some_and(|parent| parent.descendants_incomplete);
        let capped = node
            .distinct
            .as_ref()
            .is_some_and(|distinct| distinct.is_capped());
        let coverage = node.gaps.coverage(inherited, capped);
        let descendants_incomplete = node.gaps.covers_descendants(inherited);
        let (minimum, maximum) = node
            .extremes
            .as_mut()
            .map_or((None, None), |extremes| extremes.take_proof());
        let distinct = node
            .distinct
            .as_mut()
            .map_or_else(Vec::new, |distinct| distinct.take_proof());
        Self {
            parent: node.parent as usize,
            segment: node.segment,
            minimum,
            maximum,
            distinct,
            coverage,
            descendants_incomplete,
        }
    }
}

/// The unchanged aggregate and comparison evidence for one merged structural path.
#[derive(Debug)]
pub struct FieldStatisticsReducedRow<'result, 'source> {
    index: usize,
    node: &'result ReducedNode<'source>,
    nodes: &'result [ReducedNode<'source>],
    statistic: &'result FieldStat,
}

impl<'result> FieldStatisticsReducedRow<'result, '_> {
    /// Borrows the unchanged statistics of this row.
    #[must_use]
    pub const fn statistic(&self) -> &'result FieldStat {
        self.statistic
    }

    /// Returns original path segments from this row back towards the Record.
    pub fn leaf_to_root(&self) -> impl Iterator<Item = &str> + '_ {
        iter::successors(Some(self.index), |&node| {
            self.nodes.get(node).map(|node| node.parent)
        })
        .take_while(|&node| node != ROOT as usize)
        .filter_map(|node| self.nodes.get(node).map(|node| node.segment))
    }

    /// Returns the coverage of each metric within the admitted Record scan.
    #[must_use]
    pub const fn metric_coverage(&self) -> FieldStatisticsMetricCoverage {
        self.node.coverage
    }

    /// Borrows the winning minimum operand when its physical position was observed.
    #[must_use]
    pub const fn minimum(&self) -> Option<&FieldStatisticsOperand> {
        self.node.minimum.as_ref()
    }

    /// Borrows the winning maximum operand when its physical position was observed.
    #[must_use]
    pub const fn maximum(&self) -> Option<&FieldStatisticsOperand> {
        self.node.maximum.as_ref()
    }

    /// Borrows the final distinct representatives, including the witness that capped the count.
    #[must_use]
    pub fn distinct(&self) -> &[FieldStatisticsFingerprint] {
        &self.node.distinct
    }
}
