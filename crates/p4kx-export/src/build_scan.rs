// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{Archive, CancelToken, DataForge};

use crate::catalog_crafting::CraftingKinds;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::EntryCapture;
use crate::collected::Collected;
use crate::options::{CatalogRun, CatalogStage};
use crate::scan::{Scan, ScanDomain};
use crate::scan_health::ScanRead;
use crate::starmap_containers::jump_class::JumpEndpointRecords;
use crate::starmap_containers::{PlacementScope, Placements};
use crate::{CatalogError, CatalogOptions};

/// Reads required placements and assembles ordered selected Record rows.
pub(super) fn scan<'f>(
    archive: &Archive,
    forge: &'f DataForge<'f>,
    files: &'f ArchiveFiles<'f>,
    entities: &Arc<Entities<'f>>,
    run: CatalogRun<'_, '_>,
    projection: &'f CatalogProjection<'f, 'f>,
    crafting: CraftingKinds<'f>,
) -> Result<(Collected<'f>, Placements), CatalogError> {
    run.begin(CatalogStage::WorldPlacements);
    let options = run.options;
    let scope = placement_scope(forge, options);
    let mut placements = match scope {
        Some(scope) => {
            let endpoints = JumpEndpointRecords::read(forge, projection.original(), options)?;
            Placements::read_for_catalog(
                archive,
                options.cancel,
                &EntryCapture::for_input(projection.original()),
                scope,
                &endpoints,
            )?
        }
        None => Placements::default(),
    };
    run.begin(CatalogStage::Records);
    let scanned = Scan::run(
        forge,
        files,
        Arc::clone(entities),
        options,
        projection,
        &crafting,
        ScanDomain::all(&placements),
    )?;
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    run.begin(CatalogStage::Projections);
    let mut collected = collect(crafting, scanned);
    let entry_ledger = if matches!(scope, Some(PlacementScope::RecordSelection)) {
        placements.ledger.clone()
    } else {
        std::mem::take(&mut placements.ledger)
    };
    collected.ledger.merge(entry_ledger);
    collected.ledger.archive_inventory_problem = crate::export_archive_health::inventory(archive);
    let original = projection.original();
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

pub(super) fn collect<'f>(mut crafting: CraftingKinds<'f>, scanned: ScanRead<'f>) -> Collected<'f> {
    crafting.ledger.merge(scanned.ledger);
    let mut collected = Collected::new(scanned.picked, crafting);
    collected.resource_group_members = scanned.resource_group_members;
    collected
        .resource_group_members
        .sort_by_cached_key(|row| (row.physical.guid.text(), row.physical.record_index));
    collected.tags.entity_tags = scanned.entity_tags;
    collected.entity_configurations = scanned.entity_configurations;
    collected.tags.order_entities();
    collected
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
