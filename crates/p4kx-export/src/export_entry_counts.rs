// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::export_health::ExportSourceCounts;

/// Actual required Entry operations, independently of retained partial output rows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ExportEntryCounts {
    /// Distinct Entry inputs selected by the actual reader.
    pub selected: usize,
    /// Inputs whose required read completed; this does not assert full-file integrity validation.
    pub read_successful: usize,
    /// Inputs whose typed projection completed, including zero-row projections.
    pub projected: usize,
    /// Inputs intentionally excluded by a documented source policy.
    pub intentionally_skipped: usize,
    /// Inputs whose required read failed, regardless of retained partial output.
    pub failed_required_reads: usize,
    /// Inputs whose typed projection failed, regardless of retained partial output.
    pub failed_projections: usize,
    /// Distinct Entry dependencies consumed by this kind.
    pub dependencies: usize,
    /// Entry dependencies whose required read or projection failed.
    pub failed_dependencies: usize,
    /// Entries whose source metadata prevented a definitive selection decision.
    pub failed_selection_candidates: usize,
}

impl From<ExportSourceCounts> for ExportEntryCounts {
    fn from(counts: ExportSourceCounts) -> Self {
        Self {
            selected: counts.selected,
            read_successful: counts.read_successful,
            projected: counts.projected,
            intentionally_skipped: counts.intentionally_skipped,
            failed_required_reads: counts.omitted_after_read_failure,
            failed_projections: counts.omitted_after_projection_failure,
            dependencies: counts.dependencies,
            failed_dependencies: counts.failed_dependencies,
            failed_selection_candidates: counts.failed_selection_candidates,
        }
    }
}
