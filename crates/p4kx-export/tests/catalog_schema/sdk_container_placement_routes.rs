// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::StarmapTraversalBudget;
use p4kx_export::catalog_queries::loot::{
    LootDocumentQuery, LootDocumentSelection, read_loot_documents,
};
use p4kx_export::catalog_queries::starmap;
use p4kx_export::catalog_queries::starmap::{
    ContainerPlacementSelection, read_container_placements,
};
use p4kx_extraction::Archive;

use super::super::{
    CONTAINERS, LOCATION_INSTANCES, LOCATION_PHYSICS_GRIDS, LOOT_DOCUMENTS, UNRESOLVED,
};
use super::{Route, TestResult};

pub(super) fn read(archive: &Archive, root_path: &str) -> TestResult<[Route; 5]> {
    let root = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == root_path)
        .ok_or("original root archive Entry")?;
    let root_index = u32::try_from(root.index())?;
    let documents = read_loot_documents(
        archive,
        LootDocumentQuery::new(LootDocumentSelection::Container {
            container_entry_index: root_index,
        }),
        None,
    )?;
    let starmap = read_container_placements(
        archive,
        ContainerPlacementSelection::SystemRoot(root_index),
        StarmapTraversalBudget::new(),
        None,
    )?;
    let [grid] = starmap.location_physics_grids.as_slice() else {
        return Err("one synthetic grid descriptor".into());
    };

    let locations = starmap::read_location_instances(
        archive,
        ContainerPlacementSelection::SystemRoot(root_index),
        StarmapTraversalBudget::new(),
        None,
    )?;
    Ok([
        Route::queried_rows(CONTAINERS, &starmap.container_instances, None)?,
        Route::queried_rows(
            LOCATION_INSTANCES,
            &locations.location_instances,
            "system-root and exact occurrence query with synthetic Catalog parity; installed parity open",
        )?,
        Route::queried_value(
            LOCATION_PHYSICS_GRIDS,
            grid.as_ref(),
            "direct descriptor and its native producer-stage parity verified; installed canonical occurrence parity open",
        )?,
        Route::queried_rows(
            LOOT_DOCUMENTS,
            &documents.rows,
            "physical document selection and synthetic Catalog parity; installed normal-Catalog parity open",
        )?,
        Route::queried_rows(UNRESOLVED, &starmap.unresolved_container_links, None)?,
    ])
}
