// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Entry};

use crate::Kind;
use crate::container_read;
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDiagnosis, SourceDisposition};
use crate::loot_entxml::{self, EntityLink};
use crate::loot_unreadable::Unreadable;
use crate::starmap_index;

const MAX_ENTXML_LEN: u64 = 1 << 20;

/// The actual source operations and readable entity links of one container.
#[derive(Default)]
pub(crate) struct Read {
    pub(crate) links: Vec<(String, EntityLink)>,
    pub(crate) unreadable: Option<Unreadable>,
    pub(crate) ledger: ExportLedger,
}

/// Reads each selected entity file once and retains read and XML projection failures.
pub(crate) fn read(archive: &Archive, entry: usize) -> Read {
    let mut read = Read::default();
    let Some(socpak) = archive.entries().get(entry) else {
        read.unreadable = Some(Unreadable::NoSuchEntry);
        return read;
    };
    let source = EntrySource::outer(socpak);
    let Ok(container) = archive.open_container(socpak) else {
        source.record(
            &mut read.ledger,
            Kind::LootContainers,
            SourceDisposition::ReadFailed,
        );
        read.unreadable = Some(Unreadable::NotAContainer);
        return read;
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
        read_entity(&container, socpak, inner, &mut read);
    }
    read
}

fn read_entity(container: &Archive, socpak: &Entry, inner: &Entry, read: &mut Read) {
    let source = EntrySource::inner(socpak, inner).operation(EntryOperation::EntityLinks);
    let Some(bytes) = container_read::read_limited(container, inner, MAX_ENTXML_LEN) else {
        source.record(
            &mut read.ledger,
            Kind::LootContainers,
            SourceDisposition::ReadFailed,
        );
        read.unreadable = Some(Unreadable::EntityFile);
        return;
    };
    match loot_entxml::read(&bytes) {
        Ok(link) => {
            if link.malformed_config {
                source.diagnosed(
                    &mut read.ledger,
                    Kind::LootContainers,
                    SourceDisposition::ProjectionFailed,
                    SourceDiagnosis {
                        code: Some(ExportDiagnosticCode::MalformedSource),
                        field: Some("multiConfigRef".to_owned()),
                        ..SourceDiagnosis::default()
                    },
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
