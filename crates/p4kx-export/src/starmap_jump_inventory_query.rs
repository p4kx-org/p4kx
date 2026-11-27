// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_assembly::Entities;
use p4kx_extraction::{
    Archive, ArchiveWatch, Build, CancelToken, DataForge, DataForgeEntry, Guid, ProvenanceMode,
    SourceInventory,
};
use serde::Serialize;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projections::JumpConnection;
use crate::catalog_query_session::{checkpoint, load_error};
use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::export_archive_health;
use crate::export_entry_counts::ExportEntryCounts;
use crate::export_health_entries;
use crate::starmap_containers::jump_class::JumpEndpointRecords;
use crate::starmap_containers::jump_endpoint::JumpEndpoint;
use crate::starmap_containers::{PlacementScope, Placements};
use crate::{CatalogError, CatalogOptions, ExportAccountingError, Kind, StarmapTraversalBudget};

/// Availability of the authored relationship inventory.
#[derive(Debug, Serialize)]
#[serde(transparent)]
#[must_use]
pub struct JumpConnectionInventory(CatalogTable<JumpConnection>);

impl JumpConnectionInventory {
    /// Absence in the inspected grammar, or unavailability when coverage is incomplete.
    #[must_use]
    pub const fn availability(&self) -> DatasetAvailability {
        self.0.availability
    }

    /// The typed relationship inventory, which currently has no authored row shape.
    #[must_use]
    pub fn rows(&self) -> &[JumpConnection] {
        &self.0.rows
    }
}

/// Original source coverage for the jump inventory.
#[derive(Debug)]
#[non_exhaustive]
pub struct JumpInventoryCoverage {
    /// Whether the root Archive directory establishes the admitted root inventory.
    pub directory_complete: bool,
    /// Whether the required Starmap Record classifications could be determined.
    pub record_classifications_complete: bool,
    /// Whether every required endpoint occurrence was determined.
    pub endpoint_inventory_complete: bool,
    /// Number of canonical system roots considered.
    pub systems: usize,
    /// Number of observed endpoints with incomplete partner evidence.
    pub incomplete_partner_occurrences: usize,
    /// Original Entry outcomes counted by the canonical Entry health compiler.
    pub source_entries: ExportEntryCounts,
}

/// Canonical jump endpoint occurrences and authored relationship inventory.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct JumpInventorySnapshot {
    /// Build metadata from the watched Archive.
    pub build: Option<Build>,
    /// Availability of the endpoint placement table.
    pub endpoint_availability: DatasetAvailability,
    /// Physical occurrences in canonical system and Child order.
    pub endpoints: Vec<JumpEndpoint>,
    /// Global relationship availability for the same endpoint evidence.
    pub connections: JumpConnectionInventory,
    /// Coverage of the sources used by this query.
    pub coverage: JumpInventoryCoverage,
    /// Original sources and uses for the two tables.
    pub source_inventory: SourceInventory,
}

/// Reads the jump domain from every canonical system root.
/// The endpoint table retains observed occurrences from an incomplete traversal.
///
/// # Errors
/// Returns input, source evidence, Archive mutation or cancellation errors.
pub fn read_jump_inventory(
    archive: &Archive,
    budget: StarmapTraversalBudget,
    cancel: Option<&CancelToken>,
) -> Result<JumpInventorySnapshot, CatalogError> {
    read_jump_inventory_with_provenance_mode(archive, budget, ProvenanceMode::MarkersOnly, cancel)
}

/// Reads the starmap jump inventory with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_jump_inventory`.
pub fn read_jump_inventory_with_provenance_mode(
    archive: &Archive,
    budget: StarmapTraversalBudget,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<JumpInventorySnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    let entry =
        DataForgeEntry::read_until(archive, &|| cancel.is_some_and(CancelToken::is_cancelled))
            .map_err(load_error)?;
    let forge = entry.open().map_err(CatalogError::DataForge)?;
    let origins =
        CatalogOrigins::for_archive_records_with_provenance_mode(archive, &entry, provenance_mode)?;
    let (mut placements, candidates) =
        read_placements(archive, &forge, &origins, budget, cancel, None)?;
    let entries = export_health_entries::compile(std::mem::take(&mut placements.ledger.entries))?;
    let source_entries = export_health_entries::counts(Kind::StarmapObjects, &entries.sources)?;
    let jumps = finish_jumps(
        archive,
        placements,
        &candidates,
        source_entries,
        &origins,
        cancel,
    )?;
    watch.check()?;
    checkpoint(cancel)?;
    Ok(jumps)
}

fn read_placements(
    archive: &Archive,
    forge: &DataForge<'_>,
    origins: &CatalogOrigins,
    budget: StarmapTraversalBudget,
    cancel: Option<&CancelToken>,
    records: Option<&[Guid]>,
) -> Result<(Placements, JumpEndpointRecords), CatalogError> {
    let input = OriginalCatalogInput::new(forge, origins);
    let capture = EntryCapture::for_input(&input);
    let options = CatalogOptions {
        cancel,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let candidates = JumpEndpointRecords::read(forge, &input, &options)?;
    let scope = PlacementScope::for_catalog(forge, None)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let mut placements = Placements::read_for_jump_endpoints(
        archive,
        cancel,
        budget,
        &capture,
        scope,
        &candidates,
        records,
    )?;
    let entities = Entities::new(forge);
    placements.join_jump_operators(&entities, &input, cancel)?;
    Ok((placements, candidates))
}

fn finish_jumps(
    archive: &Archive,
    mut placements: Placements,
    candidates: &JumpEndpointRecords,
    source_entries: ExportEntryCounts,
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<JumpInventorySnapshot, CatalogError> {
    let connections = JumpConnectionInventory(CatalogTable::empty(
        placements.jump_connection_availability(),
    ));
    let endpoints = placements.take_jump_endpoints();
    let coverage = JumpInventoryCoverage {
        directory_complete: export_archive_health::inventory(archive).is_none(),
        record_classifications_complete: candidates.is_complete(),
        endpoint_inventory_complete: !matches!(
            endpoints.availability,
            DatasetAvailability::Unavailable
        ),
        systems: placements.counts.systems,
        incomplete_partner_occurrences: endpoints
            .rows
            .iter()
            .filter(|endpoint| !endpoint.partner_scope_complete())
            .count(),
        source_entries,
    };
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    Ok(JumpInventorySnapshot {
        build: archive.build().cloned(),
        endpoint_availability: endpoints.availability,
        endpoints: endpoints.rows,
        connections,
        coverage,
        source_inventory,
    })
}

#[path = "starmap_joint_placement_query.rs"]
mod joint;
