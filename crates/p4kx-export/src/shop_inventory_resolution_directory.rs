// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashMap};

use super::{
    Archive, CancelToken, CatalogError, Entry, EntryCapture, ExportLedger, Inventories,
    InventoryFileEvidence, InventoryFiles, ReadInventory, inventory_stem, source,
};

impl Inventories {
    pub(crate) fn prepare_resolution(
        archive: &Archive,
        needed_bodies: &BTreeSet<&str>,
        capture: &EntryCapture<'_>,
        ledger: &mut ExportLedger,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, usize), CatalogError> {
        let occurrences = survey(archive, cancel)?;
        let mut by_stem = HashMap::new();
        by_stem
            .try_reserve(occurrences.len())
            .map_err(CatalogError::JsonSourceAllocation)?;
        let mut payloads = 0;
        for (stem, entries) in occurrences {
            source::check_cancel(cancel)?;
            let files = match entries.as_slice() {
                [entry] if needed_bodies.contains(stem.as_str()) => {
                    let read = ReadInventory::read(archive, entry, capture, cancel)?;
                    let (_, items, evidence) = read.retain(ledger);
                    let mut files = InventoryFiles::default();
                    source::push(&mut files.physical, evidence)?;
                    files.selected = Some(items);
                    payloads += 1;
                    files
                }
                entries => directory_files(entries, archive, capture, cancel)?,
            };
            by_stem.insert(stem, files);
        }
        Ok((
            Self {
                count: by_stem.len(),
                by_stem,
                complete_directory: crate::export_archive_health::inventory(archive).is_none(),
            },
            payloads,
        ))
    }
}

fn survey<'archive>(
    archive: &'archive Archive,
    cancel: Option<&CancelToken>,
) -> Result<HashMap<String, Vec<&'archive Entry>>, CatalogError> {
    let mut occurrences: HashMap<String, Vec<&Entry>> = HashMap::new();
    for entry in archive.entries() {
        source::check_cancel(cancel)?;
        if let Some(stem) = inventory_stem(entry) {
            occurrences
                .try_reserve(1)
                .map_err(CatalogError::JsonSourceAllocation)?;
            source::push(occurrences.entry(stem).or_default(), entry)?;
        }
    }
    Ok(occurrences)
}

fn directory_files(
    entries: &[&Entry],
    archive: &Archive,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
) -> Result<InventoryFiles, CatalogError> {
    let mut files = InventoryFiles::default();
    for entry in entries {
        source::check_cancel(cancel)?;
        source::push(
            &mut files.physical,
            InventoryFileEvidence::directory(archive, entry, capture)?,
        )?;
    }
    Ok(files)
}
