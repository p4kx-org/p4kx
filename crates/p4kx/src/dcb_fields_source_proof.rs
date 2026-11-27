// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::field_statistics_report::{
    FieldStatisticsAddressedOrigins, FieldStatisticsReport, FieldStatisticsSelectionProof,
};
use p4kx_extraction::query::{QueryUnavailable, ReferenceCoverage, TraversalInput};
use p4kx_extraction::{FieldOrigin, SourceUseRef};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceProof<'report> {
    selection_mode: SelectionMode,
    reference_coverage: Option<ReportedCoverage>,
    selection: &'report FieldOrigin,
    selection_attempts: &'report [SourceUseRef],
    root_attempts: &'report [SourceUseRef],
    value_attempts: &'report [SourceUseRef],
    row_attempts: Vec<RowAttempts<'report>>,
    unresolved_ancestry_positions: &'report [usize],
    unavailable_traversals: Vec<UnavailableTraversal>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum SelectionMode {
    AllRecords,
    IncompleteHeaders,
    ObservedQuery,
    Unavailable,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum ReportedCoverage {
    Complete,
    Incomplete,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RowAttempts<'report> {
    row: usize,
    source_uses_refs: &'report [SourceUseRef],
}

impl<'report> SourceProof<'report> {
    pub(crate) fn of(
        report: &'report FieldStatisticsReport<'_>,
        displayed: &FieldStatisticsAddressedOrigins,
    ) -> Self {
        let (selection_mode, reference_coverage) = match report.selection_proof {
            FieldStatisticsSelectionProof::AllRecords => (SelectionMode::AllRecords, None),
            FieldStatisticsSelectionProof::IncompleteHeaders => {
                (SelectionMode::IncompleteHeaders, None)
            }
            FieldStatisticsSelectionProof::QueryHeaders { .. } => (
                SelectionMode::ObservedQuery,
                Some(ReportedCoverage::Complete),
            ),
            FieldStatisticsSelectionProof::QueryRuntime {
                reference_coverage, ..
            } => (
                SelectionMode::ObservedQuery,
                Some(if reference_coverage == ReferenceCoverage::Complete {
                    ReportedCoverage::Complete
                } else {
                    ReportedCoverage::Incomplete
                }),
            ),
            _ => (SelectionMode::Unavailable, None),
        };
        let row_attempts = displayed
            .ordinals
            .iter()
            .enumerate()
            .filter_map(|(row, &ordinal)| {
                let original = report.fields.get(ordinal)?;
                (!original.attempts.is_empty()).then_some(RowAttempts {
                    row,
                    source_uses_refs: &original.attempts,
                })
            })
            .collect();
        Self {
            selection_mode,
            reference_coverage,
            selection: &report.selection_origin,
            selection_attempts: &report.selection_attempts,
            root_attempts: &report.root_attempts,
            value_attempts: &report.counters.value_attempts,
            row_attempts,
            unresolved_ancestry_positions: &report.unresolved_ancestry_positions,
            unavailable_traversals: report
                .unavailable_traversals
                .iter()
                .map(UnavailableTraversal::of)
                .collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum UnavailableTraversal {
    Text {
        offset: u32,
    },
    Field {
        owner_start: Option<u64>,
        offset: u64,
        byte_count: u64,
        input: &'static str,
    },
    Instance {
        structure: u32,
        instance: u32,
    },
    Unavailable,
}

impl UnavailableTraversal {
    const fn of(gap: &QueryUnavailable) -> Self {
        match gap {
            QueryUnavailable::Text { offset, .. } => Self::Text { offset: *offset },
            QueryUnavailable::Field {
                owner_start,
                offset,
                byte_count,
                input,
                ..
            } => Self::Field {
                owner_start: *owner_start,
                offset: *offset,
                byte_count: *byte_count,
                input: match input {
                    TraversalInput::Value => "value",
                    TraversalInput::Pointer => "pointer",
                    TraversalInput::Reference => "reference",
                    TraversalInput::WeakLink => "weakLink",
                    TraversalInput::Array => "array",
                    _ => "unavailable",
                },
            },
            QueryUnavailable::Instance {
                structure,
                instance,
                ..
            } => Self::Instance {
                structure: *structure,
                instance: *instance,
            },
            _ => Self::Unavailable,
        }
    }
}
