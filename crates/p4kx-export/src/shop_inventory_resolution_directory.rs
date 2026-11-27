// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashMap};

use super::{
    Archive, CancelToken, CatalogError, Entry, EntryCapture, ExportLedger, Inventories,
    InventoryFileEvidence, InventoryFiles, ReadInventory, directory_origin, inventory_stem, source,
};

impl Inventories {
    pub(crate) fn prepare_resolution(
        archive: &Archive,
        needed_bodies: &BTreeSet<&str>,
        capture: &EntryCapture<'_>,
        ledger: &mut ExportLedger,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, usize), CatalogError> {
        Self::prepare_selective(
            archive,
            capture,
            ledger,
            cancel,
            |stem, entries| match entries {
                [entry] if needed_bodies.contains(stem) => Some(*entry),
                _ => None,
            },
        )
    }

    pub(crate) fn prepare_shop(
        archive: &Archive,
        stem: Option<&str>,
        capture: &EntryCapture<'_>,
        ledger: &mut ExportLedger,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, usize), CatalogError> {
        Self::prepare_shops(
            archive,
            |group| stem == Some(group),
            capture,
            ledger,
            cancel,
        )
    }

    pub(crate) fn prepare_shops(
        archive: &Archive,
        includes: impl Fn(&str) -> bool,
        capture: &EntryCapture<'_>,
        ledger: &mut ExportLedger,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, usize), CatalogError> {
        Self::prepare_selective(archive, capture, ledger, cancel, |group, entries| {
            if includes(group) {
                entries.last().copied()
            } else {
                None
            }
        })
    }

    fn prepare_selective<'archive>(
        archive: &'archive Archive,
        capture: &EntryCapture<'_>,
        ledger: &mut ExportLedger,
        cancel: Option<&CancelToken>,
        select: impl Fn(&str, &[&'archive Entry]) -> Option<&'archive Entry>,
    ) -> Result<(Self, usize), CatalogError> {
        let occurrences = survey(archive, cancel)?;
        let mut by_stem = HashMap::new();
        by_stem
            .try_reserve(occurrences.len())
            .map_err(CatalogError::JsonSourceAllocation)?;
        let mut payloads = 0;
        for (stem, entries) in occurrences {
            let selected = select(stem.as_str(), &entries).map(Entry::index);
            let mut files = InventoryFiles::default();
            for entry in entries {
                source::check_cancel(cancel)?;
                let evidence = if selected == Some(entry.index()) {
                    let read = ReadInventory::read(archive, entry, capture, cancel)?;
                    let (_, items, evidence) = read.retain(ledger);
                    files.selected = Some(items);
                    payloads += 1;
                    evidence
                } else {
                    InventoryFileEvidence::directory(archive, entry, capture)?
                };
                source::push(&mut files.physical, evidence)?;
            }
            by_stem.insert(stem, files);
        }
        let complete_directory = crate::export_archive_health::inventory(archive).is_none();
        Ok((
            Self {
                count: by_stem.len(),
                by_stem,
                complete_directory,
                directory_origin: directory_origin(archive, capture, complete_directory)?,
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
