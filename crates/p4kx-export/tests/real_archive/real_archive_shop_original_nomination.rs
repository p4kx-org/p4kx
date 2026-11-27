// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::StarmapTraversalBudget;
use p4kx_export::catalog_queries::shops::{
    ResolutionStatus, ShopInventoryResolution, ShopRegistrySelection, ShopResolutionSelection,
    read_shop_inventory_resolutions_with_provenance_mode,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, DatasetAvailability, ProvenanceMode,
};
use p4kx_extraction::Archive;

use super::super::{equipment_input::with_archive, scoped_rows_digest};
use super::{check_key_source, check_located, registry_coordinate};
use crate::real_archive_catalog_table_parity::{Origins, TestResult, wire_row};
use crate::real_archive_verification::print_line;

#[test]
#[ignore = "real Archive: nominate one physical Shop from original scope outside the measured query"]
fn nominate_located_shop_from_original_archive() -> TestResult {
    with_archive(nominate)
}

fn nominate(archive: &Archive) -> TestResult {
    let candidates = registry_candidates(archive)?;
    let session = CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)
        .map_err(|_| "open a reusable DataForge session for Shop nomination")?
        .with_provenance_mode(ProvenanceMode::Full);
    let parsed = session
        .parsed(None)
        .map_err(|_| "parse the DataForge session")?;
    for (position, selection) in candidates.iter().copied().enumerate() {
        let snapshot = parsed
            .read_shop(selection, StarmapTraversalBudget::default(), None)
            .map_err(|_| "read the nominated original Shop candidate")?;
        if snapshot.build.as_ref() != archive.build()
            || snapshot.selection != selection
            || !matches!(snapshot.availability, DatasetAvailability::Available)
            || !snapshot.coverage.registry_complete
        {
            return Err(
                "the selected Shop does not retain a conclusive original registry identity".into(),
            );
        }
        let shop = snapshot
            .row
            .as_ref()
            .ok_or("an admitted physical Shop candidate is missing")?;
        if shop.starmap_object_guid.value().is_none()
            || shop
                .item_guids
                .value()
                .is_none_or(|items| items.as_slice().is_empty())
        {
            continue;
        }
        check_located(&snapshot, shop)?;
        check_key_source(&snapshot, shop)?;
        let rows = [wire_row(shop)?];
        Origins::default().extend(&snapshot.source_inventory, &rows)?;
        let digest = scoped_rows_digest(&rows)?;
        print_line(&format!(
            "original located Shop nomination entryIndex={} nodeIndex={} checked={} candidates={} digest={digest}",
            selection.entry_index,
            selection.node_index,
            position + 1,
            candidates.len(),
        ));
        return Ok(());
    }
    Err("none of the resolved original Shop candidates proved both a Starmap link and nonempty inventory".into())
}

fn registry_candidates(archive: &Archive) -> TestResult<Vec<ShopRegistrySelection>> {
    let resolution = read_shop_inventory_resolutions_with_provenance_mode(
        archive,
        ShopResolutionSelection::All,
        ProvenanceMode::Full,
        None,
    )
    .map_err(|_| "observe original Shop registry and inventory resolutions")?;
    if resolution.build.as_ref() != archive.build()
        || !matches!(resolution.availability, DatasetAvailability::Available)
        || !resolution.coverage.registry_complete
        || resolution.coverage.discovery.as_ref().is_none_or(|scope| {
            !scope.declarations_complete || !scope.inventory_directory.source_set_complete()
        })
    {
        return Err(
            "the original Shop registry and compatibility inventory survey must be conclusive"
                .into(),
        );
    }
    resolution
        .rows
        .iter()
        .filter(|row| unique_inventory(row))
        .map(|row| registry_coordinate(row.shop_key(), &resolution.source_inventory))
        .collect()
}

fn unique_inventory(row: &ShopInventoryResolution) -> bool {
    row.status().value() == Some(&ResolutionStatus::Resolved)
        && row.status().origin().source_set_complete()
        && row.physical_targets().origin().source_set_complete()
        && matches!(
            row.physical_targets().value().map(Vec::as_slice),
            Some([target]) if target.value().is_some()
        )
}
