// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::ErrorKind;

use p4kx_extraction::{Archive, Entry};

use crate::catalog_read::original::EntryCapture;
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::soc_preset::PresetScan;
use crate::{CatalogError, Kind};

use super::harvest_locations::Soc;

/// Reads the first SOC's preset and records each actually consumed Archive Entry.
pub(crate) fn read(
    archive: &Archive,
    entry: usize,
    capture: &EntryCapture<'_>,
) -> Result<(Soc, ExportLedger), CatalogError> {
    let mut ledger = ExportLedger::default();
    let Some(socpak) = archive.entries().get(entry) else {
        return Ok((Soc::Unreadable("no such entry"), ledger));
    };
    let source = EntrySource::outer(capture, archive, socpak)?;
    source.receipt().attempted_payload()?;
    let Ok(container) = archive.open_container(socpak) else {
        source.record(
            &mut ledger,
            Kind::HarvestableLocations,
            SourceDisposition::ReadFailed,
        );
        return Ok((Soc::Unreadable("cannot be opened as a container"), ledger));
    };
    source.record(
        &mut ledger,
        Kind::HarvestableLocations,
        SourceDisposition::Projected,
    );
    let soc = scan(archive, &container, socpak, &mut ledger, capture)?;
    Ok((soc, ledger))
}

fn scan(
    archive: &Archive,
    container: &Archive,
    socpak: &Entry,
    ledger: &mut ExportLedger,
    capture: &EntryCapture<'_>,
) -> Result<Soc, CatalogError> {
    let Some(soc) = crate::container_read::soc_entries(container).next() else {
        return Ok(Soc::Plain);
    };
    let source = EntrySource::inner(capture, archive, socpak, container, soc)?
        .operation(EntryOperation::SocPreset);
    source.receipt().attempted_payload()?;
    let Ok(reader) = container.open_entry(soc) else {
        source.record(
            ledger,
            Kind::HarvestableLocations,
            SourceDisposition::ReadFailed,
        );
        return Ok(Soc::Unreadable("its .soc cannot be opened"));
    };
    let (soc, disposition) = match PresetScan::read(reader) {
        Ok(preset) => (
            preset.map_or(Soc::Plain, Soc::Preset),
            SourceDisposition::Projected,
        ),
        Err(error) if error.kind() == ErrorKind::FileTooLarge => (
            Soc::Unreadable("its .soc is over the scan limit"),
            SourceDisposition::ReadFailed,
        ),
        Err(_) => (
            Soc::Unreadable("its .soc cannot be read"),
            SourceDisposition::ReadFailed,
        ),
    };
    source.record(ledger, Kind::HarvestableLocations, disposition);
    Ok(soc)
}

#[cfg(test)]
mod tests;
