// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{Archive, CancelToken, DataForge};

use crate::catalog_crafting::CraftingKinds;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::EntryCapture;
use crate::collected::Collected;
use crate::scan::Scan;
use crate::starmap_containers::Placements;
use crate::{CatalogError, CatalogOptions};

/// Reads required placements and assembles ordered selected Record rows.
pub(super) fn scan<'f>(
    archive: &Archive,
    forge: &'f DataForge<'f>,
    files: &'f ArchiveFiles<'f>,
    entities: &Arc<Entities<'f>>,
    options: &CatalogOptions<'_>,
    projection: &'f CatalogProjection<'f, 'f>,
    crafting: CraftingKinds<'f>,
) -> Result<(Collected<'f>, Placements), CatalogError> {
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
    let placements = if wanted {
        Placements::read(
            archive,
            options.cancel,
            &EntryCapture::for_input(projection.original()),
        )?
    } else {
        Placements::default()
    };
    let scanned = Scan::run(
        forge,
        files,
        Arc::clone(entities),
        options,
        projection,
        &crafting,
        &placements,
    )?;
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let mut crafting = crafting;
    crafting.ledger.merge(scanned.ledger);
    let mut collected = Collected::new(scanned.picked, crafting);
    collected.resource_group_members = scanned.resource_group_members;
    collected
        .resource_group_members
        .sort_by_cached_key(|row| (row.physical.guid.text(), row.physical.record_index));
    collected.tags.entity_tags = scanned.entity_tags;
    collected.entity_configurations = scanned.entity_configurations;
    collected.tags.order_entities();

    collected.ledger.merge(placements.ledger.clone());
    Ok((collected, placements))
}
