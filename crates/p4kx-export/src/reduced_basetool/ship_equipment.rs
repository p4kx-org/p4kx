// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::path::Path;

use p4kx_extraction::CancelToken;

use super::ReducedError;
use super::cargo_grids::CargoGrids;
use super::cargo_placements;
use super::completeness;
use super::ship_cargo::{self, ShipCargo};
use super::ship_loadout::ShipLoadout;
use crate::catalog_queries::ParsedCatalogQueries;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogSequence, StagedCatalogTable};
use crate::contract_loadout::EntityKind;

const LOADOUTS: &str = "loadouts";

pub(super) struct ShipEquipment {
    pub(super) loadouts: StagedCatalogSequence,
    pub(super) cargo_grids: StagedCatalogTable,
    pub(super) cargo_placements: StagedCatalogTable,
    pub(super) ship_base_facts: StagedCatalogTable,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    mut cargo: BTreeMap<u32, ShipCargo>,
) -> Result<ShipEquipment, ReducedError> {
    let (grids, cargo_grids) = CargoGrids::collect(parsed, scratch, cancel)?;
    let mut loadouts = StagedCatalogRows::new(scratch, cancel);
    let mut placements = StagedCatalogRows::new(scratch, cancel);
    let mut facts = StagedCatalogRows::new(scratch, cancel);
    let mut placement_count = 0;
    let summary = parsed.visit_ship_loadout_placements(
        cancel,
        |record| Ok(grids.links(record)),
        |owner| {
            let loadout = owner.row.ok_or(ReducedError::IncompleteScope(LOADOUTS))?;
            if loadout.record_index() != owner.record_index
                || loadout.entity_guid() != owner.guid
                || loadout.entity_kind() != EntityKind::Ship
            {
                return Err(ReducedError::IncompleteScope(LOADOUTS));
            }
            let base = cargo
                .remove(&owner.record_index)
                .ok_or(ReducedError::IncompleteScope(ship_cargo::PATH))?;
            if base.ship_record_index.value != Some(owner.record_index)
                || base.ship_guid.value.map(|guid| guid.get()) != Some(owner.guid)
            {
                return Err(ReducedError::IncompleteScope(ship_cargo::PATH));
            }
            let installed =
                cargo_placements::stage(loadout, owner.placements, &grids, &mut placements)?;
            placement_count += installed.len();
            loadouts.append(&ShipLoadout::from(loadout))?;
            facts.append(&base.row(&installed)?)?;
            Ok(())
        },
    )?;
    completeness::all_fields(LOADOUTS, summary.availability, &summary.coverage)?;
    if !summary.dependencies_complete || !cargo.is_empty() {
        return Err(ReducedError::IncompleteScope(LOADOUTS));
    }
    completeness::rows(LOADOUTS, summary.owner_count, summary.row_count)?;
    let loadouts = loadouts.finish_sequence()?;
    completeness::rows(LOADOUTS, summary.row_count, loadouts.len())?;
    let placements = placements.finish(grids.placement_availability(placement_count))?;
    completeness::rows(cargo_placements::PATH, placement_count, placements.len())?;
    let facts = facts.finish(summary.availability)?;
    completeness::rows(ship_cargo::PATH, summary.row_count, facts.len())?;
    Ok(ShipEquipment {
        loadouts,
        cargo_grids,
        cargo_placements: placements,
        ship_base_facts: facts,
    })
}
