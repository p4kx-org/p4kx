// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, ProvenanceMode};

use super::{
    ContainerPlacementSelection, ContainerPlacementSnapshot, SelectedPlacementRead, read_placements,
};
use crate::starmap_walk::{PlacementTable, SelectedPlacementRows};
use crate::{CatalogError, StarmapTraversalBudget};

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
    read_container_placements_with_provenance_mode(
        archive,
        selection,
        budget,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads container placements with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_container_placements`.
pub fn read_container_placements_with_provenance_mode(
    archive: &Archive,
    selection: ContainerPlacementSelection<'_>,
    budget: StarmapTraversalBudget,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<ContainerPlacementSnapshot, CatalogError> {
    let SelectedPlacementRead {
        root_entry_index,
        rows,
        coverage,
        source_inventory,
    } = read_placements(
        archive,
        selection,
        budget,
        provenance_mode,
        cancel,
        PlacementTable::Containers,
    )?;
    let SelectedPlacementRows {
        child_node_indices,
        external_child_step_indices,
        instances,
        unresolved,
        location_physics,
        ..
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
}
