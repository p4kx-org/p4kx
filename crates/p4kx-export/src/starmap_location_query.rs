// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, ProvenanceMode, SourceInventory};

use super::{ContainerPlacementCoverage, ContainerPlacementSelection, SelectedPlacementRead};
use crate::starmap_walk::PlacementTable;
use crate::{CatalogError, StarmapTraversalBudget};

pub use crate::starmap_containers::location_declaration::{
    ChildLocation, LocationAttribute, LocationMetadataElement, LocationTag,
};
pub use crate::starmap_containers::location_instance::LocationInstance;
pub use crate::starmap_parse::entxml::{
    AreaEntity, EntXmlCandidate, EntXmlDocument, EntXmlLink, HarvestableComponent,
};
pub use crate::starmap_parse::hcd::{HcdAttribute, HcdDocument, HcdLink, HcdSelectionFailure};

/// Canonical typed location occurrences reached from one physical system root.
#[derive(Debug)]
#[non_exhaustive]
pub struct LocationPlacementSnapshot {
    /// Build metadata present in the watched Archive.
    pub build: Option<Build>,
    /// Original system root Entry index.
    pub root_entry_index: u32,
    /// Child XML ordinals when the query selected an occurrence.
    pub child_node_indices: Option<Vec<u32>>,
    /// External document steps when the query selected an exact occurrence.
    pub external_child_step_indices: Option<Vec<usize>>,
    /// Selected typed location rows in original traversal order.
    pub location_instances: Vec<LocationInstance>,
    /// Measured coverage of the selected root's descriptor graph.
    pub coverage: ContainerPlacementCoverage,
    /// Original sources and uses for the selected read and returned rows.
    pub source_inventory: SourceInventory,
}

/// Reads canonical typed locations and their companions from one original system root.
/// The occurrence selection limits output. Descriptor and companion coverage belongs to the root.
///
/// # Errors
/// Returns an error for cancellation, Archive changes or contradictory source evidence.
pub fn read_location_instances(
    archive: &Archive,
    selection: ContainerPlacementSelection<'_>,
    budget: StarmapTraversalBudget,
    cancel: Option<&CancelToken>,
) -> Result<LocationPlacementSnapshot, CatalogError> {
    read_location_instances_with_provenance_mode(
        archive,
        selection,
        budget,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads location instances with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_location_instances`.
pub fn read_location_instances_with_provenance_mode(
    archive: &Archive,
    selection: ContainerPlacementSelection<'_>,
    budget: StarmapTraversalBudget,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<LocationPlacementSnapshot, CatalogError> {
    let SelectedPlacementRead {
        root_entry_index,
        rows,
        coverage,
        source_inventory,
    } = super::read_placements(
        archive,
        selection,
        budget,
        provenance_mode,
        cancel,
        PlacementTable::Locations,
    )?;
    Ok(LocationPlacementSnapshot {
        build: archive.build().cloned(),
        root_entry_index,
        child_node_indices: rows.child_node_indices,
        external_child_step_indices: rows.external_child_step_indices,
        location_instances: rows.location_instances,
        coverage,
        source_inventory,
    })
}
