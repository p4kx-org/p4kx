// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, DataForgeEntry, SourceInventory};

use super::super::loadout_query::{AlignmentDependencyState, Loadout, LoadoutDependencyCoverage};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, ParsedCatalogQueries,
};
use crate::catalog_v3::resource_network_placements::ResourceNetworkPlacementTables;

#[path = "equipment_topology_placement_read.rs"]
mod read;

/// The result of selecting one physical ship Loadout root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResourceTopologyRootState {
    /// The resolver produced the selected ship Loadout, possibly with partial coverage.
    Resolved,
    /// No physical Record has the requested index.
    MissingRecord,
    /// The original Record is outside the canonical Ship selection.
    NotAdmitted,
    /// The original header could not establish Ship admission.
    UnreadableHeader,
    /// The admitted Record could not be resolved as a Loadout.
    ResolutionUnavailable,
}

/// One physical declaration role reached by the topology projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResourceTopologyDependencyRole {
    /// Installed resources of a root or the physical item attached to a port.
    ResourceComponents,
    /// Original named and weak links in a declaring context.
    LinkDeclarations,
    /// Original room associations in a declaring context.
    RoomDeclarations,
}

/// Result of inspecting a reached physical Record for a topology role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResourceTopologyDependency {
    /// Physical index of the reached Record in the selected DataForge.
    pub record_index: u32,
    /// Original declaration family inspected at the Record.
    pub role: ResourceTopologyDependencyRole,
    /// The outcome of inspecting this family.
    pub state: AlignmentDependencyState,
}

/// Canonical resource topology placements and origins for one selected physical ship.
#[derive(Debug)]
#[non_exhaustive]
pub struct ResourceTopologyPlacementSnapshot {
    /// Original Archive Build.
    pub build: Option<Build>,
    /// Outcome of the exact root selection and resolution.
    pub root_state: ResourceTopologyRootState,
    /// Resolved Loadout used to traverse placement port paths.
    pub loadout: Option<Loadout>,
    /// The three canonical placement tables for the selected root.
    pub placements: ResourceNetworkPlacementTables,
    /// Selected owner coverage from the Loadout resolver.
    pub coverage: CatalogRecordCoverage,
    /// Original operations and alignment roles reached by the resolver and projection.
    pub loadout_dependencies: LoadoutDependencyCoverage,
    /// Typed original declaration dependencies in physical Record order.
    pub declaration_dependencies: Vec<ResourceTopologyDependency>,
    /// Original source uses shared by the loadout and all three tables.
    pub source_inventory: SourceInventory,
}

/// Reads canonical resource topology placements for one physical Ship Record.
///
/// # Errors
/// Returns Archive, source-evidence or cancellation errors.
pub fn read_resource_topology_placements(
    archive: &Archive,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<ResourceTopologyPlacementSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_resource_topology_placements(ship_record_index, cancel)
}

/// Reads topology placements using decoded DataForge bytes from the same Archive.
///
/// # Errors
/// Returns query errors or an origin mismatch for a foreign Entry.
pub fn read_resource_topology_placements_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    ship_record_index: u32,
    cancel: Option<&CancelToken>,
) -> Result<ResourceTopologyPlacementSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_resource_topology_placements(ship_record_index, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads scoped topology placements using the session's original Entry.
    ///
    /// # Errors
    /// Returns Archive, source-evidence or cancellation errors.
    pub fn read_resource_topology_placements(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<ResourceTopologyPlacementSnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_resource_topology_placements(ship_record_index, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads scoped topology placements using the parsed original DataForge.
    ///
    /// # Errors
    /// Returns Archive, source-evidence or cancellation errors.
    pub fn read_resource_topology_placements(
        &self,
        ship_record_index: u32,
        cancel: Option<&CancelToken>,
    ) -> Result<ResourceTopologyPlacementSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::run(archive, entry, forge, ship_record_index, cancel)
        })
    }
}
