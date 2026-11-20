// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::StarmapTraversalBudget;
use p4kx_export::catalog_queries::starmap::{
    ContainerPlacementSelection, read_container_placements,
};
use p4kx_extraction::Archive;

use super::super::{CONTAINERS, LOCATION_PHYSICS_GRIDS, UNRESOLVED};
use super::{Route, TestResult};

const REMAINING: &str =
    "system-root and occurrence query available; root enumeration and installed parity open";

pub(super) fn read(archive: &Archive, root_path: &str) -> TestResult<[Route; 3]> {
    let root = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == root_path)
        .ok_or("original root archive Entry")?;
    let starmap = read_container_placements(
        archive,
        ContainerPlacementSelection::SystemRoot(u32::try_from(root.index())?),
        StarmapTraversalBudget::new(),
        None,
    )?;
    let [grid] = starmap.location_physics_grids.as_slice() else {
        return Err("one synthetic grid descriptor".into());
    };
    Ok([
        Route::queried_rows(CONTAINERS, &starmap.container_instances, REMAINING)?,
        Route::queried_value(
            LOCATION_PHYSICS_GRIDS,
            grid.as_ref(),
            "direct descriptor and its native producer-stage parity verified; installed canonical occurrence parity open",
        )?,
        Route::queried_rows(UNRESOLVED, &starmap.unresolved_container_links, REMAINING)?,
    ])
}
