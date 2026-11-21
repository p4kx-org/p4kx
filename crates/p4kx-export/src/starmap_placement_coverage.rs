// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use crate::export_entry_counts::ExportEntryCounts;
use crate::export_health_entries;
use crate::export_ledger::ExportLedger;
use crate::starmap_containers::finish_route_coverage;
use crate::starmap_walk::SystemRead;
use crate::{CatalogError, Kind};

/// Outcome of selecting the requested physical Entry as the canonical system root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum ContainerRootStatus {
    /// The requested root is the unique canonical Entry.
    Selected,
    /// A complete Archive directory supplies no canonical root at the requested index.
    Absent,
    /// More than one physical Entry matches the canonical root path.
    Ambiguous,
    /// The directory cannot establish that the requested root is missing.
    Unavailable,
}

/// Observed coverage of one system's container descriptor reads and placement traversal.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ContainerPlacementCoverage {
    /// Outcome of selecting the requested root.
    pub root: ContainerRootStatus,
    /// Whether original descriptor enumeration could establish its full selected inventory.
    pub source_inventory_complete: bool,
    /// Whether every admitted descendant route was traversable within the requested budget.
    pub routes_complete: bool,
    /// True when the caller's Child occurrence budget stopped a route.
    pub placement_budget_exhausted: bool,
    /// Distinct physical descriptor body reads attempted from the selected root.
    pub attempted_descriptors: usize,
    /// Occurrences whose container body was unavailable or unparseable.
    pub unreadable_containers: usize,
    /// Occurrences whose declared target was missing.
    pub missing_containers: usize,
    /// Malformed or stopped Child placement operations.
    pub malformed_children: usize,
    /// Physical and authored Entry outcomes counted by the canonical Entry health compiler.
    pub source_entries: ExportEntryCounts,
}

impl ContainerPlacementCoverage {
    /// Whether the query can assert the absence of further matching occurrences.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        matches!(
            self.root,
            ContainerRootStatus::Selected | ContainerRootStatus::Absent
        ) && self.source_inventory_complete
            && self.routes_complete
            && self.unreadable_containers == 0
            && self.missing_containers == 0
            && self.malformed_children == 0
            && self.source_entries.failed_required_reads == 0
            && self.source_entries.failed_projections == 0
            && self.source_entries.failed_dependencies == 0
            && self.source_entries.failed_selection_candidates == 0
    }
}

impl ContainerPlacementCoverage {
    pub(super) fn measure(
        root: ContainerRootStatus,
        directory_complete: bool,
        attempted_descriptors: usize,
        read: &SystemRead,
        mut ledger: ExportLedger,
    ) -> Result<Self, CatalogError> {
        let routes_complete = matches!(
            root,
            ContainerRootStatus::Selected | ContainerRootStatus::Absent
        ) && finish_route_coverage(std::slice::from_ref(read), &mut ledger)
            && read.unreadable == 0
            && read.missing == 0
            && read.malformed == 0;
        let source_inventory_complete = directory_complete
            && !ledger
                .incomplete_inventories
                .contains(&Kind::StarmapObjects)
            && !matches!(
                root,
                ContainerRootStatus::Ambiguous | ContainerRootStatus::Unavailable
            );
        let entries = export_health_entries::compile(ledger.entries)?;
        let source_entries = export_health_entries::counts(Kind::StarmapObjects, &entries.sources)?;
        Ok(Self {
            root,
            source_inventory_complete,
            routes_complete,
            placement_budget_exhausted: read.placement_budget_exhausted,
            attempted_descriptors,
            unreadable_containers: read.unreadable,
            missing_containers: read.missing,
            malformed_children: read.malformed,
            source_entries,
        })
    }
}
