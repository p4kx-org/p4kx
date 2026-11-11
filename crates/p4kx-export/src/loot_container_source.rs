// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Entry};

use crate::catalog_read::original::{EntryCapture, EntryDiagnosis, EntryRead};
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::loot_entxml::{self, CapturedEntityLink};
use crate::loot_unreadable::Unreadable;
use crate::starmap_index;
use crate::{CatalogError, Kind};

const MAX_ENTXML_LEN: u64 = 1 << 20;

/// The actual source operations and readable entity links of one container.
#[derive(Default)]
pub(crate) struct Read {
    pub(crate) links: Vec<(String, CapturedEntityLink)>,
    pub(crate) unreadable: Option<Unreadable>,
    pub(crate) ledger: ExportLedger,
}

/// Reads each selected entity file once and retains read and XML projection failures.
pub(crate) fn read(
    archive: &Archive,
    entry: usize,
    capture: &EntryCapture<'_>,
) -> Result<Read, CatalogError> {
    let mut read = Read::default();
    let Some(socpak) = archive.entries().get(entry) else {
        read.unreadable = Some(Unreadable::NoSuchEntry);
        return Ok(read);
    };
    let source = EntrySource::outer(capture, archive, socpak)?;
    source.receipt().attempted_payload()?;
    let Ok(container) = archive.open_container(socpak) else {
        source.record(
            &mut read.ledger,
            Kind::LootContainers,
            SourceDisposition::ReadFailed,
        );
        read.unreadable = Some(Unreadable::NotAContainer);
        return Ok(read);
    };
    source.record(
        &mut read.ledger,
        Kind::LootContainers,
        SourceDisposition::Projected,
    );
    let entity_files = container
        .entries()
        .iter()
        .filter(|inner| is_entity_file(inner.path()));
    for inner in entity_files {
        read_entity(archive, &container, socpak, inner, &mut read, capture)?;
    }
    Ok(read)
}

fn read_entity(
    archive: &Archive,
    container: &Archive,
    socpak: &Entry,
    inner: &Entry,
    read: &mut Read,
    capture: &EntryCapture<'_>,
) -> Result<(), CatalogError> {
    let source = EntrySource::inner(capture, archive, socpak, container, inner)?
        .operation(EntryOperation::EntityLinks);
    let Ok(bytes) = EntryRead::read(source.receipt().clone(), container, inner, MAX_ENTXML_LEN)?
    else {
        source.record(
            &mut read.ledger,
            Kind::LootContainers,
            SourceDisposition::ReadFailed,
        );
        read.unreadable = Some(Unreadable::EntityFile);
        return Ok(());
    };
    match loot_entxml::read_original(&bytes, capture)? {
        Ok(link) => {
            if link.value().malformed_config {
                source.diagnosed(
                    &mut read.ledger,
                    Kind::LootContainers,
                    SourceDisposition::ProjectionFailed,
                    EntryDiagnosis::field(ExportDiagnosticCode::MalformedSource, "multiConfigRef"),
                );
            } else {
                source.record(
                    &mut read.ledger,
                    Kind::LootContainers,
                    SourceDisposition::Projected,
                );
            }
            read.links.push((inner.path().to_owned(), link));
        }
        Err(error) => {
            source.record(
                &mut read.ledger,
                Kind::LootContainers,
                SourceDisposition::ProjectionFailed,
            );
            read.unreadable = Some(Unreadable::EntityXml(error));
        }
    }
    Ok(())
}

fn is_entity_file(path: &str) -> bool {
    starmap_index::key(path)
        .replace('\\', "/")
        .contains("entdata/")
        && Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("entxml"))
}

#[cfg(test)]
mod tests;
