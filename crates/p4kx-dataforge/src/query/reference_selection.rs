// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::query::source::QuerySource;
use crate::query::{CompiledQuery, Query, QueryError, QueryErrorKind};
use crate::record::NamedRecords;
use crate::{DataForge, ObservedReferenceIndex, ReferenceIndex, ReferenceScanGap};

/// Whether original Reference-index evidence covers the Query's lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReferenceCoverage {
    /// The Query has no Reference-index lookup or its original scan is complete.
    Complete,
    /// The observed scan has gaps or its evidence could not be retained.
    Incomplete,
}

impl Query {
    /// Compiles the Query and observes its actual Reference-index targets and incoming edges.
    /// Coverage is incomplete when only the ordinary index can be built.
    ///
    /// # Errors
    /// Returns the same Query errors as the ordinary compiler.
    pub fn compile_observing_references<'source>(
        &self,
        forge: &'source DataForge<'source>,
        mut observe: impl FnMut(QuerySource),
    ) -> Result<(CompiledQuery<'source>, ReferenceCoverage), QueryError> {
        let mut coverage = ReferenceCoverage::Complete;
        let compiled = self.compile_with_lookup(forge, |targets| {
            for &index in &targets.positions {
                observe(QuerySource::ReferenceTarget {
                    index,
                    kind: targets.kind,
                });
            }
            let found = if let Ok(index) = ObservedReferenceIndex::build(forge) {
                coverage = scan_coverage(&index, &mut observe);
                observed(&targets.positions, &index, &mut observe, &mut coverage)
            } else {
                coverage = ReferenceCoverage::Incomplete;
                ordinary(&targets.positions, &ReferenceIndex::build(forge))
            };
            Some(found)
        })?;
        Ok((compiled, coverage))
    }
}

pub(super) fn ordinary(targets: &[u32], index: &ReferenceIndex) -> Box<[u32]> {
    let found = targets
        .iter()
        .flat_map(|target| index.referrers(*target))
        .collect();
    sorted_unique(found)
}

fn scan_coverage(
    index: &ObservedReferenceIndex<'_>,
    observe: &mut impl FnMut(QuerySource),
) -> ReferenceCoverage {
    observe(QuerySource::ReferenceScan {
        end: index.forge().record_count() as u64,
    });
    for &gap in index.gaps() {
        let (record, ordinal) = match gap {
            ReferenceScanGap::Record { record_index } => (record_index, None),
            ReferenceScanGap::Value {
                record_index,
                value_ordinal,
            } => (record_index, Some(value_ordinal)),
        };
        observe(QuerySource::ReferenceGap { record, ordinal });
    }
    if index.gaps().is_empty() {
        ReferenceCoverage::Complete
    } else {
        ReferenceCoverage::Incomplete
    }
}

fn observed(
    targets: &[u32],
    index: &ObservedReferenceIndex<'_>,
    observe: &mut impl FnMut(QuerySource),
    coverage: &mut ReferenceCoverage,
) -> Box<[u32]> {
    let mut found = Vec::new();
    for &target in targets {
        let mut previous = None;
        for edge in index.index().incoming(target) {
            let source = edge.source();
            if let Some(original) = index.origin(edge) {
                observe(QuerySource::ReferenceEdge {
                    source,
                    ordinal: original.value_ordinal(),
                    target,
                });
            } else {
                *coverage = ReferenceCoverage::Incomplete;
            }
            if previous != Some(source) {
                found.push(source);
                previous = Some(source);
            }
        }
    }
    sorted_unique(found)
}

fn sorted_unique(mut indices: Vec<u32>) -> Box<[u32]> {
    indices.sort_unstable();
    indices.dedup();
    indices.into_boxed_slice()
}

impl Query {
    /// Returns the sorted indices of the Records that refer to the Records `referencing` names.
    pub(super) fn referrers(
        &self,
        forge: &DataForge<'_>,
        lookup: impl FnOnce(&NamedRecords) -> Option<Box<[u32]>>,
    ) -> Result<Option<Box<[u32]>>, QueryError> {
        let Some(referent) = &self.referencing else {
            return Ok(None);
        };
        let text = referent.as_str();
        let targets = forge.select_named_records(text);
        if targets.positions.is_empty() {
            return Err(QueryError::new(
                QueryErrorKind::UnknownReferencedRecord {
                    text: text.to_owned(),
                },
                referent.span(),
            ));
        }
        lookup(&targets)
            .map(Some)
            .ok_or_else(|| QueryError::new(QueryErrorKind::MissingReferenceIndex, referent.span()))
    }
}
