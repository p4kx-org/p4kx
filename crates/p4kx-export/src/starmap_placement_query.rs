// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to canonical container occurrences and unresolved links.

use std::sync::Arc;

use p4kx_extraction::{Archive, ArchiveWatch, Build, CancelToken, SourceInventory};
use serde::Serialize;

use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::{EntryCapture, with_entry_capture};
use crate::export_archive_health;
use crate::export_health_entries;
use crate::export_ledger::ExportLedger;
use crate::starmap_containers::{finish_route_coverage, unique_system_roots};
use crate::starmap_index::{self, Index};
use crate::starmap_parse::{self, DescriptorScope, Read};
use crate::starmap_walk::{PlacementRows, SelectedContainerRows, System, SystemRead};
use crate::{CatalogError, Kind, StarmapTraversalBudget};

#[path = "starmap_definition_query.rs"]
mod definitions;

pub use crate::catalog_read::original::{PhysicalEntryRef, SourcedXmlAttribute};
pub use crate::catalog_table::DatasetAvailability;
pub use crate::export_entry_counts::ExportEntryCounts;
pub use crate::location_physics::{
    LocationPhysicsDeclaration, LocationPhysicsDescriptorSelection,
    LocationPhysicsDescriptorSnapshot, LocationPhysicsDescriptorStatus, LocationPhysicsFields,
    LocationPhysicsGrid, LocationPhysicsGridControls, LocationVoxelTarget,
    read_location_physics_descriptor,
};
pub use crate::starmap_containers::instance::{ContainerInstance, ContainerInstanceKey};
pub use crate::starmap_containers::unresolved::{
    ContainerTargetReadStatus, ContainerTargetStatus, UnresolvedChild, UnresolvedContainerLink,
};
pub use definitions::{
    DisplayFields, ObjectNavigation, ObjectText, RadarProperties, RadarRoomParams,
    StarmapAmenityDefinition, StarmapAssetManager, StarmapAsteroidRing, StarmapDefinitionFamily,
    StarmapDefinitionRows, StarmapDefinitionSelection, StarmapDefinitionSnapshot, StarmapLocation,
    StarmapObjectDefinition, StarmapObjectLinks, StarmapObjectOwned, StarmapPresetDefinition,
    StarmapQuantumTravel, StarmapTypeDefinition, VisibilityFields, with_starmap_definitions,
    with_starmap_definitions_from,
};

/// A canonical physical system root or a Child route within it.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum ContainerPlacementSelection<'route> {
    /// Every occurrence reached from one canonical original system root.
    SystemRoot(u32),
    /// All occurrences sharing the complete ordered Child ordinal sequence.
    Occurrence {
        /// Original physical system root Entry index.
        root_entry_index: u32,
        /// Document-local Child XML ordinals in root-to-leaf order.
        child_node_indices: &'route [u32],
    },
    /// One physical occurrence identified by its Child ordinals and external document steps.
    ExactOccurrence {
        /// Original physical system root Entry index.
        root_entry_index: u32,
        /// Document-local Child XML ordinals in root-to-leaf order.
        child_node_indices: &'route [u32],
        /// Sorted zero-based positions at which traversal entered an external document.
        external_child_step_indices: &'route [usize],
    },
}

impl ContainerPlacementSelection<'_> {
    fn selected_rows(self) -> (u32, SelectedContainerRows) {
        match self {
            Self::SystemRoot(root) => (root, SelectedContainerRows::new(None, None)),
            Self::Occurrence {
                root_entry_index,
                child_node_indices,
            } => (
                root_entry_index,
                SelectedContainerRows::new(Some(child_node_indices.to_vec()), None),
            ),
            Self::ExactOccurrence {
                root_entry_index,
                child_node_indices,
                external_child_step_indices,
            } => (
                root_entry_index,
                SelectedContainerRows::new(
                    Some(child_node_indices.to_vec()),
                    Some(external_child_step_indices.to_vec()),
                ),
            ),
        }
    }
}

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
    fn measure(
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

/// Canonical container tables for one physically selected system and optional occurrence route.
#[derive(Debug)]
#[non_exhaustive]
pub struct ContainerPlacementSnapshot {
    /// Build metadata present in the watched Archive.
    pub build: Option<Build>,
    /// Requested physical system root Entry index.
    pub root_entry_index: u32,
    /// Requested Child ordinals when the query selected an occurrence route.
    pub child_node_indices: Option<Vec<u32>>,
    /// Requested external document step positions for an exact occurrence selection.
    pub external_child_step_indices: Option<Vec<usize>>,
    /// Resolved container rows in original traversal order.
    pub container_instances: Vec<ContainerInstance>,
    /// Unresolved external Child rows in original traversal order.
    pub unresolved_container_links: Vec<UnresolvedContainerLink>,
    /// Presence of location grids within the admitted and selected source scope.
    pub location_physics_availability: DatasetAvailability,
    /// Physical descriptors joined to exact route rows by their container reference.
    pub location_physics_grids: Vec<Arc<LocationPhysicsGrid>>,
    /// Measured coverage of the selected descriptor graph and traversal.
    pub coverage: ContainerPlacementCoverage,
    /// Original sources and uses for the selected read and returned rows.
    pub source_inventory: SourceInventory,
}

/// Reads canonical container rows and unresolved links from one original system root.
/// The queried occurrence selects emitted rows. The walker checks the reachable system graph.
///
/// # Errors
/// Returns an error for cancellation, Archive changes or contradictory source evidence.
pub fn read_container_placements(
    archive: &Archive,
    selection: ContainerPlacementSelection<'_>,
    budget: StarmapTraversalBudget,
    cancel: Option<&CancelToken>,
) -> Result<ContainerPlacementSnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    let (root_entry_index, mut rows) = selection.selected_rows();
    with_entry_capture(archive, |capture| {
        let coverage = read_selected(
            archive,
            root_entry_index,
            budget,
            cancel,
            capture,
            &mut rows,
        )?;
        checkpoint(cancel)?;
        let source_inventory = capture.source_uses().freeze()?;
        watch.check()?;
        let SelectedContainerRows {
            child_node_indices,
            external_child_step_indices,
            instances,
            unresolved,
            location_physics,
        } = rows;
        let complete = coverage.is_complete() && !location_physics.unavailable;
        let grids = location_physics.into_rows();
        Ok(ContainerPlacementSnapshot {
            build: archive.build().cloned(),
            root_entry_index,
            child_node_indices,
            external_child_step_indices,
            container_instances: instances,
            unresolved_container_links: unresolved,
            location_physics_availability: crate::location_physics::stage::availability(
                complete,
                !grids.is_empty(),
            ),
            location_physics_grids: grids,
            coverage,
            source_inventory,
        })
    })
}

fn read_selected(
    archive: &Archive,
    root_entry_index: u32,
    budget: StarmapTraversalBudget,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
    rows: &mut SelectedContainerRows,
) -> Result<ContainerPlacementCoverage, CatalogError> {
    let index = Index::new(archive);
    let directory_complete = export_archive_health::inventory(archive).is_none();
    let mut ledger = ExportLedger::default();
    let _directory = capture
        .source_uses()
        .observe_root_directory_paths(archive)?;
    let Some(root_candidate) = candidate(&index, root_entry_index) else {
        let root = if directory_complete {
            ContainerRootStatus::Absent
        } else {
            ContainerRootStatus::Unavailable
        };
        return ContainerPlacementCoverage::measure(
            root,
            directory_complete,
            0,
            &SystemRead::default(),
            ledger,
        );
    };
    let admitted =
        unique_system_roots(archive, &index, capture, vec![root_candidate], &mut ledger)?;
    let [(name, position)] = admitted.as_slice() else {
        return ContainerPlacementCoverage::measure(
            ContainerRootStatus::Ambiguous,
            directory_complete,
            0,
            &SystemRead::default(),
            ledger,
        );
    };
    let RootRead {
        attempted_descriptors,
        mut system,
        observations,
    } = read_root(
        archive,
        &index,
        capture,
        cancel,
        budget,
        (name, *position),
        rows,
    )?;
    ledger.merge(observations);
    rows.location_physics = std::mem::take(&mut system.location_physics);
    ContainerPlacementCoverage::measure(
        ContainerRootStatus::Selected,
        directory_complete,
        attempted_descriptors,
        &system,
        ledger,
    )
}

struct RootRead {
    attempted_descriptors: usize,
    system: SystemRead,
    observations: ExportLedger,
}

fn read_root(
    archive: &Archive,
    index: &Index<'_>,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    budget: StarmapTraversalBudget,
    (name, position): (&Arc<str>, usize),
    rows: &mut SelectedContainerRows,
) -> Result<RootRead, CatalogError> {
    let Read {
        bodies,
        ledger: observations,
        ..
    } = starmap_parse::read_all_scoped(
        archive,
        index,
        &[position],
        cancel,
        (budget.source_children, DescriptorScope::ContainerPlacements),
        capture,
    )?;
    checkpoint(cancel)?;
    let system = System::run_with_output(
        index,
        &bodies,
        name,
        position,
        cancel,
        budget,
        (
            archive,
            capture,
            None,
            PlacementRows::SelectedContainers(rows),
        ),
    )?;
    Ok(RootRead {
        attempted_descriptors: bodies.len(),
        system,
        observations,
    })
}

fn candidate(index: &Index<'_>, requested: u32) -> Option<(Arc<str>, usize)> {
    let requested = usize::try_from(requested).ok()?;
    let entry = index
        .entries
        .iter()
        .find(|entry| entry.index() == requested)?;
    let requested_path = starmap_index::key(entry.path());
    index.systems().into_iter().find(|(_, position)| {
        index
            .entries
            .get(*position)
            .is_some_and(|candidate| starmap_index::key(candidate.path()) == requested_path)
    })
}
