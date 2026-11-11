// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::TestEntry;

use crate::catalog_read::original::{EntryRead, EntryXml};
use crate::entry_source::EntrySource;
use crate::export_ledger::ExportLedger;
use crate::shop_inventories::{Inventories, ItemList};
use crate::shop_inventory_name;
use crate::shop_inventory_path::InventoryStem;
use crate::{CatalogError, ExportAccountingError};

const SELECTOR: &str = "fixture_inventory_request.xml";

pub(super) fn request(stem: &str) -> TestEntry {
    TestEntry::stored(
        SELECTOR,
        format!("<Shop offlineInventory=\"Scripts/ShopInventories/{stem}.json\"/>"),
    )
}

pub(super) fn items(
    archive: &Archive,
    expected: &str,
    ledger: &mut ExportLedger,
) -> Result<ItemList, CatalogError> {
    crate::catalog_read::original::with_entry_capture(archive, |capture| {
        let entry = archive
            .entries()
            .iter()
            .find(|entry| entry.path() == SELECTOR)
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let source = EntrySource::outer(capture, archive, entry)?;
        let read = EntryRead::read(
            source.receipt().clone(),
            archive,
            entry,
            entry.uncompressed_size(),
        )?
        .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        let xml = EntryXml::parse(&read, capture)?
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        let path = xml
            .attribute(xml.root(), "offlineInventory")?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let value = shop_inventory_name::stem(path.as_str())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if value != expected {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let selected = InventoryStem::captured(capture, &path, value)?;
        Inventories::selected(archive, &selected, ledger, capture)
    })
}
