// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, CancelToken};
use thiserror::Error;

use crate::catalog_rows::spool::RowSpoolError;
use crate::{CatalogError, CatalogFile};

mod blueprint_catalog;
mod blueprint_cost_context;
mod blueprint_recipe_coordinates;
mod blueprint_taxonomy;
mod blueprint_tiers;
mod blueprints;
mod bounded_output;
mod cargo_globals;
mod cargo_grids;
mod cargo_placements;
mod cell;
mod commodities;
mod completeness;
mod component;
mod contract_scope;
mod crafting_global;
mod damage_resistance_macros;
mod dismantle_processes;
mod durability;
mod entities;
mod faction_reputations;
mod inventory;
mod item;
mod item_definitions;
mod jump_inventory;
mod jump_point_configuration;
mod manufacturer;
mod manufacturer_definitions;
mod mission_boards;
mod number;
mod plan;
mod quantum_drives;
mod record_identity;
mod record_link;
mod record_targets;
mod reputation_catalog;
mod reputation_contexts;
mod reputation_perks;
mod reputation_ranks;
mod resource_density;
mod resource_groups;
mod resource_properties;
mod resource_quality;
mod resource_types;
mod selected_texts;
mod ship;
mod ship_cargo;
mod ship_equipment;
mod ship_loadout;
mod shops;
mod suit_armor;
mod wearable_fields;

#[cfg(test)]
mod blueprint_taxonomy_tests;
#[cfg(test)]
mod blueprints_tests;
#[cfg(test)]
mod cargo_globals_tests;
#[cfg(test)]
mod commodity_tests;
#[cfg(test)]
mod contract_scope_tests;
#[cfg(test)]
mod crafting_dependencies_tests;
#[cfg(test)]
mod durability_suit_tests;
#[cfg(test)]
mod entity_tests;
#[cfg(test)]
mod faction_reputations_tests;
#[cfg(test)]
mod item_definitions_tests;
#[cfg(test)]
mod jump_inventory_tests;
#[cfg(test)]
mod jump_point_configuration_tests;
#[cfg(test)]
mod mission_board_tests;
#[cfg(test)]
mod quantum_drive_tests;
#[cfg(test)]
mod reputation_perk_tests;
#[cfg(test)]
mod reputation_ranks_tests;
#[cfg(test)]
mod resource_group_tests;
#[cfg(test)]
mod resource_type_tests;
#[cfg(test)]
mod ship_equipment_tests;
#[cfg(test)]
mod wearable_fields_tests;

#[cfg(test)]
mod paint_fleet_tests;

#[cfg(test)]
mod tests;

#[derive(Debug, Error)]
enum ReducedError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Spool(#[from] RowSpoolError),
    #[error("the selected data is incomplete at {0}")]
    IncompleteScope(&'static str),
    #[error("the selected Shop inventory at Entry {0} is unreadable")]
    UnreadableInventory(u32),
    #[error("ship Record {0} declares a manufacturer shape without a reduced projection")]
    UnsupportedManufacturer(u32),
    #[error("the selected dataset {path} does not contain the target at Record {index}")]
    UnresolvedRecordLink { path: &'static str, index: u32 },
    #[error("a reduced Shop inventory value could not resolve its original typed address")]
    InvalidInventoryReference,
    #[error("the {kind:?} reduced output exceeds {limit} bytes")]
    Limit {
        kind: bounded_output::LimitKind,
        limit: u64,
    },
    #[error("could not discard the staged reduced output after {primary}: {cleanup}")]
    Cleanup {
        primary: Box<Self>,
        cleanup: p4kx_extraction::OwnedOutputCleanupError,
    },
}

fn prepare_shop_slice<'output>(
    archive: &Archive,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    limits: bounded_output::Limits,
    create: impl FnOnce() -> io::Result<CatalogFile<'output>>,
) -> Result<(CatalogFile<'output>, bounded_output::Sizes), ReducedError> {
    let watch = ArchiveWatch::new(archive);
    watch.check().map_err(CatalogError::from)?;
    let slice = shops::collect(archive, scratch, cancel)?;
    let decoded = bounded_output::measure(&slice, limits.decoded, cancel)
        .map_err(|error| slice.take_spool_failure().map_or(error, ReducedError::from))?;
    let mut file = create()?;
    let result = (|| {
        let sizes = bounded_output::write(&slice, &mut file, decoded, limits, cancel)
            .map_err(|error| slice.take_spool_failure().map_or(error, ReducedError::from))?;
        watch.check().map_err(CatalogError::from)?;
        crate::catalog_query_session::checkpoint(cancel)?;
        Ok(sizes)
    })();
    match result {
        Ok(sizes) => Ok((file, sizes)),
        Err(primary) => match file.discard() {
            Ok(()) => Err(primary),
            Err(cleanup) => Err(ReducedError::Cleanup {
                primary: Box::new(primary),
                cleanup,
            }),
        },
    }
}
