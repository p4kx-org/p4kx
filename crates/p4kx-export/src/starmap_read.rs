// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, CancelToken, Entry, SourceUse};

use crate::catalog_read::original::{EntryCapture, EntryRead, EntryReadError};
use crate::container_read;
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::starmap_index::{SOCPAK, key};
use crate::{CatalogError, Kind};

/// The largest container XML that is read, 128 MiB.
const MAX_XML_LEN: u64 = 128 << 20;

/// The main XML paired to its original Entry read.
pub(crate) struct MainXml {
    pub(crate) container: Archive,
    pub(crate) source: EntrySource,
    pub(crate) bytes: EntryRead,
    pub(crate) fallback: Option<String>,
}

#[derive(Default)]
pub(crate) struct DescriptorInputs {
    pub(crate) directory: Option<SourceUse>,
    pub(crate) payload: Option<DescriptorPayload>,
}

pub(crate) enum DescriptorPayload {
    Parsed(SourceUse),
    Attempted(SourceUse),
}

pub(crate) struct DescriptorSurveyRead<'survey> {
    pub(crate) inputs: &'survey mut DescriptorInputs,
    pub(crate) cancel: Option<&'survey CancelToken>,
}

/// Returns the normalized path of a main placement XML candidate.
fn placement_xml_path(entry: &Entry) -> Option<String> {
    let path = key(entry.path());
    let mut components = path.rsplit('\\');
    let _file = components.next();
    (!components.any(|component| component == "hcd")).then_some(path)
}

/// Returns the first eligible fallback XML Entry.
fn first_xml(entries: &[Entry]) -> Result<&Entry, String> {
    entries
        .iter()
        .find(|inner| {
            placement_xml_path(inner).is_some_and(|name| {
                Path::new(&name)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
                    && !name.contains("_editor")
                    && !name.contains("metadata")
            })
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
    main_xml_with_survey(archive, socpak, capture, ledger, None)
}

pub(crate) fn main_xml_with_survey(
    archive: &Archive,
    socpak: &Entry,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    survey: Option<DescriptorSurveyRead<'_>>,
) -> Result<Result<MainXml, String>, CatalogError> {
    let source = EntrySource::outer(capture, archive, socpak)?.dependency();
    let attempt = source.receipt().payload_attempt()?;
    let container = match archive.open_container(socpak) {
        Ok(container) => container,
        Err(error) => {
            if let Some(survey) = survey {
                survey.inputs.payload = Some(DescriptorPayload::Attempted(attempt));
            }
            source.record(ledger, Kind::StarmapObjects, SourceDisposition::ReadFailed);
            return Ok(Err(format!("cannot be opened as a container: {error:#}")));
        }
    };
    let parent = source.clone().operation(EntryOperation::ContainerXml);
    source.record(ledger, Kind::StarmapObjects, SourceDisposition::Projected);
    read_main(archive, socpak, container, capture, parent, ledger, survey)
}

fn read_main(
    archive: &Archive,
    socpak: &Entry,
    container: Archive,
    capture: &EntryCapture<'_>,
    parent: EntrySource,
    ledger: &mut ExportLedger,
    mut survey: Option<DescriptorSurveyRead<'_>>,
) -> Result<Result<MainXml, String>, CatalogError> {
    let selected = selected_xml(&container, socpak);
    if let Some(survey) = &mut survey {
        survey.inputs.directory =
            container_read::observe_container_directory(&container, capture.source_uses())?;
    }
    let (xml, fallback) = match selected {
        Ok(Descriptor::Named(xml)) => (xml, None),
        Ok(Descriptor::Fallback(xml)) => (xml, Some(xml.path().to_owned())),
        Ok(Descriptor::Ambiguous(candidates)) => {
            for candidate in candidates {
                EntrySource::inner(capture, archive, socpak, &container, candidate)?
                    .record_ambiguous_candidate(ledger, Kind::StarmapObjects, "mainXml", None)?;
            }
            return Ok(Err(
                "several physical entries qualify as the main XML".to_owned()
            ));
        }
        Err(error) => {
            parent.record(
                ledger,
                Kind::StarmapObjects,
                SourceDisposition::ProjectionFailed,
            );
            return Ok(Err(error));
        }
    };
    let source = EntrySource::inner(capture, archive, socpak, &container, xml)?
        .dependency()
        .operation(EntryOperation::ContainerXml);
    match read_descriptor(&container, xml, &source, survey)? {
        Ok(bytes) => Ok(Ok(MainXml {
            container,
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

fn read_descriptor(
    container: &Archive,
    xml: &Entry,
    source: &EntrySource,
    survey: Option<DescriptorSurveyRead<'_>>,
) -> Result<Result<EntryRead, EntryReadError>, CatalogError> {
    let receipt = source.receipt();
    let Some(survey) = survey else {
        return EntryRead::read_bounded_complete(
            receipt.clone(),
            container,
            xml,
            MAX_XML_LEN,
            None,
        );
    };
    let read = EntryRead::read_bounded_verified(
        receipt.clone(),
        container,
        xml,
        MAX_XML_LEN,
        survey.cancel,
    )?;
    if let Err(EntryReadError::Open(_) | EntryReadError::Read(_)) = &read {
        survey.inputs.payload = Some(DescriptorPayload::Attempted(receipt.payload_attempt()?));
    }
    Ok(read)
}

enum Descriptor<'archive> {
    Named(&'archive Entry),
    Fallback(&'archive Entry),
    Ambiguous(Vec<&'archive Entry>),
}

fn selected_xml<'archive>(
    container: &'archive Archive,
    socpak: &Entry,
) -> Result<Descriptor<'archive>, String> {
    let mut named = named_descriptors(container, socpak)?;
    if let Some(first) = named.next() {
        return Ok(choose_descriptor(first, named, Descriptor::Named));
    }
    let first = first_xml(container.entries())?;
    let wanted = key(first.path());
    let same_path = container
        .entries()
        .iter()
        .filter(|entry| entry.index() != first.index() && key(entry.path()) == wanted);
    Ok(choose_descriptor(first, same_path, Descriptor::Fallback))
}

fn choose_descriptor<'archive>(
    first: &'archive Entry,
    mut others: impl Iterator<Item = &'archive Entry>,
    unique: impl FnOnce(&'archive Entry) -> Descriptor<'archive>,
) -> Descriptor<'archive> {
    let Some(second) = others.next() else {
        return unique(first);
    };
    Descriptor::Ambiguous([first, second].into_iter().chain(others).collect())
}

fn named_descriptors<'archive>(
    container: &'archive Archive,
    socpak: &Entry,
) -> Result<impl Iterator<Item = &'archive Entry>, String> {
    let stem = key(socpak.path());
    let stem = stem
        .rsplit('\\')
        .next()
        .and_then(|name| name.strip_suffix(SOCPAK))
        .ok_or("not a socpak name")?;
    let main = format!("{stem}.xml");
    Ok(container.entries().iter().filter(move |inner| {
        placement_xml_path(inner)
            .is_some_and(|path| path.rsplit('\\').next() == Some(main.as_str()))
    }))
}

#[cfg(test)]
pub(crate) fn has_ambiguous_descriptor(ledger: &ExportLedger) -> bool {
    ledger
        .entries
        .iter()
        .any(|entry| entry.diagnosis.code == Some(crate::ExportDiagnosticCode::AmbiguousIdentity))
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
