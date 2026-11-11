// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::catalog_row::{CatalogRows, ExcludedCatalogRow};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_inspection::FieldInspection;
use crate::health_diagnostic_observation::DiagnosticObservation as ExportDiagnostic;
pub(crate) use crate::record_derived_observation::DerivedFailure;
pub(crate) use crate::record_observation::SourceObservation;
pub(crate) use crate::record_source_status::{SourceDiagnosis, SourceDisposition};
use crate::selection::Kind;
use crate::template_issues::TemplateWarningExamples;

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
    pub(crate) template_warning_examples: TemplateWarningExamples,
    pub(crate) field_inspections: Vec<FieldInspection>,
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
        self.template_warning_examples
            .extend(other.template_warning_examples);
        self.field_inspections.extend(other.field_inspections);
        self.skipped_rows.extend(other.skipped_rows);
        self.excluded_catalog_rows
            .extend(other.excluded_catalog_rows);
    }
}

/// A contradiction in production accounting prevents publication.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
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
    /// Retained original source facts contradict their actual occurrence or value.
    #[error("conflicting original source datum")]
    ConflictingSourceDatum,
    /// A retained catalog subject has no matching original input binding.
    #[error("catalog health subject is not bound to its original input")]
    UnboundHealthSubject,
    /// Health capture was used after its immutable result was sealed.
    #[error("health capture is already sealed")]
    HealthCaptureClosed,
    /// A reserved original Loot occurrence was not completed.
    #[error("original Loot value {ordinal} was not completed")]
    UnfilledLootValue {
        /// The original owned-value preorder position.
        ordinal: u64,
    },
    /// A canonical Loot value or error variant cannot be retained by this reader.
    #[error("unsupported original Loot source value")]
    UnsupportedLootSourceValue,
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
    /// A reserved transformation occurrence has no completed original-node projection.
    #[error("an original transformation node was not completed")]
    UnfilledTransformationNode,
    /// Counter arithmetic exceeded the supported range.
    #[error("export health counter exceeds its supported range")]
    CounterOverflow,
}
