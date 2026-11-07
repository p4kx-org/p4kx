// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::ErrorKind;

use p4kx_extraction::{Archive, Entry};

use crate::Kind;
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::soc_preset::PresetScan;

use super::harvest_locations::Soc;

/// Reads the first SOC's preset and records each actually consumed Archive Entry.
pub(crate) fn read(archive: &Archive, entry: usize) -> (Soc, ExportLedger) {
    let mut ledger = ExportLedger::default();
    let Some(socpak) = archive.entries().get(entry) else {
        return (Soc::Unreadable("no such entry"), ledger);
    };
    let source = EntrySource::outer(socpak);
    let Ok(container) = archive.open_container(socpak) else {
        source.record(
            &mut ledger,
            Kind::HarvestableLocations,
            SourceDisposition::ReadFailed,
        );
        return (Soc::Unreadable("cannot be opened as a container"), ledger);
    };
    source.record(
        &mut ledger,
        Kind::HarvestableLocations,
        SourceDisposition::Projected,
    );
    let soc = scan(&container, socpak, &mut ledger);
    (soc, ledger)
}

fn scan(container: &Archive, socpak: &Entry, ledger: &mut ExportLedger) -> Soc {
    let Some(soc) = crate::container_read::soc_entries(container).next() else {
        return Soc::Plain;
    };
    let source = EntrySource::inner(socpak, soc).operation(EntryOperation::SocPreset);
    let Ok(reader) = container.open_entry(soc) else {
        source.record(
            ledger,
            Kind::HarvestableLocations,
            SourceDisposition::ReadFailed,
        );
        return Soc::Unreadable("its .soc cannot be opened");
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
    soc
}

#[cfg(test)]
mod tests;
