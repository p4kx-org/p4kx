// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, ArchiveWatch, CancelToken, SourceInventory};

use super::coverage::{compile_entry_counts, descriptor_inventory_complete, entry_counts_complete};
use super::system_roots::{self, RootChoice};
use super::{
    ContainerInstance, ContainerSystemRootInventory, DatasetAvailability, ExportEntryCounts,
    UnresolvedContainerLink,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::with_entry_capture;
use crate::export_ledger::ExportLedger;
use crate::starmap_containers::finish_route_coverage;
use crate::starmap_index::{Index, SystemRootCandidates};
use crate::starmap_parse::{self, DescriptorScope, Read};
use crate::starmap_walk::{PlacementRows, System, SystemRead};
use crate::{CatalogError, ExportAccountingError, StarmapTraversalBudget};

/// One row or original root completion event from the full container placement visit.
#[derive(Debug)]
#[non_exhaustive]
pub enum ContainerPlacementEvent<'visit> {
    /// The next resolved occurrence in the Normal writer's traversal.
    Instance(&'visit ContainerInstance),
    /// The next authored unresolved link in the Normal writer's traversal.
    UnresolvedLink(&'visit UnresolvedContainerLink),
    /// The completed attempts and sealed source inventory for the preceding root's rows.
    RootCompleted(&'visit ContainerPlacementRootVisit),
}

/// Observed row counts for the two placement tables.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct ContainerPlacementRowCounts {
    /// Resolved container occurrences.
    pub instances: usize,
    /// Authored unresolved links.
    pub unresolved_links: usize,
}

impl ContainerPlacementRowCounts {
    pub(crate) fn record_instance(&mut self) -> Result<(), CatalogError> {
        self.instances = add_count(self.instances, 1)?;
        Ok(())
    }

    pub(crate) fn record_unresolved_link(&mut self) -> Result<(), CatalogError> {
        self.unresolved_links = add_count(self.unresolved_links, 1)?;
        Ok(())
    }

    fn add(self, other: Self) -> Result<Self, CatalogError> {
        Ok(Self {
            instances: add_count(self.instances, other.instances)?,
            unresolved_links: add_count(self.unresolved_links, other.unresolved_links)?,
        })
    }
}

/// The full attempt to read one uniquely selected system root.
#[derive(Debug)]
#[non_exhaustive]
pub struct ContainerPlacementRootVisit {
    /// Original physical system root Entry index.
    pub root_entry_index: u32,
    /// Rows emitted for this root.
    pub rows: ContainerPlacementRowCounts,
    /// Observed root-relative descriptor and traversal coverage.
    pub coverage: ContainerPlacementRootCoverage,
    /// Original sources and uses retained for this root.
    pub source_inventory: SourceInventory,
}

/// Original coverage of both placement tables for one physical system.
#[derive(Debug)]
#[non_exhaustive]
pub struct ContainerPlacementRootCoverage {
    /// Whether the directory and descriptor survey establish the source scope.
    pub descriptor_inventory_complete: bool,
    /// Whether admitted descendant routes could be fully traversed.
    pub routes_complete: bool,
    /// Distinct physical descriptors whose reads were attempted within this root.
    pub attempted_descriptors: usize,
    /// Occurrences whose container body could not be read or parsed.
    pub unreadable_containers: usize,
    /// Occurrences with conclusive original lookup evidence for a missing target.
    pub verified_missing_containers: usize,
    /// Occurrences whose missing target could not be verified.
    pub unverified_missing_containers: usize,
    /// Malformed or stopped Child placement operations.
    pub malformed_children: usize,
    /// Physical and authored Entry outcomes within the selected scope.
    pub source_entries: ExportEntryCounts,
}

impl ContainerPlacementRootCoverage {
    /// Whether the root supplies conclusive coverage of both placement tables.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.descriptor_inventory_complete
            && self.routes_complete
            && self.unreadable_containers == 0
            && self.unverified_missing_containers == 0
            && self.malformed_children == 0
            && entry_counts_complete(&self.source_entries)
    }

    fn measure(
        directory_complete: bool,
        attempted_descriptors: usize,
        read: &SystemRead,
        mut ledger: ExportLedger,
    ) -> Result<Self, CatalogError> {
        let routes_complete = finish_route_coverage(std::slice::from_ref(read), &mut ledger);
        let descriptor_inventory_complete =
            directory_complete && descriptor_inventory_complete(&ledger);
        let unverified_missing_containers = read
            .missing
            .checked_sub(read.verified_missing)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let source_entries = compile_entry_counts(ledger)?;
        Ok(Self {
            descriptor_inventory_complete,
            routes_complete,
            attempted_descriptors,
            unreadable_containers: read.unreadable,
            verified_missing_containers: read.verified_missing,
            unverified_missing_containers,
            malformed_children: read.malformed,
            source_entries,
        })
    }
}

/// Scope and counts established by a full container placement visit.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ContainerPlacementVisitSummary {
    /// Original system-root directory choices and their source inventory.
    pub root_inventory: ContainerSystemRootInventory,
    /// Shared source-family availability for the two placement tables.
    pub availability: DatasetAvailability,
    /// Number of physical system roots that reported a completed attempt.
    pub visited_roots: usize,
    /// Rows emitted across all visited roots.
    pub rows: ContainerPlacementRowCounts,
    complete: bool,
}

impl ContainerPlacementVisitSummary {
    /// Whether root selection and traversal supply complete evidence for both tables.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}

/// Visits the unfiltered container placement tables in Normal traversal order.
/// Commit consumer-owned output after successful completion with the required coverage.
///
/// # Errors
/// Returns input, evidence, Archive mutation, visitor or cancellation errors.
pub fn visit_container_placements(
    archive: &Archive,
    cancel: Option<&CancelToken>,
    mut visit: impl FnMut(ContainerPlacementEvent<'_>) -> Result<(), CatalogError>,
) -> Result<ContainerPlacementVisitSummary, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    let index = Index::new(archive);
    let choices = system_roots::choose_roots(&index, cancel)?;
    let root_inventory = system_roots::capture_roots(archive, &index, &choices, cancel)?;
    let directory_complete = root_inventory.directory_complete();
    let availability = availability(&root_inventory);
    let mut complete = root_inventory.is_complete();
    let mut visited_roots = 0;
    let mut rows = ContainerPlacementRowCounts::default();
    for (choice, root) in choices.iter().zip(&root_inventory.roots) {
        checkpoint(cancel)?;
        let SystemRootCandidates::Unique(position) = &choice.candidates else {
            continue;
        };
        let entry = root
            .selected_entry()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let completed = visit_root(
            archive,
            &index,
            (choice, **position, entry.entry_index()),
            directory_complete,
            cancel,
            &mut visit,
        )?;
        rows = rows.add(completed.rows)?;
        visited_roots = add_count(visited_roots, 1)?;
        complete &= completed.coverage.is_complete();
        watch.check()?;
        visit(ContainerPlacementEvent::RootCompleted(&completed))?;
    }
    checkpoint(cancel)?;
    watch.check()?;
    Ok(ContainerPlacementVisitSummary {
        root_inventory,
        availability,
        visited_roots,
        rows,
        complete,
    })
}

fn visit_root(
    archive: &Archive,
    index: &Index<'_>,
    (choice, position, root_entry_index): (&RootChoice<'_>, usize, u32),
    directory_complete: bool,
    cancel: Option<&CancelToken>,
    visit: &mut impl FnMut(ContainerPlacementEvent<'_>) -> Result<(), CatalogError>,
) -> Result<ContainerPlacementRootVisit, CatalogError> {
    with_entry_capture(archive, |capture| {
        let Read { bodies, ledger, .. } = starmap_parse::read_all_scoped(
            archive,
            index,
            &[position],
            cancel,
            (None, DescriptorScope::ContainerRows),
            capture,
        )?;
        checkpoint(cancel)?;
        let attempted_descriptors = bodies.len();
        let read = System::run_with_output(
            index,
            &bodies,
            &choice.system,
            position,
            cancel,
            StarmapTraversalBudget::new(),
            (
                archive,
                capture,
                None,
                PlacementRows::ContainerVisitor(visit),
            ),
        )?;
        drop(bodies);
        checkpoint(cancel)?;
        let rows = read.visited_container_rows;
        let coverage = ContainerPlacementRootCoverage::measure(
            directory_complete,
            attempted_descriptors,
            &read,
            ledger,
        )?;
        drop(read);
        let source_inventory = capture.source_uses().finish_with(|| checkpoint(cancel))?;
        Ok(ContainerPlacementRootVisit {
            root_entry_index,
            rows,
            coverage,
            source_inventory,
        })
    })
}

fn availability(inventory: &ContainerSystemRootInventory) -> DatasetAvailability {
    if !inventory.roots.is_empty() {
        DatasetAvailability::Available
    } else if inventory.directory_complete() {
        DatasetAvailability::Absent
    } else {
        DatasetAvailability::Unavailable
    }
}

fn add_count(left: usize, right: usize) -> Result<usize, CatalogError> {
    Ok(left
        .checked_add(right)
        .ok_or(ExportAccountingError::CounterOverflow)?)
}
