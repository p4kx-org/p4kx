// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{
    Archive, CancelToken, Entry, InputScope, SourceInput, SourceLocation, SourceUseRole,
};

use crate::catalog_read::original::{EntryCapture, EntryDiagnosis, EntryRead};
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::loot_entxml::{self, CapturedEntityLink};
use crate::loot_unreadable::Unreadable;
use crate::starmap_index;
use crate::{CatalogError, ExportAccountingError, Kind};

/// The actual source operations and readable entity links of one container.
#[derive(Default)]
pub(crate) struct Read {
    pub(crate) links: Vec<(String, CapturedEntityLink)>,
    pub(crate) unreadable: Option<Unreadable>,
    pub(crate) original_scope_unknown: bool,
    pub(crate) ledger: ExportLedger,
}

/// Reads each selected entity file once and retains read and XML projection failures.
pub(crate) fn read(
    archive: &Archive,
    entry: usize,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
    entity_index: Option<usize>,
) -> Result<Read, CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
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
    read.original_scope_unknown = crate::export_archive_health::inventory(&container).is_some();
    let mut read_member = |inner| {
        read_entity(
            archive, &container, socpak, inner, &mut read, cancel, capture,
        )
    };
    if let Some(wanted) = entity_index {
        if let Some(inner) = exact_entity(&container, wanted, capture)? {
            read_member(inner)?;
        }
    } else {
        for inner in container
            .entries()
            .iter()
            .filter(|inner| is_entity_file(inner.path()))
        {
            read_member(inner)?;
        }
    }
    Ok(read)
}

fn exact_entity<'archive>(
    container: &'archive Archive,
    wanted: usize,
    capture: &EntryCapture<'_>,
) -> Result<Option<&'archive Entry>, CatalogError> {
    let sources = capture.source_uses();
    let Some(inner) = container.entries().get(wanted) else {
        let outer = container
            .container_input_origin()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        capture.check_origin(&outer)?;
        let scope = sources.observe(SourceInput::Archive(&outer), InputScope::Payload)?;
        let count = u64::try_from(container.entries().len())
            .map_err(|_| ExportAccountingError::CounterOverflow)?;
        scope.at(
            SourceLocation::Subresource {
                namespace: "socArchiveDirectory".to_owned(),
                indices: vec![0, count],
                field: "entryCount".to_owned(),
            },
            SourceUseRole::Selector,
        )?;
        return Ok(None);
    };
    if inner.index() != wanted {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    if is_entity_file(inner.path()) {
        return Ok(Some(inner));
    }
    let view = container
        .observed_entry(inner)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    capture.check_archive(view.input_origin())?;
    let scope = sources.observe(
        SourceInput::ArchiveView(view),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    scope.at(
        SourceLocation::Metadata {
            field: "path".to_owned(),
        },
        SourceUseRole::Selector,
    )?;
    Ok(None)
}

fn read_entity(
    archive: &Archive,
    container: &Archive,
    socpak: &Entry,
    inner: &Entry,
    read: &mut Read,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<(), CatalogError> {
    let source = EntrySource::inner(capture, archive, socpak, container, inner)?
        .operation(EntryOperation::EntityLinks);
    let Ok(bytes) = EntryRead::read_verified(source.receipt().clone(), container, inner, cancel)?
    else {
        source.record(
            &mut read.ledger,
            Kind::LootContainers,
            SourceDisposition::ReadFailed,
        );
        read.unreadable = Some(Unreadable::EntityFile);
        return Ok(());
    };
    match loot_entxml::read_original(&bytes, capture, cancel)? {
        Ok(link) => {
            read.original_scope_unknown |= !link.original_complete();
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
