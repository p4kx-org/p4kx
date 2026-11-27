// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;

use super::ReducedError;
use super::jump_inventory::{self, JumpInventory};
use super::selected_tables::SelectedTables;
use super::selected_texts::SelectedTexts;
use super::starmap_objects::Dependencies;
use super::{
    jump_point_configuration, quantum_drives, starmap_definitions, starmap_objects,
    starmap_placements,
};
use crate::StarmapTraversalBudget;
use crate::catalog_queries::ParsedCatalogQueries;

/// Stages the selected navigation tables and their referenced texts.
pub(super) fn stage(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    budget: StarmapTraversalBudget,
    tables: &mut SelectedTables,
    texts: &mut SelectedTexts,
) -> Result<(), ReducedError> {
    let types = starmap_definitions::collect_types(parsed, scratch, cancel, texts)?;
    let amenities = starmap_definitions::collect_amenities(parsed, scratch, cancel)?;
    let objects = starmap_objects::collect(
        parsed,
        &Dependencies {
            types: &types.targets,
            amenities: &amenities.targets,
        },
        scratch,
        cancel,
        texts,
    )?;
    let navigation =
        starmap_placements::collect(parsed, &objects.targets, budget, scratch, cancel)?;
    let JumpInventory {
        endpoints,
        connections,
    } = navigation.jumps;

    tables.table(starmap_definitions::TYPES, types.table)?;
    tables.table(starmap_definitions::AMENITIES, amenities.table)?;
    tables.table(starmap_objects::PATH, objects.table)?;
    tables.table(starmap_placements::PATH, navigation.placements)?;
    tables.table(jump_inventory::ENDPOINTS, endpoints)?;
    tables.table(jump_inventory::CONNECTIONS, connections)?;
    tables.table(
        "configurations.quantumDrives",
        quantum_drives::collect(parsed, scratch, cancel)?,
    )?;
    tables.table(
        jump_point_configuration::PATH,
        jump_point_configuration::collect(parsed, scratch, cancel)?,
    )?;
    Ok(())
}
