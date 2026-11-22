// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::ChunkFile;
use p4kx_extraction::{Archive, CancelToken, Entry, FieldOrigin, SourceUse, SourceUseRole};

use crate::catalog_read::original::{EntryCapture, EntryRead};
use crate::container_read;
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::loot_soc::{ENTITIES, EntityTags};
use crate::loot_unreadable::Unreadable;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) struct Counted {
    pub(crate) tags: Result<EntityTags, Unreadable>,
    pub(crate) origin: FieldOrigin,
}

impl Counted {
    fn new(
        tags: Result<EntityTags, Unreadable>,
        uses: &[SourceUse],
        capture: &EntryCapture<'_>,
    ) -> Self {
        Self {
            origin: capture.source_uses().origin(uses, tags.is_ok()),
            tags,
        }
    }
}

/// Counts each selected SOC and retains its original directory and consumed payload regions.
pub(crate) fn read(
    archive: &Archive,
    entry: usize,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<(Counted, ExportLedger), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let mut ledger = ExportLedger::default();
    let Some(socpak) = archive.entries().get(entry) else {
        return Ok((
            Counted::new(Err(Unreadable::NoSuchEntry), &[], capture),
            ledger,
        ));
    };
    let source = EntrySource::outer(capture, archive, socpak)?;
    source.receipt().attempted_payload()?;
    let Ok(container) = archive.open_container(socpak) else {
        source.record(
            &mut ledger,
            Kind::LootContainers,
            SourceDisposition::ReadFailed,
        );
        return Ok((
            Counted::new(Err(Unreadable::NotAContainer), &[], capture),
            ledger,
        ));
    };
    source.record(
        &mut ledger,
        Kind::LootContainers,
        SourceDisposition::Projected,
    );
    let counted = read_members(archive, socpak, &container, cancel, capture, &mut ledger)?;
    Ok((counted, ledger))
}

fn read_members(
    archive: &Archive,
    outer: &Entry,
    container: &Archive,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
) -> Result<Counted, CatalogError> {
    let directory = container_read::observe_container_directory(container, capture.source_uses())?;
    let mut uses = Vec::new();
    let mut failure = if let Some(directory) = directory {
        push_use(&mut uses, directory)?;
        None
    } else {
        ledger.incomplete_inventory(Kind::LootContainers);
        Some(Unreadable::IncompleteDirectory)
    };
    let mut counts = EntityTags::default();
    for soc in container_read::soc_entries(container) {
        let original = EntrySource::inner(capture, archive, outer, container, soc)?
            .operation(EntryOperation::SocTags);
        let observed = read_soc(
            container,
            soc,
            &original,
            cancel,
            capture,
            &mut counts,
            &mut uses,
        )?;
        let disposition = match observed {
            Ok(()) => SourceDisposition::Projected,
            Err(reason) => {
                failure.get_or_insert(reason);
                if matches!(reason, Unreadable::SocFile) {
                    SourceDisposition::ReadFailed
                } else {
                    SourceDisposition::ProjectionFailed
                }
            }
        };
        original.record(ledger, Kind::LootContainers, disposition);
    }
    Ok(Counted::new(
        failure.map_or(Ok(counts), Err),
        &uses,
        capture,
    ))
}

fn read_soc(
    container: &Archive,
    soc: &Entry,
    source: &EntrySource,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
    counts: &mut EntityTags,
    uses: &mut Vec<SourceUse>,
) -> Result<Result<(), Unreadable>, CatalogError> {
    let Ok(bytes) = EntryRead::read_verified(source.receipt().clone(), container, soc, cancel)?
    else {
        return Ok(Err(Unreadable::SocFile));
    };
    let file = match counts.count_soc(bytes.bytes()) {
        Ok(file) => file,
        Err(error) => return Ok(Err(Unreadable::Soc(error))),
    };
    retain_inputs(&bytes, &file, capture, uses)?;
    Ok(Ok(()))
}

fn retain_inputs(
    bytes: &EntryRead,
    file: &ChunkFile<&[u8]>,
    capture: &EntryCapture<'_>,
    uses: &mut Vec<SourceUse>,
) -> Result<(), CatalogError> {
    for (range, namespace, field) in [
        (file.header_range(), "cryEngineChunkHeader", "layout"),
        (file.table_range(), "cryEngineChunkTable", "rows"),
    ] {
        push_use(
            uses,
            bytes.observe_payload_range(
                capture,
                range,
                namespace,
                field,
                SourceUseRole::Selector,
            )?,
        )?;
    }
    for (_, chunk) in file.chunks_of(ENTITIES) {
        let start = chunk.offset();
        let end = start
            .checked_add(chunk.size())
            .ok_or(ExportAccountingError::CounterOverflow)?;
        push_use(
            uses,
            bytes.observe_payload_range(
                capture,
                start..end,
                "socEntityChunk",
                "xml",
                SourceUseRole::Payload,
            )?,
        )?;
    }
    Ok(())
}

fn push_use(uses: &mut Vec<SourceUse>, observed: SourceUse) -> Result<(), CatalogError> {
    uses.try_reserve(1)
        .map_err(CatalogError::SourceUseAllocation)?;
    uses.push(observed);
    Ok(())
}

#[cfg(test)]
mod tests;
