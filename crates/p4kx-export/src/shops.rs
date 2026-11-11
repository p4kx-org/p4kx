// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_assembly::ArchiveFiles;
use p4kx_extraction::{Archive, CancelToken};

use crate::catalog_read::original::{EntryCapture, EntryDiagnosis};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::collected::Collected;
use crate::contract::{Shop, StarmapObject};
use crate::crafting_warning::CraftingWarning;
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_containers::{Containers, Inventory};
use crate::shop_entry_read;
use crate::shop_inventories::{Inventories, ItemList, PreparedInventories, is_placeholder};
use crate::shop_inventory_resolution::{InventoryResolver, ShopInventoryResolution};
use crate::shop_links::Links;
use crate::shop_prefabs::Prefabs;
#[cfg(test)]
use crate::shop_registry::RegistryShop;
use crate::shop_registry::{self, RegistryNeed};
use crate::shop_summary::ShopSummary;
use crate::{CatalogError, CatalogOptions, Kind};

/// The shop registry of the layer recovery data.
pub(crate) const REGISTRY: &str = "Data\\Libs\\CosmoTools\\LayerRecovery\\LayerBackups.xml";

#[path = "shop_list.rs"]
mod shop_list;
#[cfg(test)]
use shop_list::read as list;

/// The shops of the catalog and what reading them found.
pub(crate) struct Built {
    pub(crate) ledger: ExportLedger,
    pub(crate) shops: Vec<Shop>,
    pub(crate) resolutions: CatalogTable<ShopInventoryResolution>,
    pub(crate) summary: ShopSummary,
    pub(crate) unreadable_inventories: Vec<String>,
}

impl Built {
    const fn without_registry(summary: ShopSummary, ledger: ExportLedger) -> Self {
        let availability = if summary.registry_missing {
            DatasetAvailability::Absent
        } else {
            DatasetAvailability::Unavailable
        };
        Self {
            ledger,
            shops: Vec::new(),
            resolutions: CatalogTable::empty(availability),
            summary,
            unreadable_inventories: Vec::new(),
        }
    }
}

struct ShopDeclarations {
    containers: Containers,
    prefabs: Prefabs,
}

impl ShopDeclarations {
    fn read(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        capture: &EntryCapture<'_>,
        summary: &mut ShopSummary,
    ) -> Result<Self, CatalogError> {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let mut containers = Containers::read(archive, cancel, capture)?;
        let prefabs = Prefabs::read(archive, cancel, capture)?;
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        containers.add_prefabs(&prefabs)?;
        summary.prefab_ambiguous = prefabs
            .by_entity
            .keys()
            .filter(|entity| matches!(containers.inventory(entity), Inventory::Ambiguous))
            .count();
        summary.count_sources(&containers, &prefabs);
        Ok(Self {
            containers,
            prefabs,
        })
    }

    fn complete(&self) -> bool {
        !self
            .containers
            .ledger
            .incomplete_inventories
            .contains(&Kind::Shops)
            && !self
                .prefabs
                .ledger
                .incomplete_inventories
                .contains(&Kind::Shops)
            && !self.containers.unattributed_declarations
            && !self.prefabs.unattributed_declarations
    }
}

/// Reads the shop registry and the shop containers into `collected` and returns the counts. With
/// a Filter no shop is read: shops are selected by no record path.
pub(crate) fn add_shops(
    archive: &Archive,
    files: &ArchiveFiles<'_>,
    collected: &mut Collected<'_>,
    options: &CatalogOptions<'_>,
    capture: &EntryCapture<'_>,
) -> Result<ShopSummary, CatalogError> {
    let PreparedInventories {
        table,
        ledger,
        mut inventories,
    } = Inventories::prepare(archive, capture, options.cancel)?;
    collected.shop_inventories = table;
    collected.ledger.merge(ledger);
    if options.filter.is_some() {
        collected.ledger.inventory(Kind::Shops);
        collected.shop_inventory_resolution.availability = DatasetAvailability::Unavailable;
        return Ok(ShopSummary::default());
    }
    let built = shops(
        archive,
        files,
        &collected.starmap_objects,
        options.cancel,
        capture,
        &mut inventories,
    )?;
    collected.ledger.merge(built.ledger);
    let summary = built.summary;
    if summary.registry_unreadable {
        collected
            .warnings
            .push(CraftingWarning::UnreadableShopRegistry);
    } else if summary.registry_missing && !collected.starmap_objects.is_empty() {
        collected
            .warnings
            .push(CraftingWarning::MissingShopRegistry);
    }
    if summary.containers_unreadable > 0 {
        collected
            .warnings
            .push(CraftingWarning::UnreadableShopContainers(
                summary.containers_unreadable,
            ));
    }
    if summary.inventories_unreadable > 0 {
        collected
            .warnings
            .push(CraftingWarning::UnreadableShopInventories {
                count: summary.inventories_unreadable,
                details: built.unreadable_inventories,
            });
    }
    if summary.inventories_missing > 0 {
        collected
            .warnings
            .push(CraftingWarning::MissingShopInventories(
                summary.inventories_missing,
            ));
    }
    if summary.containers_with_shop > 0 && summary.soc_components == 0 {
        collected.warnings.push(CraftingWarning::NoShopComponents);
    }
    collected.shops = built.shops;
    collected.shop_inventory_resolution = built.resolutions;
    Ok(summary)
}

/// Reads the shop registry of `archive` and the shop containers, and links each shop's location
/// to the starmap object of `starmap` that carries its name.
pub(crate) fn shops(
    archive: &Archive,
    files: &ArchiveFiles<'_>,
    starmap: &[StarmapObject<'_>],
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
    inventories: &mut Inventories,
) -> Result<Built, CatalogError> {
    let mut summary = ShopSummary::default();
    let mut ledger = ExportLedger::default();
    ledger.inventory(Kind::Shops);
    let need = if starmap.is_empty() {
        RegistryNeed::Optional
    } else {
        RegistryNeed::Required
    };
    let registry =
        shop_registry::read_source(archive, files, need, &mut summary, &mut ledger, capture)?;
    let Some((registry, registry_source)) = registry else {
        return Ok(Built::without_registry(summary, ledger));
    };
    let declarations = ShopDeclarations::read(archive, cancel, capture, &mut summary)?;
    let declaration_complete = declarations.complete();
    let ShopDeclarations {
        containers,
        prefabs,
    } = declarations;
    summary.registry_entries = registry.len();
    let links = Links::new(starmap);
    let lists = inventory_lists(&containers, inventories, &mut summary, &mut ledger)?;
    let unreadable_inventories = unreadable_details(&lists);

    let resolver = InventoryResolver::new(
        &containers,
        inventories,
        &lists,
        &prefabs,
        capture,
        declaration_complete,
    );
    let (projected, resolutions) = shop_list::with_resolutions(
        &registry,
        &containers,
        &links,
        &lists,
        &mut summary,
        &resolver,
    )?;
    record_registry(registry_source, projected.disposition, &mut ledger);
    ledger.merge(containers.ledger);
    ledger.merge(prefabs.ledger);
    Ok(Built {
        ledger,
        shops: projected.shops,
        resolutions: CatalogTable {
            availability: DatasetAvailability::Available,
            rows: resolutions,
        },
        summary,
        unreadable_inventories,
    })
}

fn inventory_lists<'a>(
    containers: &'a Containers,
    inventories: &mut Inventories,
    summary: &mut ShopSummary,
    ledger: &mut ExportLedger,
) -> Result<Lists<'a>, CatalogError> {
    let lists: Lists<'_> = containers
        .inventory_selections()?
        .into_iter()
        .filter(|(stem, _)| !is_placeholder(stem))
        .map(|(stem, selection)| {
            inventories
                .items(&selection, ledger)
                .map(|items| (stem, items))
        })
        .collect::<Result<_, CatalogError>>()?;
    summary.soc_components = containers.components;
    summary.inventories_named = containers.inventory_stems().len();
    summary.inventories_in_archive = inventories.count();
    summary.inventories_unreadable = lists
        .values()
        .filter(|list| matches!(list, ItemList::Unreadable(_)))
        .count();
    summary.inventories_missing = lists
        .values()
        .filter(|list| **list == ItemList::Missing)
        .count();
    summary.item_ids_dropped = lists
        .values()
        .map(|list| match list {
            ItemList::Items { dropped, .. } => *dropped,
            ItemList::Missing | ItemList::Unreadable(_) => 0,
        })
        .sum();
    Ok(lists)
}

fn record_registry(source: EntrySource, disposition: SourceDisposition, ledger: &mut ExportLedger) {
    let diagnosis = if disposition == SourceDisposition::ProjectionFailed {
        shop_entry_read::diagnosis(
            ExportDiagnosticCode::MalformedSource,
            "ShopBackup.ID/SuperName",
        )
    } else {
        EntryDiagnosis::default()
    };
    source.diagnosed(ledger, Kind::Shops, disposition, diagnosis);
}

/// Returns the reasons of the unreadable lists of `lists`, ordered and without repeats.
fn unreadable_details(lists: &Lists<'_>) -> Vec<String> {
    let mut details: Vec<String> = lists
        .values()
        .filter_map(|list| match list {
            ItemList::Unreadable(reason) => Some(reason.clone()),
            ItemList::Items { .. } | ItemList::Missing => None,
        })
        .collect();
    details.sort_unstable();
    details.dedup();
    details
}

type Lists<'a> = HashMap<&'a str, ItemList>;

#[cfg(test)]
mod tests;
