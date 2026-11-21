// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, CancelToken, Entry, Filter, Guid, SourceValue};

use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::{EntryCapture, EntryDiagnosis, EntryRead, EntrySourceXml};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;
use crate::entry_source::EntrySource;
use crate::export_archive_health;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::selection::starts_with_folder;
use crate::{CatalogError, CatalogOptions, Kind, starmap_index};

#[path = "subsumption_event_control.rs"]
mod control;
#[path = "subsumption_event_document.rs"]
mod document;

pub use control::{SubsumptionEventControl, SubsumptionEventField, SubsumptionEventValue};
pub use document::{SubsumptionEvent, SubsumptionEventDocument, SubsumptionEventFormat};

const FOLDER: &str = "data/libs/subsumption/events/";
pub(crate) const KIND: Kind = Kind::SubsumptionEvents;

pub(crate) struct Selection<'query> {
    pub(crate) entries: &'query [Entry],
    pub(crate) path: Option<&'query str>,
    pub(crate) id: Option<Guid>,
    pub(crate) filter: Option<&'query Filter>,
    pub(crate) cancel: Option<&'query CancelToken>,
}

pub(crate) fn stage(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    options: &CatalogOptions<'_>,
    ledger: &mut ExportLedger,
    directory: &Path,
) -> Result<StagedCatalogTable, CatalogError> {
    let mut rows = StagedCatalogRows::new(directory, options.cancel);
    let availability = visit(
        archive,
        capture,
        &Selection {
            entries: archive.entries(),
            path: None,
            id: None,
            filter: options.filter,
            cancel: options.cancel,
        },
        ledger,
        |row| rows.append(&row).map_err(Into::into),
    )?;
    rows.finish(availability).map_err(Into::into)
}

pub(crate) fn visit(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    selection: &Selection<'_>,
    ledger: &mut ExportLedger,
    mut emit: impl FnMut(SubsumptionEventDocument) -> Result<(), CatalogError>,
) -> Result<DatasetAvailability, CatalogError> {
    ledger.inventory(KIND);
    let mut available = false;
    let mut unknown = export_archive_health::inventory(archive).is_some();
    for entry in selection.entries {
        checkpoint(selection.cancel)?;
        if !includes(entry.path())
            || selection
                .path
                .is_some_and(|key| starmap_index::key(entry.path()) != key)
        {
            continue;
        }
        if selection
            .filter
            .is_some_and(|filter| !filter.matches(entry.path()))
        {
            unknown = true;
            continue;
        }
        let source =
            EntrySource::outer(capture, archive, entry)?.operation(EntryOperation::SourceXml);
        match observe(archive, capture, entry, &source, selection)? {
            Observation::Projected(row) => {
                emit(row)?;
                available = true;
                source.record(ledger, KIND, SourceDisposition::Projected);
            }
            Observation::OutsideSelection => {
                source.record(ledger, KIND, SourceDisposition::SkippedAfterRead);
            }
            Observation::Failed(disposition, code) => {
                unknown = true;
                source.diagnosed(
                    ledger,
                    KIND,
                    disposition,
                    EntryDiagnosis::technical(code, "content", document::ROOT),
                );
            }
        }
    }
    Ok(if available {
        DatasetAvailability::Available
    } else if unknown {
        DatasetAvailability::Unavailable
    } else {
        DatasetAvailability::Absent
    })
}

enum Observation {
    Projected(SubsumptionEventDocument),
    OutsideSelection,
    Failed(SourceDisposition, ExportDiagnosticCode),
}

fn observe(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    entry: &Entry,
    source: &EntrySource,
    selection: &Selection<'_>,
) -> Result<Observation, CatalogError> {
    let Ok(read) =
        EntryRead::read_verified(source.receipt().clone(), archive, entry, selection.cancel)?
    else {
        return Ok(Observation::Failed(
            SourceDisposition::ReadFailed,
            ExportDiagnosticCode::SourceReadFailure,
        ));
    };
    let xml = match EntrySourceXml::parse(&read, capture)? {
        Ok(xml) if !xml.is_lossy() => xml,
        Ok(_) | Err(_) => {
            return Ok(Observation::Failed(
                SourceDisposition::ProjectionFailed,
                ExportDiagnosticCode::MalformedSource,
            ));
        }
    };
    let root = xml.root();
    if root.name() != document::ROOT
        || root.namespace().is_some()
        || selection
            .id
            .is_some_and(|id| !document::contains_id(root, id))
    {
        return Ok(Observation::OutsideSelection);
    }
    let format = SubsumptionEventFormat::from(xml.format());
    let format = read
        .receipt()
        .payload_read_result(SourceValue::Present(format), true)?;
    Ok(Observation::Projected(document::read(
        &xml,
        format,
        selection.cancel,
    )?))
}

fn includes(path: &str) -> bool {
    starts_with_folder(path, FOLDER)
        && path
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("xml"))
}

#[cfg(test)]
#[path = "subsumption_event_tests.rs"]
mod tests;
