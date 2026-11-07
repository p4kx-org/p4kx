// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken};

use crate::Kind;
use crate::container_read::{self, MAX_SOC_LEN};
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::loot_soc::EntityTags;
use crate::loot_unreadable::Unreadable;

/// Counts each selected SOC and retains every actual source operation.
pub(crate) fn read(
    archive: &Archive,
    entry: usize,
    cancel: Option<&CancelToken>,
) -> (Result<EntityTags, Unreadable>, ExportLedger) {
    let mut ledger = ExportLedger::default();
    let Some(socpak) = archive.entries().get(entry) else {
        return (Err(Unreadable::NoSuchEntry), ledger);
    };
    let source = EntrySource::outer(socpak);
    let Ok(container) = archive.open_container(socpak) else {
        source.record(
            &mut ledger,
            Kind::LootContainers,
            SourceDisposition::ReadFailed,
        );
        return (Err(Unreadable::NotAContainer), ledger);
    };
    source.record(
        &mut ledger,
        Kind::LootContainers,
        SourceDisposition::Projected,
    );
    let mut counts = EntityTags::default();
    let mut failure = None;
    for soc in container_read::soc_entries(&container) {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            break;
        }
        let source = EntrySource::inner(socpak, soc).operation(EntryOperation::SocTags);
        let Some(bytes) = container_read::read_limited(&container, soc, MAX_SOC_LEN) else {
            source.record(
                &mut ledger,
                Kind::LootContainers,
                SourceDisposition::ReadFailed,
            );
            failure.get_or_insert(Unreadable::SocFile);
            continue;
        };
        match counts.add_soc(&bytes) {
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
    (failure.map_or(Ok(counts), Err), ledger)
}

#[cfg(test)]
mod tests;
