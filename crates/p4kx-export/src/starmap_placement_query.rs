// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to canonical container occurrences and unresolved links.

use std::sync::Arc;

use p4kx_extraction::{Archive, ArchiveWatch, Build, CancelToken, ProvenanceMode, SourceInventory};

use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::{EntryCapture, with_entry_capture_mode};
use crate::export_archive_health;
use crate::export_ledger::ExportLedger;
use crate::starmap_containers::unique_system_roots;
use crate::starmap_index::{self, Index};
use crate::starmap_parse::{self, DescriptorScope, Read};
use crate::starmap_walk::{
    PlacementRows, PlacementTable, SelectedPlacementRows, System, SystemRead,
};
use crate::{CatalogError, StarmapTraversalBudget};

#[path = "starmap_definition_query.rs"]
mod definitions;

#[path = "starmap_placement_coverage.rs"]
mod coverage;

#[path = "starmap_location_query.rs"]
mod locations;

#[path = "starmap_jump_inventory_query.rs"]
mod jump_inventory;

#[path = "starmap_object_placement_query.rs"]
mod object_placements;

#[path = "starmap_system_root_inventory.rs"]
mod system_roots;

#[path = "starmap_container_placement_visit.rs"]
mod container_visit;

pub use crate::catalog_projections::JumpConnection;
pub use crate::catalog_read::original::{PhysicalEntryRef, SourcedXmlAttribute};
pub use crate::catalog_table::DatasetAvailability;
pub use crate::catalog_v3::starmap_placement::StarmapPlacement;
pub use crate::export_entry_counts::ExportEntryCounts;
pub use crate::guid_text::CatalogGuid;
pub use crate::location_physics::{
    LocationPhysicsDeclaration, LocationPhysicsDescriptorSelection,
    LocationPhysicsDescriptorSnapshot, LocationPhysicsDescriptorStatus, LocationPhysicsFields,
    LocationPhysicsGrid, LocationPhysicsGridControls, LocationVoxelTarget,
    read_location_physics_descriptor, read_location_physics_descriptor_with_provenance_mode,
};
pub use crate::starmap_containers::instance::{ContainerInstance, ContainerInstanceKey};
pub use crate::starmap_containers::jump_endpoint::JumpEndpoint;
pub use crate::starmap_containers::unresolved::{
    ContainerTargetReadStatus, ContainerTargetStatus, UnresolvedChild, UnresolvedContainerLink,
};
pub use coverage::{ContainerPlacementCoverage, ContainerRootStatus};
pub use definitions::{
    DisplayFields, ObjectNavigation, ObjectText, RadarProperties, RadarRoomParams,
    StarmapAmenityDefinition, StarmapAssetManager, StarmapAsteroidRing, StarmapDefinitionFamily,
    StarmapDefinitionRows, StarmapDefinitionSelection, StarmapDefinitionSnapshot, StarmapLocation,
    StarmapObjectDefinition, StarmapObjectLinks, StarmapObjectOwned, StarmapPresetDefinition,
    StarmapQuantumTravel, StarmapTypeDefinition, VisibilityFields, with_starmap_definitions,
    with_starmap_definitions_from,
};

pub use jump_inventory::{
    JumpConnectionInventory, JumpInventoryCoverage, JumpInventorySnapshot, read_jump_inventory,
    read_jump_inventory_with_provenance_mode,
};
pub use locations::{
    AreaEntity, ChildLocation, EntXmlCandidate, EntXmlDocument, EntXmlLink, HarvestableComponent,
    HcdAttribute, HcdDocument, HcdLink, HcdSelectionFailure, LocationAttribute, LocationInstance,
    LocationMetadataElement, LocationPlacementSnapshot, LocationTag, read_location_instances,
    read_location_instances_with_provenance_mode,
};
pub use object_placements::{
    StarmapObjectPlacementCoverage, StarmapObjectPlacementSnapshot, StarmapObjectPlacementStatus,
    read_starmap_object_placement, read_starmap_object_placement_with_provenance_mode,
};

pub use container_visit::{
    ContainerPlacementEvent, ContainerPlacementRootCoverage, ContainerPlacementRootVisit,
    ContainerPlacementRowCounts, ContainerPlacementVisitSummary, visit_container_placements,
    visit_container_placements_with_provenance_mode,
};
pub use system_roots::{
    ContainerSystemRoot, ContainerSystemRootInventory, read_container_system_roots,
    read_container_system_roots_with_provenance_mode,
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
    fn selected_rows(self, table: PlacementTable) -> (u32, SelectedPlacementRows) {
        match self {
            Self::SystemRoot(root) => (root, SelectedPlacementRows::new(table, None, None)),
            Self::Occurrence {
                root_entry_index,
                child_node_indices,
            } => (
                root_entry_index,
                SelectedPlacementRows::new(table, Some(child_node_indices.to_vec()), None),
            ),
            Self::ExactOccurrence {
                root_entry_index,
                child_node_indices,
                external_child_step_indices,
            } => (
                root_entry_index,
                SelectedPlacementRows::new(
                    table,
                    Some(child_node_indices.to_vec()),
                    Some(external_child_step_indices.to_vec()),
                ),
            ),
        }
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

#[path = "starmap_container_query.rs"]
mod container_query;

pub use container_query::{
    read_container_placements, read_container_placements_with_provenance_mode,
};

pub(super) struct SelectedPlacementRead {
    pub(super) root_entry_index: u32,
    pub(super) rows: SelectedPlacementRows,
    pub(super) coverage: ContainerPlacementCoverage,
    pub(super) source_inventory: SourceInventory,
}

pub(super) fn read_placements(
    archive: &Archive,
    selection: ContainerPlacementSelection<'_>,
    budget: StarmapTraversalBudget,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
    table: PlacementTable,
) -> Result<SelectedPlacementRead, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    let (root_entry_index, mut rows) = selection.selected_rows(table);
    with_entry_capture_mode(archive, provenance_mode, |capture| {
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
        Ok(SelectedPlacementRead {
            root_entry_index,
            rows,
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
    rows: &mut SelectedPlacementRows,
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
    rows: &mut SelectedPlacementRows,
) -> Result<RootRead, CatalogError> {
    let scope = match rows.table {
        PlacementTable::Containers => DescriptorScope::ContainerPlacements,
        PlacementTable::Locations => DescriptorScope::LocationPlacements,
    };
    let Read {
        bodies,
        ledger: observations,
        ..
    } = starmap_parse::read_all_scoped(
        archive,
        index,
        &[position],
        cancel,
        (budget.source_children, scope),
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
        (archive, capture, None, PlacementRows::Selected(rows)),
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
