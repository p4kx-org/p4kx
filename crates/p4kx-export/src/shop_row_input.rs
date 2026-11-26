// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ArchiveFiles;
use p4kx_extraction::{Archive, CancelToken};

use super::{Input, ShopRegistrySelection, ShopRowCoverage};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::DatasetAvailability;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_containers::{Component, Containers, Inventory};
use crate::shop_inventories::{Inventories, is_placeholder};
use crate::shop_inventory_path::InventoryStem;
use crate::shop_inventory_resolution::InventoryResolver;
use crate::shop_registry::{self, RegistryDocument, RegistryNeed, RegistryShop};
use crate::shop_summary::ShopSummary;
use crate::shops::shop_list::{self, PreparedShop, ShopPlacement};
use crate::shops::{Lists, ShopDeclarations, record_registry};
use crate::{CatalogError, ExportAccountingError, Kind, export_health_entries};

struct RegistryMatch {
    availability: DatasetAvailability,
    complete: bool,
    owner: Option<RegistryOwner>,
}

struct RegistryOwner {
    row: RegistryShop,
    placement: ShopPlacement,
}

pub(super) fn read(
    archive: &Archive,
    requested: ShopRegistrySelection,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
) -> Result<Input, CatalogError> {
    let mut ledger = ExportLedger::default();
    ledger.inventory(Kind::Shops);
    let mut summary = ShopSummary::default();
    let RegistryMatch {
        availability,
        complete,
        owner,
    } = registry(
        archive,
        requested,
        capture,
        cancel,
        &mut summary,
        &mut ledger,
    )?;
    let (shop, declarations_complete, inventory_payloads) = match owner {
        Some(owner) => {
            let declarations = ShopDeclarations::read(archive, cancel, capture, &mut summary)?;
            let complete = declarations.complete();
            let (shop, payloads) =
                prepare(archive, owner, declarations, capture, cancel, &mut ledger)?;
            (Some(shop), Some(complete), payloads)
        }
        None => (None, None, 0),
    };
    let entries = export_health_entries::compile(ledger.entries)?;
    let entries = export_health_entries::counts(Kind::Shops, &entries.sources)?;
    Ok(Input {
        shop,
        availability,
        coverage: ShopRowCoverage {
            registry_complete: complete,
            declarations_complete,
            inventory_payloads,
            names: None,
            entries,
        },
    })
}

fn registry(
    archive: &Archive,
    requested: ShopRegistrySelection,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    summary: &mut ShopSummary,
    ledger: &mut ExportLedger,
) -> Result<RegistryMatch, CatalogError> {
    let RegistryInput {
        document,
        availability,
        directory_complete,
    } = read_registry(archive, capture, summary, ledger)?;
    let Some(RegistryDocument {
        rows,
        source: original,
        complete: registry_complete,
    }) = document
    else {
        return Ok(RegistryMatch {
            availability,
            complete: matches!(availability, DatasetAvailability::Absent),
            owner: None,
        });
    };
    let selected_entry = original.receipt().entry_index() == Some(requested.entry_index as usize);
    let mut retained = None;
    let disposition = shop_list::visit_registry(&rows, summary, |raw, placement, _| {
        checkpoint(cancel)?;
        if selected_entry && raw.id_origin.matches_node(requested.node_index) {
            retained = Some(placement);
        }
        Ok(())
    })?;
    record_registry(original, disposition, ledger);
    let owner = retained
        .map(|placement| {
            let row = rows
                .into_iter()
                .find(|raw| raw.id_origin.matches_node(requested.node_index))
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            Ok::<_, CatalogError>(RegistryOwner { row, placement })
        })
        .transpose()?;
    Ok(RegistryMatch {
        availability,
        complete: directory_complete
            && registry_complete
            && disposition == SourceDisposition::Projected,
        owner,
    })
}

pub(super) struct RegistryInput {
    pub(super) document: Option<RegistryDocument>,
    pub(super) availability: DatasetAvailability,
    pub(super) directory_complete: bool,
}

pub(super) fn read_registry(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    summary: &mut ShopSummary,
    ledger: &mut ExportLedger,
) -> Result<RegistryInput, CatalogError> {
    let directory_complete = crate::export_archive_health::inventory(archive).is_none();
    let document = shop_registry::read_source(
        archive,
        &ArchiveFiles::new(archive),
        RegistryNeed::Optional,
        summary,
        ledger,
        capture,
    )?;
    let availability = if document.is_some() {
        DatasetAvailability::Available
    } else if summary.registry_missing && directory_complete {
        DatasetAvailability::Absent
    } else {
        DatasetAvailability::Unavailable
    };
    Ok(RegistryInput {
        document,
        availability,
        directory_complete,
    })
}

fn prepare(
    archive: &Archive,
    RegistryOwner { row, placement }: RegistryOwner,
    declarations: ShopDeclarations,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    ledger: &mut ExportLedger,
) -> Result<(PreparedShop, usize), CatalogError> {
    let complete = declarations.complete();
    let ShopDeclarations {
        containers,
        prefabs,
        ..
    } = declarations;
    let stem = selected_stem(&containers, &placement.entity_guid)?;
    let (mut inventories, inventory_payloads) = Inventories::prepare_shop(
        archive,
        stem.map(InventoryStem::as_str),
        capture,
        ledger,
        cancel,
    )?;
    let mut lists = Lists::new();
    if let Some(stem) = stem {
        lists.insert(stem.as_str(), inventories.items(stem, ledger)?);
    }
    let resolver = InventoryResolver::new(
        &containers,
        &inventories,
        &lists,
        &prefabs,
        capture,
        complete,
    );
    let resolution = resolver.row(&row, &placement.key, &placement.entity_guid)?;
    let cells = resolver.compatibility(&resolution, &placement.entity_guid)?;
    let prepared = shop_list::prepare(&row, placement, &containers, cells, capture.source_uses())?;
    ledger.merge(containers.ledger);
    ledger.merge(prefabs.ledger);
    Ok((prepared, inventory_payloads))
}

fn selected_stem<'source>(
    containers: &'source Containers,
    entity: &str,
) -> Result<Option<&'source InventoryStem>, CatalogError> {
    let Inventory::One(wanted) = containers.inventory(entity) else {
        return Ok(None);
    };
    if is_placeholder(wanted) {
        return Ok(None);
    }
    let (components, _) = containers
        .inventory_observations(entity)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let found = components
        .iter()
        .find_map(|component| match component {
            Component::File(stem) if stem.as_str() == wanted => Some(stem),
            _ => None,
        })
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    Ok(Some(found))
}
