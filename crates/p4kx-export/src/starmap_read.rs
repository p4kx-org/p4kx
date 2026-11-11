// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Entry};

use crate::catalog_read::original::{EntryCapture, EntryRead, EntryReadError};
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::starmap_index::{SOCPAK, key};
use crate::{CatalogError, Kind};

/// The largest container XML that is read, 128 MiB.
const MAX_XML_LEN: u64 = 128 << 20;

/// The main XML paired to its original Entry read.
pub(crate) struct MainXml {
    pub(crate) source: EntrySource,
    pub(crate) bytes: EntryRead,
    pub(crate) fallback: Option<String>,
}

/// Returns the first XML entry that is no editor or metadata file.
fn first_xml(entries: &[Entry]) -> Result<&Entry, String> {
    entries
        .iter()
        .find(|inner| {
            let name = key(inner.path());
            Path::new(&name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
                && !name.contains("_editor")
                && !name.contains("metadata")
        })
        .ok_or_else(|| {
            let first: Vec<&str> = entries.iter().map(Entry::path).take(8).collect();
            format!("holds no main XML; its entries start with {first:?}")
        })
}

/// Reads the named main XML or its existing first eligible fallback through its original admission.
pub(crate) fn main_xml(
    archive: &Archive,
    socpak: &Entry,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
) -> Result<Result<MainXml, String>, CatalogError> {
    let source = EntrySource::outer(capture, archive, socpak)?.dependency();
    source.receipt().attempted_payload()?;
    let container = match archive.open_container(socpak) {
        Ok(container) => container,
        Err(error) => {
            source.record(ledger, Kind::StarmapObjects, SourceDisposition::ReadFailed);
            return Ok(Err(format!("cannot be opened as a container: {error:#}")));
        }
    };
    let parent = source.clone().operation(EntryOperation::ContainerXml);
    source.record(ledger, Kind::StarmapObjects, SourceDisposition::Projected);
    read_main(archive, socpak, &container, capture, parent, ledger)
}

fn read_main(
    archive: &Archive,
    socpak: &Entry,
    container: &Archive,
    capture: &EntryCapture<'_>,
    parent: EntrySource,
    ledger: &mut ExportLedger,
) -> Result<Result<MainXml, String>, CatalogError> {
    let (xml, fallback) = match selected_xml(container, socpak) {
        Ok(selected) => selected,
        Err(error) => {
            parent.record(
                ledger,
                Kind::StarmapObjects,
                SourceDisposition::ProjectionFailed,
            );
            return Ok(Err(error));
        }
    };
    let source = EntrySource::inner(capture, archive, socpak, container, xml)?
        .dependency()
        .operation(EntryOperation::ContainerXml);
    match EntryRead::read(source.receipt().clone(), container, xml, MAX_XML_LEN)? {
        Ok(bytes) => Ok(Ok(MainXml {
            source,
            bytes,
            fallback,
        })),
        Err(error) => {
            source.record(ledger, Kind::StarmapObjects, SourceDisposition::ReadFailed);
            Ok(Err(read_failure(xml, error)))
        }
    }
}

fn selected_xml<'archive>(
    container: &'archive Archive,
    socpak: &Entry,
) -> Result<(&'archive Entry, Option<String>), String> {
    let stem = key(socpak.path());
    let stem = stem
        .rsplit('\\')
        .next()
        .and_then(|name| name.strip_suffix(SOCPAK))
        .ok_or("not a socpak name")?;
    let main = format!("{stem}.xml");
    let named = container
        .entries()
        .iter()
        .find(|inner| key(inner.path()).rsplit('\\').next() == Some(main.as_str()));
    let xml = match named {
        Some(xml) => xml,
        None => first_xml(container.entries())?,
    };
    Ok((xml, named.is_none().then(|| xml.path().to_owned())))
}

fn read_failure(xml: &Entry, error: EntryReadError) -> String {
    match error {
        EntryReadError::Limit { .. } if xml.uncompressed_size() > MAX_XML_LEN => format!(
            "its XML holds {} bytes, over the limit",
            xml.uncompressed_size()
        ),
        EntryReadError::Limit { .. } => "its XML exceeds the read limit".to_owned(),
        EntryReadError::Open(error) => format!("its XML cannot be opened: {error:#}"),
        EntryReadError::Read(error) => format!("its XML cannot be read: {error}"),
    }
}
