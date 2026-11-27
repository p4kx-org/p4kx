// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;

use super::ReducedError;
use super::record_targets::RecordTargets;
use super::selected_tables::SelectedTables;
use super::selected_texts::SelectedTexts;
use super::{
    attachable_modifiers, cargo_grids, cargo_placements, damage_resistance_macros,
    declared_attachment_constraints, declared_ports, durability, entities, item_definitions,
    manufacturer_definitions, shield_generators, ship_cargo, ship_equipment, suit_armor,
    vehicle_armor, wearable_fields,
};
use crate::catalog_queries::ParsedCatalogQueries;
use crate::ship_store_pledge::ShipStorePledges;

pub(super) struct Equipment {
    pub(super) ship_store_pledges: ShipStorePledges,
    pub(super) item_targets: RecordTargets,
}

/// Stages the selected Item, Ship, Loadout and equipment data.
pub(super) fn stage(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    tables: &mut SelectedTables,
    texts: &mut SelectedTexts,
) -> Result<Equipment, ReducedError> {
    let entities = entities::collect(parsed, scratch, cancel, texts)?;
    tables.sequence(entities::ITEMS, entities.items)?;
    tables.sequence(entities::SHIPS, entities.ships)?;
    tables.sequence(entities::PAINTS, entities.paints)?;
    tables.sequence(entities::STORE_SLUGS, entities.ship_store_slugs)?;
    tables.table(item_definitions::PATH, entities.item_definitions)?;
    tables.table(manufacturer_definitions::PATH, entities.manufacturers)?;
    tables.table(declared_ports::PATH, entities.equipment.ports)?;
    tables.table(
        declared_attachment_constraints::PATH,
        entities.equipment.attachments,
    )?;

    let ships = ship_equipment::collect(parsed, scratch, cancel, entities.ship_cargo)?;
    tables.sequence(ship_equipment::LOADOUTS, ships.loadouts)?;
    tables.table(cargo_grids::PATH, ships.cargo_grids)?;
    tables.table(cargo_placements::PATH, ships.cargo_placements)?;
    tables.table(ship_cargo::PATH, ships.ship_base_facts)?;

    stage_configurations(parsed, scratch, cancel, tables)?;
    Ok(Equipment {
        ship_store_pledges: entities.ship_store_pledges,
        item_targets: entities.item_targets,
    })
}

fn stage_configurations(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    tables: &mut SelectedTables,
) -> Result<(), ReducedError> {
    let wearables = wearable_fields::collect(parsed, scratch, cancel)?;
    tables.table(wearable_fields::CLOTHING, wearables.clothing)?;
    tables.table(wearable_fields::RADARS, wearables.radars)?;

    let suits = suit_armor::collect(parsed, scratch, cancel)?;
    tables.table(damage_resistance_macros::PATH, suits.macros)?;
    tables.table(suit_armor::PATH, suits.suits)?;

    tables.table(
        durability::PATH,
        durability::collect(parsed, scratch, cancel)?,
    )?;
    tables.table(
        shield_generators::PATH,
        shield_generators::collect(parsed, scratch, cancel)?,
    )?;
    tables.table(
        attachable_modifiers::PATH,
        attachable_modifiers::collect(parsed, scratch, cancel)?,
    )?;
    tables.table(
        vehicle_armor::PATH,
        vehicle_armor::collect(parsed, scratch, cancel)?,
    )
}
