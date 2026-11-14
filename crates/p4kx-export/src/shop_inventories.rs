// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Archive, CancelToken, Entry};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::DatasetAvailability;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::schema_fields::every_field_required;
use crate::shop_entry_read;
pub(crate) use crate::shop_inventory_compatibility::ItemList;
use crate::shop_inventory_file::{
    InventoryFileEvidence, ReadInventory, RecordedItems, ShopInventoryDefinition,
};
use crate::shop_inventory_path::{FOLDER, InventoryStem};
use crate::shop_inventory_source_read as source;
use crate::starmap_index;
use crate::{CatalogError, Kind};

const PLACEHOLDERS: [&str; 2] = ["temp_outpost_shopinventory", "inv_cryastro_dummy"];

pub(crate) fn is_placeholder(stem: &str) -> bool {
    PLACEHOLDERS.contains(&stem)
}

/// Physical offline inventory definitions selected independently of the shop registry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShopInventoryTable {
    availability: DatasetAvailability,
    meaning: InventoryMeaning,
    pub(crate) rows: Vec<ShopInventoryDefinition>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum InventoryMeaning {
    OfflineDesignTime,
}

impl Default for ShopInventoryTable {
    fn default() -> Self {
        Self {
            availability: DatasetAvailability::Absent,
            meaning: InventoryMeaning::OfflineDesignTime,
            rows: Vec::new(),
        }
    }
}

pub(crate) struct PreparedInventories {
    pub(crate) table: ShopInventoryTable,
    pub(crate) ledger: ExportLedger,
    pub(crate) inventories: Inventories,
}

/// The compatibility lookup from the same read that produced each physical definition.
pub(crate) struct Inventories {
    count: usize,
    by_stem: HashMap<String, InventoryFiles>,
    complete_directory: bool,
}

#[derive(Default)]
struct InventoryFiles {
    selected: Option<RecordedItems>,
    physical: Vec<InventoryFileEvidence>,
}

impl Inventories {
    pub(crate) fn prepare(
        archive: &Archive,
        capture: &EntryCapture<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<PreparedInventories, CatalogError> {
        let mut ledger = ExportLedger::default();
        ledger.inventory(Kind::ShopInventories);
        let mut table = ShopInventoryTable::default();
        let mut by_stem: HashMap<String, InventoryFiles> = HashMap::new();
        for entry in archive.entries() {
            source::check_cancel(cancel)?;
            let Some(stem) = inventory_stem(entry) else {
                continue;
            };
            let read = ReadInventory::read(archive, entry, capture, cancel)?;
            let (definition, items, evidence) = read.retain(&mut ledger);
            source::push(&mut table.rows, definition)?;
            by_stem
                .try_reserve(1)
                .map_err(CatalogError::JsonSourceAllocation)?;
            let files = by_stem.entry(stem).or_default();
            source::push(&mut files.physical, evidence)?;
            files.selected = Some(items);
        }
        table.rows.sort_unstable_by(|left, right| {
            let left = &left.identity;
            let right = &right.identity;
            left.normalized_path
                .value()
                .cmp(&right.normalized_path.value())
                .then_with(|| left.entry_index.cmp(&right.entry_index))
        });
        let incomplete = crate::export_archive_health::inventory(archive).is_some();
        if incomplete {
            ledger.incomplete_inventory(Kind::ShopInventories);
        }
        table.availability = if table.rows.is_empty() {
            if incomplete {
                DatasetAvailability::Unavailable
            } else {
                DatasetAvailability::Absent
            }
        } else {
            DatasetAvailability::Available
        };
        Ok(PreparedInventories {
            table,
            ledger,
            inventories: Self {
                count: by_stem.len(),
                by_stem,
                complete_directory: !incomplete,
            },
        })
    }

    pub(crate) const fn count(&self) -> usize {
        self.count
    }

    pub(crate) fn files(&self, stem: &str) -> &[InventoryFileEvidence] {
        self.by_stem
            .get(stem)
            .map_or(&[], |files| files.physical.as_slice())
    }

    pub(crate) const fn complete_directory(&self) -> bool {
        self.complete_directory
    }

    pub(crate) fn items(
        &mut self,
        stem: &InventoryStem,
        ledger: &mut ExportLedger,
    ) -> Result<ItemList, CatalogError> {
        if let Some(items) = self
            .by_stem
            .get_mut(stem.as_str())
            .and_then(|files| files.selected.take())
        {
            Ok(items.selected(ledger))
        } else {
            missing(stem, ledger)
        }
    }

    #[cfg(test)]
    pub(crate) fn selected(
        archive: &Archive,
        stem: &InventoryStem,
        ledger: &mut ExportLedger,
        capture: &EntryCapture<'_>,
    ) -> Result<ItemList, CatalogError> {
        let Some(entry) = archive
            .entries()
            .iter()
            .rev()
            .find(|entry| inventory_stem(entry).as_deref() == Some(stem.as_str()))
        else {
            return missing(stem, ledger);
        };
        let read = ReadInventory::read(archive, entry, capture, None)?;
        Ok(read.items.selected(ledger))
    }
}

fn inventory_stem(entry: &Entry) -> Option<String> {
    let raw = entry.path();
    let is_json = raw
        .len()
        .checked_sub(".json".len())
        .and_then(|start| raw.get(start..))
        .is_some_and(|tail| tail.eq_ignore_ascii_case(".json"));
    if !is_json {
        return None;
    }
    let path = starmap_index::key(raw);
    let file = path.strip_prefix(FOLDER)?;
    let stem = file.strip_suffix(".json")?;
    (!stem.is_empty() && !stem.contains('\\')).then(|| stem.to_owned())
}

fn missing(stem: &InventoryStem, ledger: &mut ExportLedger) -> Result<ItemList, CatalogError> {
    stem.source()?
        .operation(EntryOperation::ShopInventory)
        .dependency()
        .diagnosed(
            ledger,
            Kind::Shops,
            SourceDisposition::ReadFailed,
            shop_entry_read::diagnosis(ExportDiagnosticCode::DanglingReference, "offlineInventory"),
        );
    Ok(ItemList::Missing)
}

#[cfg(test)]
#[path = "shop_inventory_projection_tests.rs"]
mod tests;
