// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Collects original sources for admitted DataForge field statistics.

use std::sync::Mutex;

use p4kx_dataforge::query::{
    Query, QueryError, QueryUnavailable, ReferenceCoverage, SelectionError,
};
use p4kx_dataforge::{
    DataForge, FieldStatistics, FieldStatisticsError, FieldStatisticsReduction,
    FieldStatisticsSourceScope,
};
use thiserror::Error;

use crate::{
    FieldOrigin, InputScope, SourceEvidenceError, SourceInput, SourceInventory, SourceScope,
    SourceUseRef, SourceUses,
};

#[path = "field_statistics_addresses.rs"]
mod addresses;
#[path = "field_statistics_report_counts.rs"]
mod counts;
#[path = "field_statistics_instance_bounds.rs"]
mod instance_bounds;
#[path = "field_statistics_report_rows.rs"]
mod rows;
#[path = "field_statistics_report_scan.rs"]
mod scan;

pub use self::addresses::{FieldStatisticsAddressError, FieldStatisticsAddressedOrigins};
pub use self::counts::FieldStatisticsCounterProof;
pub use self::rows::FieldStatisticsRowProof;

use self::scan::{Capture, Observe};

/// The remaining original input selection proof for the field statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldStatisticsSelectionProof {
    /// The complete physical Record path census has no observed unresolved names or paths.
    AllRecords,
    /// The physical path census found incomplete original Record headers.
    IncompleteHeaders,
    /// The Query admits Records through original paths or types only.
    #[non_exhaustive]
    QueryHeaders {
        /// Count of physical Records available to the original candidate census.
        record_count: usize,
    },
    /// Runtime decisions, predicate layouts and type tests are bound within the observed Query.
    #[non_exhaustive]
    QueryRuntime {
        /// The original Reference-index coverage proved by the Query compiler.
        reference_coverage: ReferenceCoverage,
    },
}

/// Failure to obtain the statistics or bind their actual source operands.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FieldStatisticsReportError {
    /// The original selection could not be compiled.
    #[error(transparent)]
    Compile(#[from] QueryError),
    /// The original runtime selection could not decide a predicate.
    #[error(transparent)]
    Select(#[from] SelectionError),
    /// The admitted fields could not be scanned.
    #[error(transparent)]
    Scan(#[from] FieldStatisticsError),
    /// The physical-source inventory could not register the retained uses.
    #[error(transparent)]
    Evidence(#[from] SourceEvidenceError),
    /// A reported field has no matching original physical event.
    #[error("field statistics lost the original field evidence")]
    UnboundField,
    /// The observed visits do not account for the canonical statistics counters.
    #[error("field statistics lost the original counter evidence")]
    UnboundCounters,
    /// The original non-cancellable selection did not produce its Record set.
    #[error("field statistics lost the original selection")]
    UnboundSelection,
}

/// Statistics and original source evidence from their canonical scanner.
#[derive(Debug)]
#[non_exhaustive]
pub struct FieldStatisticsReport<'source> {
    /// The unchanged statistics and their actual selected comparison operands.
    pub reduction: FieldStatisticsReduction<'source>,
    /// How completely the original Record membership was determined.
    pub selection_proof: FieldStatisticsSelectionProof,
    /// Shared original Record selection and admitted root evidence for every field.
    pub selection_origin: FieldOrigin,
    /// Recorded source attempts that could not contribute to selection.
    pub selection_attempts: Vec<SourceUseRef>,
    /// Unresolved Struct positions from original selection or predicate type tests.
    pub unresolved_ancestry_positions: Vec<usize>,
    /// Original attempted traversal positions without readable operand bytes.
    pub unavailable_traversals: Vec<QueryUnavailable>,
    /// Failed original instance or layout reads within the admitted roots.
    pub root_attempts: Vec<SourceUseRef>,
    /// The proof of each retained field in its original statistical output order.
    pub fields: Vec<FieldStatisticsRowProof>,
    /// Original source uses for the reported summary counters.
    pub counters: FieldStatisticsCounterProof,
    /// The one physical source inventory shared by selection and every retained metric.
    pub inventory: SourceInventory,
}

impl<'source> FieldStatisticsReport<'source> {
    /// Walks all canonical Record paths with their original census and value operands.
    ///
    /// # Errors
    ///
    /// Returns the original scan or source evidence error.
    pub fn all(
        forge: &'source DataForge<'source>,
        source: SourceInput<'_>,
    ) -> Result<Self, FieldStatisticsReportError> {
        observed(forge, source, |scope, capture| {
            let reduction = FieldStatistics::of_all_with_reduction(
                forge,
                |entry| capture.path(scope, &entry),
                |event| capture.field(scope, &event),
            )?;
            let selection = match reduction.source_scope() {
                FieldStatisticsSourceScope::AllRecordPathsObserved {
                    unreadable_paths: 0,
                    unreadable_names: 0,
                    ..
                } => FieldStatisticsSelectionProof::AllRecords,
                _ => FieldStatisticsSelectionProof::IncompleteHeaders,
            };
            Ok((reduction, selection))
        })
    }

    /// Walks the query's exact admitted Records with original runtime predicate evidence.
    ///
    /// # Errors
    ///
    /// Returns the original Query, scan or source evidence error.
    pub fn query(
        forge: &'source DataForge<'source>,
        source: SourceInput<'_>,
        query: &Query,
    ) -> Result<Self, FieldStatisticsReportError> {
        observed(forge, source, |scope, capture| {
            let (compiled, reference_coverage) = query.compile_observing_predicate_layouts(
                forge,
                |input| capture.query_type(scope, &input),
                |input| capture.query_layout(scope, &input),
                |input| capture.query_source(scope, input),
            )?;
            let records = compiled
                .run_observing_records(
                    || true,
                    |input| capture.query_source(scope, input),
                    |entry| capture.query_record(scope, &entry),
                )?
                .ok_or(FieldStatisticsReportError::UnboundSelection)?;
            let reduction =
                FieldStatistics::of_records_with_reduction(forge, records.indexes(), |event| {
                    capture.field(scope, &event);
                })?;
            let selection = if compiled.has_header_only_selection() {
                FieldStatisticsSelectionProof::QueryHeaders {
                    record_count: forge.record_count(),
                }
            } else {
                FieldStatisticsSelectionProof::QueryRuntime { reference_coverage }
            };
            Ok((reduction, selection))
        })
    }
}

fn observed<'source>(
    forge: &DataForge<'_>,
    source: SourceInput<'_>,
    walk: impl FnOnce(
        &SourceScope,
        &Mutex<Capture>,
    ) -> Result<
        (
            FieldStatisticsReduction<'source>,
            FieldStatisticsSelectionProof,
        ),
        FieldStatisticsReportError,
    >,
) -> Result<FieldStatisticsReport<'source>, FieldStatisticsReportError> {
    let sources = SourceUses::with_field_statistics_coordinates();
    let scope = sources.observe(source, InputScope::Payload)?;
    let capture = Mutex::new(Capture::default());
    let (reduction, selection_proof) = walk(&scope, &capture)?;
    let capture = capture
        .into_inner()
        .map_err(|_| SourceEvidenceError::PoisonedEvidence)?;
    let complete = capture.selection_complete(selection_proof);
    let capture = capture.finish(&sources, &scope, forge, &reduction, complete)?;
    Ok(FieldStatisticsReport {
        reduction,
        selection_proof,
        selection_origin: capture.selection_origin,
        selection_attempts: capture.selection_attempts,
        unresolved_ancestry_positions: capture.unresolved_ancestry_positions,
        unavailable_traversals: capture.unavailable_traversals,
        root_attempts: capture.root_attempts,
        fields: capture.fields,
        counters: capture.counters,
        inventory: sources.finish_with(|| Ok::<_, SourceEvidenceError>(()))?,
    })
}

#[cfg(test)]
#[path = "field_statistics_addresses_tests.rs"]
mod addresses_tests;
#[cfg(test)]
#[path = "field_statistics_ancestry_tests.rs"]
mod ancestry_tests;
#[cfg(test)]
#[path = "field_statistics_array_bounds_tests.rs"]
mod array_bounds_tests;
#[cfg(test)]
#[path = "field_statistics_header_tests.rs"]
mod header_tests;
#[cfg(test)]
#[path = "field_statistics_instance_target_tests.rs"]
mod instance_target_tests;
#[cfg(test)]
#[path = "field_statistics_instance_tests.rs"]
mod instance_tests;
#[cfg(test)]
#[path = "field_statistics_row_scalar_tests.rs"]
mod row_scalar_tests;
#[cfg(test)]
#[path = "field_statistics_selection_tests.rs"]
mod selection_tests;
#[cfg(test)]
#[path = "field_statistics_report_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "field_statistics_text_admission_tests.rs"]
mod text_admission_tests;
#[cfg(test)]
#[path = "field_statistics_text_tests.rs"]
mod text_tests;
#[cfg(test)]
#[path = "field_statistics_traversal_tests.rs"]
mod traversal_tests;
