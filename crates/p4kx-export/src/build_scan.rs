// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{Archive, CancelToken, DataForge};

use crate::build::original_input::OriginalCatalogInput;
use crate::build_harvest::add_harvest_locations;
use crate::build_loot::add_loot_containers;
use crate::build_starmap::add_starmap;
use crate::catalog_crafting::CraftingKinds;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::EntryCapture;
use crate::collected::Collected;
use crate::options::{CatalogRun, CatalogStage};
use crate::scan::{Scan, ScanDomain};

use crate::starmap_containers::jump_class::JumpEndpointRecords;
use crate::starmap_containers::{PlacementScope, Placements};
use crate::{CatalogError, CatalogOptions, StarmapSummary};

/// Reads required placements and assembles ordered selected Record rows.
pub(super) fn scan<'f>(
    archive: &Archive,
    original: &'f OriginalCatalogInput<'f, 'f>,
    files: &'f ArchiveFiles<'f>,
    entities: &Arc<Entities<'f>>,
    run: CatalogRun<'_, '_>,
    projection: &CatalogProjection<'_, 'f>,
    crafting: CraftingKinds<'f>,
) -> Result<(Collected<'f>, Placements), CatalogError> {
    run.begin(CatalogStage::WorldPlacements);
    let options = run.options;
    let forge = original.forge();
    let scope = placement_scope(forge, options);
    let mut placements = read_placements(archive, scope, original, options)?;
    run.begin(CatalogStage::Records);
    #[cfg(test)]
    run.capture_retention(
        CatalogStage::Records,
        &original.origins().uses,
        &[&placements.ledger],
    )?;
    let mut collected = Scan::run(
        original,
        files,
        Arc::clone(entities),
        options,
        projection,
        crafting,
        ScanDomain::all(&placements),
    )?;
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    placements.release_record_lookup();
    run.begin(CatalogStage::Projections);
    #[cfg(test)]
    run.capture_retention(
        CatalogStage::Projections,
        &original.origins().uses,
        &[&placements.ledger, &collected.ledger],
    )?;
    collected.finish_selection();

    // Harvest and loot consume placement dependencies in RecordSelection scope.
    if matches!(scope, Some(PlacementScope::ArchiveInventory)) {
        collected
            .ledger
            .merge(std::mem::take(&mut placements.ledger));
    }
    collected.ledger.archive_inventory_problem = crate::export_archive_health::inventory(archive);
    let capture = EntryCapture::for_input(original);
    collected.planet_tech = crate::catalog_definitions::planet_tech_definitions::read(
        archive,
        forge,
        original,
        &capture,
        &mut collected.ledger,
        options,
    )?;
    Ok((collected, placements))
}

fn read_placements(
    archive: &Archive,
    scope: Option<PlacementScope>,
    original: &OriginalCatalogInput<'_, '_>,
    options: &CatalogOptions<'_>,
) -> Result<Placements, CatalogError> {
    let Some(scope) = scope else {
        return Ok(Placements::default());
    };
    let endpoints = JumpEndpointRecords::read(original.forge(), original, options)?;
    Placements::read_for_catalog(
        archive,
        options.cancel,
        &EntryCapture::for_input(original),
        scope,
        &endpoints,
        &std::env::temp_dir(),
    )
}

pub(super) fn project_locations(
    archive: &Archive,
    placements: &mut Placements,
    collected: &mut Collected<'_>,
    capture: &EntryCapture<'_>,
    run: CatalogRun<'_, '_>,
) -> Result<StarmapSummary, CatalogError> {
    run.begin(CatalogStage::ProjectLocations);
    let summary = add_starmap(collected, placements.counts);
    let cancel = run.options.cancel;
    add_harvest_locations(archive, placements, collected, cancel, capture)?;
    add_loot_containers(archive, placements, collected, cancel, capture)?;
    Ok(summary)
}

fn placement_scope(forge: &DataForge<'_>, options: &CatalogOptions<'_>) -> Option<PlacementScope> {
    let rules = [
        crate::Kind::StarmapObjects.records(),
        crate::Kind::HarvestableProviderPresets.records(),
    ];
    let wanted = forge.records().any(|record| {
        record.record_path().is_ok_and(|path| {
            rules.iter().any(|rule| rule.matches_path(path))
                && options.filter.is_none_or(|filter| filter.matches(path))
        })
    });
    if wanted {
        Some(PlacementScope::RecordSelection)
    } else {
        options
            .filter
            .is_none()
            .then_some(PlacementScope::ArchiveInventory)
    }
}
