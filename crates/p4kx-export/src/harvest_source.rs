// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::io;

use p4kx_extraction::{Archive, CancelToken, Entry, EntryReader, Guid, SourcedValue};

use crate::catalog_read::original::EntryCapture;
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::soc_preset::{PresetMatch, PresetScan};
use crate::{CatalogError, Kind};

use super::harvest_locations::Soc;

#[derive(Clone, Copy)]
enum ReadMode<'providers> {
    FirstMatch,
    Verified(&'providers HashSet<Guid>),
}

enum Match {
    First(Guid),
    Verified(PresetMatch),
}

pub(crate) struct SocRead {
    pub(crate) fact: Soc,
    pub(crate) selected: Option<SourcedValue<Guid>>,
}

impl From<Soc> for SocRead {
    fn from(fact: Soc) -> Self {
        Self {
            fact,
            selected: None,
        }
    }
}

impl ReadMode<'_> {
    fn open<'archive>(
        self,
        container: &'archive Archive,
        entry: &Entry,
    ) -> Result<EntryReader<'archive>, p4kx_extraction::ArchiveError> {
        match self {
            Self::FirstMatch => container.open_entry(entry),
            Self::Verified(_) => container
                .open_entry_verified(entry)
                .map(EntryReader::reject_overlong),
        }
    }

    fn scan(
        self,
        reader: EntryReader<'_>,
        cancel: Option<&CancelToken>,
    ) -> io::Result<Option<Match>> {
        match self {
            Self::FirstMatch => {
                PresetScan::read(reader, cancel).map(|found| found.map(Match::First))
            }
            Self::Verified(_) => {
                PresetScan::read_to_end(reader, cancel).map(|found| found.map(Match::Verified))
            }
        }
    }
}

/// Reads the first SOC's preset and records each actually consumed Archive Entry.
pub(crate) fn read(
    archive: &Archive,
    entry: usize,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(Soc, ExportLedger), CatalogError> {
    let (observed, ledger) = read_with(archive, entry, capture, cancel, ReadMode::FirstMatch)?;
    Ok((observed.fact, ledger))
}

/// Checks the selected SOC through EOF and binds the admitted preset's decoded token ranges.
pub(crate) fn read_verified(
    archive: &Archive,
    entry: usize,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    presets: &HashSet<Guid>,
) -> Result<(SocRead, ExportLedger), CatalogError> {
    read_with(archive, entry, capture, cancel, ReadMode::Verified(presets))
}

fn read_with(
    archive: &Archive,
    entry: usize,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    mode: ReadMode<'_>,
) -> Result<(SocRead, ExportLedger), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let mut ledger = ExportLedger::default();
    let Some(socpak) = archive.entries().get(entry) else {
        return Ok((Soc::Unreadable("no such entry").into(), ledger));
    };
    let source = EntrySource::outer(capture, archive, socpak)?;
    source.receipt().attempted_payload()?;
    let Ok(container) = archive.open_container(socpak) else {
        source.record(
            &mut ledger,
            Kind::HarvestableLocations,
            SourceDisposition::ReadFailed,
        );
        return Ok((
            Soc::Unreadable("cannot be opened as a container").into(),
            ledger,
        ));
    };
    source.record(
        &mut ledger,
        Kind::HarvestableLocations,
        SourceDisposition::Projected,
    );
    let observed = scan(
        archive,
        &container,
        socpak,
        &mut ledger,
        capture,
        cancel,
        mode,
    )?;
    Ok((observed, ledger))
}

fn scan(
    archive: &Archive,
    container: &Archive,
    socpak: &Entry,
    ledger: &mut ExportLedger,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    mode: ReadMode<'_>,
) -> Result<SocRead, CatalogError> {
    let Some(soc) = crate::container_read::soc_entries(container).next() else {
        return Ok(Soc::Plain.into());
    };
    let source = EntrySource::inner(capture, archive, socpak, container, soc)?
        .operation(EntryOperation::SocPreset);
    source.receipt().attempted_payload()?;
    let Ok(reader) = mode.open(container, soc) else {
        source.record(
            ledger,
            Kind::HarvestableLocations,
            SourceDisposition::ReadFailed,
        );
        return Ok(Soc::Unreadable("its .soc cannot be opened").into());
    };
    let read = mode.scan(reader, cancel);
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let (observed, disposition) = match read {
        Err(_) => (
            Soc::Unreadable("its .soc cannot be read").into(),
            SourceDisposition::ReadFailed,
        ),
        Ok(matched) => (
            selected_preset(matched, &source, capture, mode)?,
            SourceDisposition::Projected,
        ),
    };
    source.record(ledger, Kind::HarvestableLocations, disposition);
    Ok(observed)
}

fn selected_preset(
    matched: Option<Match>,
    source: &EntrySource,
    capture: &EntryCapture<'_>,
    mode: ReadMode<'_>,
) -> Result<SocRead, CatalogError> {
    Ok(match matched {
        None => Soc::Plain.into(),
        Some(Match::First(guid)) => Soc::Preset(guid).into(),
        Some(Match::Verified(matched)) => {
            let fact = Soc::Preset(matched.guid);
            let selected = match mode {
                ReadMode::Verified(presets) if presets.contains(&matched.guid) => {
                    Some(source.receipt().harvest_preset(capture, matched)?)
                }
                _ => None,
            };
            SocRead { fact, selected }
        }
    })
}

#[cfg(test)]
mod tests;
