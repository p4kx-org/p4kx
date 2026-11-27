// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, CancelToken};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::inventory::Inventory;
use super::selected_tables::SelectedTables;
use crate::catalog_queries::DatasetAvailability;
use crate::catalog_queries::shops::{
    ResolutionStatus, Shop, ShopInventorySelection, ShopResolutionSelection, read_shop_inventories,
    read_shop_inventory_resolutions, visit_shops,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogSequence, StagedCatalogTable};
use crate::guid_text::CatalogGuid;
use crate::shop_inventories::inventory_path;
use crate::{CatalogError, ExportAccountingError, StarmapTraversalBudget};

#[derive(Serialize)]
pub(super) struct ShopSlice {
    shops: StagedCatalogSequence,
    definitions: Definitions,
    declarations: Declarations,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Definitions {
    shop_inventories: StagedCatalogTable,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Declarations {
    shop_inventory_resolution: StagedCatalogTable,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShopRow<'source> {
    key: Cell<&'source String>,
    entity_guid: Cell<&'source String>,
    starmap_object_guid: Cell<&'source CatalogGuid>,
    system: Cell<&'source String>,
    entity_name: Cell<&'source String>,
    registry_name: Cell<&'source String>,
    location: Cell<&'source String>,
    kind: Cell<&'source String>,
    brand: Cell<&'source String>,
    commodity_exchange: Cell<&'source bool>,
    inventory: Cell<&'source String>,
    containers: Cell<Vec<Cell<&'source String>>>,
    item_guids: Cell<Vec<Cell<&'source String>>>,
}

impl<'source> From<&'source Shop> for ShopRow<'source> {
    fn from(shop: &'source Shop) -> Self {
        let fields = &shop.registry;
        Self {
            key: (&fields.key).into(),
            entity_guid: (&fields.entity_guid).into(),
            starmap_object_guid: (&shop.starmap_object_guid).into(),
            system: (&fields.system).into(),
            entity_name: (&fields.entity_name).into(),
            registry_name: (&fields.registry_name).into(),
            location: (&fields.location).into(),
            kind: (&shop.kind).into(),
            brand: (&shop.brand).into(),
            commodity_exchange: (&fields.commodity_exchange).into(),
            inventory: (&shop.inventory).into(),
            containers: Cell::from(&fields.containers)
                .map(|rows| rows.iter().map(Cell::from).collect()),
            item_guids: Cell::from(&shop.item_guids)
                .map(|rows| rows.as_slice().iter().map(Cell::from).collect()),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Resolution<'source> {
    shop_key: Cell<&'source String>,
    status: Cell<&'source ResolutionStatus>,
}

pub(super) fn collect(
    archive: &Archive,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<ShopSlice, ReducedError> {
    let definitions = read_inventories(archive, scratch, cancel)?;
    let (shops, keys) = read_shops(archive, scratch, cancel)?;
    let declarations = read_resolutions(archive, scratch, cancel, &keys)?;
    Ok(ShopSlice {
        shops,
        definitions: Definitions {
            shop_inventories: definitions,
        },
        declarations: Declarations {
            shop_inventory_resolution: declarations,
        },
    })
}

impl ShopSlice {
    pub(super) fn add_to(self, tables: &mut SelectedTables) -> Result<(), ReducedError> {
        tables.sequence("shops", self.shops)?;
        tables.table(
            "definitions.shopInventories",
            self.definitions.shop_inventories,
        )?;
        tables.table(
            "declarations.shopInventoryResolution",
            self.declarations.shop_inventory_resolution,
        )
    }

    #[cfg(test)]
    pub(super) fn take_spool_failure(&self) -> Option<crate::catalog_rows::spool::RowSpoolError> {
        self.shops
            .take_failure()
            .or_else(|| self.definitions.shop_inventories.take_failure())
            .or_else(|| self.declarations.shop_inventory_resolution.take_failure())
    }
}

fn read_inventories(
    archive: &Archive,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogTable, ReducedError> {
    if crate::export_archive_health::inventory(archive).is_some() {
        return Err(ReducedError::IncompleteScope("definitions.shopInventories"));
    }
    let mut admitted = Vec::new();
    for entry in archive.entries() {
        checkpoint(cancel)?;
        if let Some(path) = inventory_path(entry) {
            admitted
                .try_reserve(1)
                .map_err(CatalogError::SourceUseAllocation)?;
            admitted.push((path, entry.index()));
        }
    }
    admitted.sort_unstable();
    let availability = if admitted.is_empty() {
        DatasetAvailability::Absent
    } else {
        DatasetAvailability::Available
    };
    let mut staged = StagedCatalogRows::new(scratch, cancel);
    for (_, index) in admitted {
        let index = u32::try_from(index)
            .map_err(|_| CatalogError::from(ExportAccountingError::CounterOverflow))?;
        let snapshot =
            read_shop_inventories(archive, ShopInventorySelection::EntryIndex(index), cancel)?;
        let [definition] = snapshot.rows.as_slice() else {
            return Err(ReducedError::UnreadableInventory(index));
        };
        if definition.identity().entry_index() != index || !snapshot.coverage.failures.is_empty() {
            return Err(ReducedError::UnreadableInventory(index));
        }
        staged.append(&Inventory::project(definition)?)?;
    }
    Ok(staged.finish(availability)?)
}

fn read_shops(
    archive: &Archive,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<(StagedCatalogSequence, Vec<String>), ReducedError> {
    let mut staged = StagedCatalogRows::new(scratch, cancel);
    let mut keys = Vec::new();
    let summary = visit_shops(archive, StarmapTraversalBudget::default(), cancel, |shop| {
        let row = ShopRow::from(shop);
        let key = row
            .key
            .value
            .ok_or(ReducedError::IncompleteScope("shops"))?;
        keys.try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        keys.push(key.clone());
        staged.append(&row)?;
        Ok::<_, ReducedError>(())
    })?;
    let coverage = summary.coverage;
    let names_complete = coverage.names.is_none_or(|names| {
        names.routes_complete
            && names.records.unreadable_headers.is_empty()
            && names.records.unreadable_bodies.is_empty()
            && names.records.incomplete_fields.is_empty()
    });
    if !coverage.registry_complete
        || coverage.declarations_complete == Some(false)
        || !names_complete
        || summary.row_count != keys.len()
        || matches!(summary.availability, DatasetAvailability::Unavailable)
    {
        return Err(ReducedError::IncompleteScope("shops"));
    }
    Ok((staged.finish_sequence()?, keys))
}

fn read_resolutions(
    archive: &Archive,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    keys: &[String],
) -> Result<StagedCatalogTable, ReducedError> {
    const PATH: &str = "declarations.shopInventoryResolution";
    let snapshot = read_shop_inventory_resolutions(archive, ShopResolutionSelection::All, cancel)?;
    let directory_complete = snapshot
        .coverage
        .discovery
        .as_ref()
        .is_none_or(|discovery| {
            discovery.declarations_complete && discovery.inventory_directory.source_set_complete()
        });
    if !snapshot.coverage.registry_complete
        || !directory_complete
        || snapshot.rows.len() != keys.len()
    {
        return Err(ReducedError::IncompleteScope(PATH));
    }
    let mut staged = StagedCatalogRows::new(scratch, cancel);
    for (row, key) in snapshot.rows.iter().zip(keys) {
        checkpoint(cancel)?;
        if row.shop_key().value() != Some(key)
            || matches!(
                row.status().value(),
                None | Some(
                    ResolutionStatus::Unreadable
                        | ResolutionStatus::EmptySource
                        | ResolutionStatus::Unscanned
                )
            )
        {
            return Err(ReducedError::IncompleteScope(PATH));
        }
        staged.append(&Resolution {
            shop_key: row.shop_key().into(),
            status: row.status().into(),
        })?;
    }
    Ok(staged.finish(snapshot.availability)?)
}
