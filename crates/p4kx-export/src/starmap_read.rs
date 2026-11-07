// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;
use std::path::Path;

use p4kx_extraction::{Archive, Entry};

use crate::Kind;
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::starmap_index::{SOCPAK, key};

/// The largest container XML that is read, 128 MiB.
const MAX_XML_LEN: u64 = 128 << 20;

/// The main XML of a container.
pub(crate) struct MainXml {
    pub(crate) source: EntrySource,
    pub(crate) bytes: Vec<u8>,
    /// The path of the XML entry taken because none is named like the container.
    pub(crate) fallback: Option<String>,
}

/// Returns the first XML entry of `entries` that is no editor or metadata file.
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

/// Reads the main XML of the container held in `socpak`: the entry named like the container, else
/// the first XML entry that is no editor or metadata file.
pub(crate) fn main_xml(
    archive: &Archive,
    socpak: &Entry,
    ledger: &mut ExportLedger,
) -> Result<MainXml, String> {
    let source = EntrySource::outer(socpak).dependency();
    let container = match archive.open_container(socpak) {
        Ok(container) => container,
        Err(error) => {
            source.record(ledger, Kind::StarmapObjects, SourceDisposition::ReadFailed);
            return Err(format!("cannot be opened as a container: {error:#}"));
        }
    };
    source.record(ledger, Kind::StarmapObjects, SourceDisposition::Projected);
    let parent = EntrySource::outer(socpak)
        .dependency()
        .operation(EntryOperation::ContainerXml);
    match selected_xml(&container, socpak) {
        Ok((xml, fallback)) => {
            let source = EntrySource::inner(socpak, xml)
                .dependency()
                .operation(EntryOperation::ContainerXml);
            match read_xml(&container, xml) {
                Ok(bytes) => Ok(MainXml {
                    source,
                    bytes,
                    fallback,
                }),
                Err(error) => {
                    source.record(ledger, Kind::StarmapObjects, SourceDisposition::ReadFailed);
                    Err(error)
                }
            }
        }
        Err(error) => {
            parent.record(
                ledger,
                Kind::StarmapObjects,
                SourceDisposition::ProjectionFailed,
            );
            Err(error)
        }
    }
}

fn selected_xml<'a>(
    container: &'a Archive,
    socpak: &Entry,
) -> Result<(&'a Entry, Option<String>), String> {
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
    let fallback = named.is_none().then(|| xml.path().to_owned());
    Ok((xml, fallback))
}

fn read_xml(container: &Archive, xml: &Entry) -> Result<Vec<u8>, String> {
    if xml.uncompressed_size() > MAX_XML_LEN {
        return Err(format!(
            "its XML holds {} bytes, over the limit",
            xml.uncompressed_size()
        ));
    }
    let mut bytes = Vec::new();
    container
        .open_entry(xml)
        .map_err(|error| format!("its XML cannot be opened: {error:#}"))?
        .take(MAX_XML_LEN)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("its XML cannot be read: {error}"))?;
    Ok(bytes)
}
