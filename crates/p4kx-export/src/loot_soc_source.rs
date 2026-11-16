// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken};

use crate::catalog_read::original::{EntryCapture, EntryRead};
use crate::container_read;
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::loot_soc::EntityTags;
use crate::loot_unreadable::Unreadable;
use crate::{CatalogError, Kind};

/// Counts each selected SOC and retains every actual source operation.
pub(crate) fn read(
    archive: &Archive,
    entry: usize,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<(Result<EntityTags, Unreadable>, ExportLedger), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let mut ledger = ExportLedger::default();
    let Some(socpak) = archive.entries().get(entry) else {
        return Ok((Err(Unreadable::NoSuchEntry), ledger));
    };
    let source = EntrySource::outer(capture, archive, socpak)?;
    source.receipt().attempted_payload()?;
    let Ok(container) = archive.open_container(socpak) else {
        source.record(
            &mut ledger,
            Kind::LootContainers,
            SourceDisposition::ReadFailed,
        );
        return Ok((Err(Unreadable::NotAContainer), ledger));
    };
    source.record(
        &mut ledger,
        Kind::LootContainers,
        SourceDisposition::Projected,
    );
    let mut counts = EntityTags::default();
    let mut failure = None;
    for soc in container_read::soc_entries(&container) {
        let source = EntrySource::inner(capture, archive, socpak, &container, soc)?
            .operation(EntryOperation::SocTags);
        let Ok(bytes) = EntryRead::read_all(source.receipt().clone(), &container, soc, cancel)?
        else {
            source.record(
                &mut ledger,
                Kind::LootContainers,
                SourceDisposition::ReadFailed,
            );
            failure.get_or_insert(Unreadable::SocFile);
            continue;
        };
        match counts.add_soc(bytes.bytes()) {
            Ok(()) => source.record(
                &mut ledger,
                Kind::LootContainers,
                SourceDisposition::Projected,
            ),
            Err(error) => {
                source.record(
                    &mut ledger,
                    Kind::LootContainers,
                    SourceDisposition::ProjectionFailed,
                );
                failure.get_or_insert(Unreadable::Soc(error));
            }
        }
    }
    Ok((failure.map_or(Ok(counts), Err), ledger))
}

#[cfg(test)]
mod tests;
