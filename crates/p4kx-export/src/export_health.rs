// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::export_diagnostic::{ExportDiagnostic, FailedExportRecord};
use crate::export_reference_issue::ExportUnresolvedReference;
use crate::selection::Kind;

/// Whether a catalog is intended for normal import or explicit partial diagnosis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub enum CatalogPublicationMode {
    /// Publishes only a successfully read and projected catalog without source warnings.
    #[default]
    Normal,
    /// Explicitly publishes diagnostic data with its incomplete health attached.
    DiagnosticPartial,
}

/// Whether all required source reads and projections completed without a blocking diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub enum ExportHealthStatus {
    /// No source read, projection or source warning makes the result incomplete.
    Complete,
    /// At least one required operation failed or produced a source warning.
    Incomplete,
}

/// The result of one export stage, independently of Filter selection scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub enum ExportStageStatus {
    /// Every required operation of the stage completed.
    Complete,
    /// At least one required operation of the stage failed.
    Incomplete,
}

/// The primary selected source unit of a kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub enum ExportSourceMode {
    /// Records selected by the kind's own source rule.
    Records,
    /// Original Records selected as inputs to a derived kind.
    RecordInputs,
    /// Archive Entries rather than DataForge Records.
    ArchiveEntries,
}

/// Accounting of one declared source unit's selected inputs and dependencies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ExportSourceCounts {
    /// Distinct selected sources, including inputs of a derived kind.
    pub selected: usize,
    /// Selected sources whose canonical content was successfully read.
    pub read_successful: usize,
    /// Selected sources whose typed projection completed, including zero-row projections.
    pub projected: usize,
    /// Selected sources intentionally left out by a documented selection policy.
    pub intentionally_skipped: usize,
    /// Selected sources omitted because their canonical content could not be read.
    pub omitted_after_read_failure: usize,
    /// Selected sources omitted because their typed projection failed.
    pub omitted_after_projection_failure: usize,
    /// Distinct sources read as dependencies of this kind.
    pub dependencies: usize,
    /// Dependencies whose required read or projection failed.
    pub failed_dependencies: usize,
    /// Sources whose selection could not be established because metadata was unreadable.
    pub failed_selection_candidates: usize,
}

/// The exact read and projection health of one catalog kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct KindExportHealth {
    /// The catalog kind.
    pub kind: Kind,
    /// Whether the inputs are selected Records, Record inputs or Archive Entries.
    pub source_mode: ExportSourceMode,
    /// Whether this build completely enumerated the kind's required source inputs.
    pub source_inventory_complete: bool,
    /// Whether all materialized source fields were inspected within the declared budgets.
    pub field_inventory_complete: bool,
    /// Canonical source-read health, including required dependencies.
    pub read: ExportStageStatus,
    /// Typed and derived projection health.
    pub projection: ExportStageStatus,
    /// Counts in distinct source-Record units.
    pub source_records: ExportSourceCounts,
    /// Counts in distinct Archive Entry units, separately from source Records.
    pub source_entries: crate::export_entry_counts::ExportEntryCounts,
    /// Rows actually serialized for this kind.
    pub written_rows: usize,
    /// Authored rows intentionally skipped or removed as duplicates.
    pub intentionally_skipped_rows: usize,
    /// Failed derived operations whose original source row can remain present.
    pub failed_derived_projections: usize,
    /// Exact affected identities for import retention, without a sample limit.
    pub failed_records: Vec<FailedExportRecord>,
    /// Exact failed Archive Entry identities, without a sample limit.
    pub failed_entries: Vec<crate::export_entry_ledger::FailedExportEntry>,
    /// Exact unresolved source links reported for this kind.
    pub unresolved_reference_count: usize,
}

/// Machine-readable export health carried by the catalog itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ExportHealth {
    /// Whether every required read and projection completed without a source warning.
    pub status: ExportHealthStatus,
    /// Overall canonical source-read health.
    pub read: ExportStageStatus,
    /// Overall typed and derived projection health.
    pub projection: ExportStageStatus,
    /// The explicitly requested publication mode.
    pub publication_mode: CatalogPublicationMode,
    /// Distinct physical source Records across all kinds, counting overlap once.
    pub unique_selected_source_records: usize,
    /// Sum of per-kind selected Record counts, including shared sources in each affected kind.
    pub selected_kind_records: usize,
    /// Sum of the rows serialized for all catalog kinds.
    pub written_rows: usize,
    /// Health in the same order as the catalog kinds.
    pub kinds: Vec<KindExportHealth>,
    /// Exact failed identities across kinds and non-kind sources, without a sample limit.
    pub failed_records: Vec<FailedExportRecord>,
    /// Exact failed Archive Entry identities across kinds, without a sample limit.
    pub failed_entries: Vec<crate::export_entry_ledger::FailedExportEntry>,
    /// Exact unresolved source links, including declared target identities beyond the example limit.
    pub unresolved_references: Vec<ExportUnresolvedReference>,
    /// Deterministically ordered diagnostic counts and bounded examples.
    pub diagnostics: Vec<ExportDiagnostic>,
}

impl ExportHealth {
    /// Returns whether every required source read and projection completed.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        matches!(self.status, ExportHealthStatus::Complete)
    }

    /// Returns whether the requested mode permits publishing this result.
    #[must_use]
    pub const fn permits_publication(&self, mode: CatalogPublicationMode) -> bool {
        self.is_complete() || matches!(mode, CatalogPublicationMode::DiagnosticPartial)
    }

    /// Returns the health of one catalog kind.
    #[must_use]
    pub fn kind(&self, kind: Kind) -> Option<&KindExportHealth> {
        self.kinds.iter().find(|health| health.kind == kind)
    }
}
