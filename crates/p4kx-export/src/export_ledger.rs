// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, Record};

use crate::catalog_row::{CatalogRows, ExcludedCatalogRow};
use crate::commodity_backfill_input::{CommodityBackfillInput, CommodityInputLimit};
use crate::export_diagnostic::{ExportDiagnostic, ExportDiagnosticCode, ExportSourceRole};
use crate::export_field_inspection::FieldInspection;
use crate::selection::Kind;

/// The exact source disposition recorded where a reader or projection finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceDisposition {
    /// A canonical read and typed projection completed, including zero-row projections.
    Projected,
    /// A documented policy excluded the Record before a canonical read.
    SkippedBeforeRead,
    /// A documented policy excluded the Record after a successful canonical read.
    SkippedAfterRead,
    /// The canonical source could not be read.
    ReadFailed,
    /// The canonical source was read but its typed projection failed.
    ProjectionFailed,
}

impl SourceDisposition {
    #[must_use]
    pub(crate) const fn read_succeeded(self) -> bool {
        matches!(
            self,
            Self::Projected | Self::SkippedAfterRead | Self::ProjectionFailed
        )
    }

    #[must_use]
    pub(crate) const fn failed(self) -> bool {
        matches!(self, Self::ReadFailed | Self::ProjectionFailed)
    }
}

/// Available technical field metadata of a source failure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SourceDiagnosis {
    pub(crate) code: Option<ExportDiagnosticCode>,
    pub(crate) field: Option<String>,
    pub(crate) source_type: Option<String>,
    pub(crate) target_guid: Option<Guid>,
}

/// Root directory evidence that cannot establish a complete Archive inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ArchiveInventoryProblem {
    Recovered,
    RecoveryLimit,
    Uncertain,
}

impl ArchiveInventoryProblem {
    #[must_use]
    pub(crate) const fn warning_code(self) -> &'static str {
        match self {
            Self::Recovered => "recovered_archive_directory",
            Self::RecoveryLimit => "archive_recovery_limit",
            Self::Uncertain => "incomplete_archive_directory",
        }
    }
}

/// An immutable source observation captured before rows are aggregated or failures deduplicated.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub(crate) struct SourceObservation {
    pub(crate) kind: Option<Kind>,
    pub(crate) record_index: u32,
    pub(crate) guid: Guid,
    pub(crate) record_path: Option<String>,
    pub(crate) role: ExportSourceRole,
    pub(crate) disposition: SourceDisposition,
    pub(crate) diagnosis: SourceDiagnosis,
}

impl SourceObservation {
    /// Captures an actual source disposition without consulting emitted-row counts.
    pub(crate) fn new(
        kind: Kind,
        record: &Record<'_>,
        role: ExportSourceRole,
        disposition: SourceDisposition,
    ) -> Self {
        Self {
            kind: Some(kind),
            record_index: record.index(),
            guid: record.guid(),
            record_path: record.record_path().ok().map(str::to_owned),
            role,
            disposition,
            diagnosis: SourceDiagnosis::default(),
        }
    }

    /// Captures a non-kind source such as the global crafting or item-recovery configuration.
    pub(crate) fn catalog_source(
        record: &Record<'_>,
        role: ExportSourceRole,
        disposition: SourceDisposition,
    ) -> Self {
        Self {
            kind: None,
            record_index: record.index(),
            guid: record.guid(),
            record_path: record.record_path().ok().map(str::to_owned),
            role,
            disposition,
            diagnosis: SourceDiagnosis::default(),
        }
    }

    /// Associates available field diagnostics with this actual observation.
    pub(crate) fn diagnosed(mut self, diagnosis: SourceDiagnosis) -> Self {
        self.diagnosis = diagnosis;
        self
    }
}

/// One failed derived operation whose original selected source row may remain present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DerivedFailure {
    pub(crate) affected_kind: Kind,
    pub(crate) output_kind: Kind,
    pub(crate) record_index: u32,
    pub(crate) role: ExportSourceRole,
    pub(crate) guid: Guid,
    pub(crate) record_path: Option<String>,
    pub(crate) diagnosis: SourceDiagnosis,
}

/// Production observations collected independently of output rows and compatibility summaries.
#[derive(Debug, Clone, Default)]
pub(crate) struct ExportLedger {
    pub(crate) archive_inventory_problem: Option<ArchiveInventoryProblem>,
    pub(crate) entries: Vec<crate::export_entry_ledger::ArchiveEntryObservation>,
    pub(crate) inventories: Vec<Kind>,
    pub(crate) incomplete_inventories: Vec<Kind>,
    pub(crate) sources: Vec<SourceObservation>,
    pub(crate) derived_failures: Vec<DerivedFailure>,
    pub(crate) field_diagnostics: Vec<ExportDiagnostic>,
    pub(crate) field_inspections: Vec<FieldInspection>,
    pub(crate) commodity_backfill_inputs: Vec<CommodityBackfillInput>,
    pub(crate) commodity_input_limits: Vec<CommodityInputLimit>,
    pub(crate) skipped_rows: Vec<(Kind, usize)>,
    pub(crate) catalog_rows: Option<CatalogRows>,
    pub(crate) conflicting_catalog_rows: bool,
    pub(crate) excluded_catalog_rows: Vec<ExcludedCatalogRow>,
}

impl ExportLedger {
    /// Records an actual Archive Entry read or projection in Entry units.
    pub(crate) fn entry(
        &mut self,
        observation: crate::export_entry_ledger::ArchiveEntryObservation,
    ) {
        self.entries.push(observation);
    }
    /// Records completion of an actual source-selection or derived-input enumeration.
    pub(crate) fn inventory(&mut self, kind: Kind) {
        self.inventories.push(kind);
    }

    /// Records an attempted source enumeration whose required inputs could not all be enumerated.
    pub(crate) fn incomplete_inventory(&mut self, kind: Kind) {
        self.inventory(kind);
        self.incomplete_inventories.push(kind);
    }
    /// Records one reader's source disposition.
    pub(crate) fn observe(&mut self, observation: SourceObservation) {
        self.sources.push(observation);
    }

    /// Records a retained-row derived failure separately from source omission.
    pub(crate) fn derived_failure(&mut self, failure: DerivedFailure) {
        self.derived_failures.push(failure);
    }

    /// Adds represented field states without inventing source-Record outcomes.
    pub(crate) fn field_diagnostic(&mut self, diagnostic: ExportDiagnostic) {
        self.field_diagnostics.push(diagnostic);
    }

    /// Reassigns an already inspected immutable source when its projection role is established.
    pub(crate) fn reassign_inspection_role(
        &mut self,
        kind: Option<Kind>,
        record_index: u32,
        from: ExportSourceRole,
        to: ExportSourceRole,
    ) {
        let matching = self.field_inspections.iter_mut().filter(|inspection| {
            inspection.kind == kind
                && inspection.record_index == record_index
                && inspection.role == from
        });
        for inspection in matching {
            inspection.contextualize(kind, to);
        }
    }

    /// Records row-unit skips, independently of source-Record skips.
    pub(crate) fn skipped_rows(&mut self, kind: Kind, count: usize) {
        if count > 0 {
            self.skipped_rows.push((kind, count));
        }
    }

    /// Combines independent production readers without inferring dispositions from their rows.
    pub(crate) fn merge(&mut self, other: Self) {
        self.conflicting_catalog_rows |= other.conflicting_catalog_rows
            || (self.catalog_rows.is_some() && other.catalog_rows.is_some());
        if self.catalog_rows.is_none() {
            self.catalog_rows = other.catalog_rows;
        }
        self.archive_inventory_problem = self
            .archive_inventory_problem
            .max(other.archive_inventory_problem);
        self.entries.extend(other.entries);
        self.inventories.extend(other.inventories);
        self.incomplete_inventories
            .extend(other.incomplete_inventories);
        self.sources.extend(other.sources);
        self.derived_failures.extend(other.derived_failures);
        self.field_diagnostics.extend(other.field_diagnostics);
        self.field_inspections.extend(other.field_inspections);
        self.commodity_backfill_inputs
            .extend(other.commodity_backfill_inputs);
        self.commodity_input_limits
            .extend(other.commodity_input_limits);
        self.skipped_rows.extend(other.skipped_rows);
        self.excluded_catalog_rows
            .extend(other.excluded_catalog_rows);
    }
}

/// A contradiction in production accounting prevents publication.
#[derive(Debug, thiserror::Error)]
pub enum ExportAccountingError {
    /// Reference resolution has no final catalog-row evidence.
    #[error("no final catalog-row evidence for source Reference resolution")]
    MissingCatalogRows,
    /// Producer evidence contains multiple final row snapshots for one catalog.
    #[error("conflicting final catalog-row evidence")]
    ConflictingCatalogRows,
    /// An emitted target row cannot be attributed to a successful physical source.
    #[error("no successful physical source for an emitted {kind} row")]
    MissingCatalogRowSource {
        /// The affected catalog target kind.
        kind: Kind,
    },
    /// A canonical Reference identifies no physical Record in its original `DataForge`.
    #[error("no physical Record at canonical Reference target {index}")]
    MissingPhysicalReferenceTarget {
        /// The physical target position in the source Build.
        index: u32,
    },
    /// A worker unwound while changing protected production evidence.
    #[error("export health evidence was poisoned by an interrupted worker")]
    PoisonedEvidence,
    /// A kind has no completed source-selection or derived-input enumeration.
    #[error("no completed source inventory for {kind}")]
    MissingInventory {
        /// The affected kind.
        kind: Kind,
    },
    /// A compatibility-summary failure has no production diagnostic.
    #[error("a source failure has no production health observation")]
    UnobservedFailure,
    /// A successful canonical source read has no bounded field inspection.
    #[error("a canonical source read has no field inspection")]
    MissingFieldInspection,
    /// Repeated inspection of the same immutable source produced different results.
    #[error("conflicting field-inspection results for one source Record")]
    ConflictingFieldInspection,
    /// A source path cannot be formatted.
    #[error("cannot format a source field path")]
    FieldPath(#[source] std::fmt::Error),
    /// One source was assigned incompatible dispositions for the same role and kind.
    #[error("conflicting source-Record dispositions")]
    ConflictingDisposition {
        /// The affected kind.
        kind: Option<Kind>,
    },
    /// A source disposition cannot be reconciled in its declared units.
    #[error("source-Record counters do not reconcile for {kind}")]
    Unreconciled {
        /// The affected kind.
        kind: Kind,
    },
    /// Tag projection trace positions do not identify their original selected parent.
    #[error("inconsistent original tag-path trace")]
    ConflictingTagTrace,
    /// Counter arithmetic exceeded the supported range.
    #[error("export health counter exceeds its supported range")]
    CounterOverflow,
}
