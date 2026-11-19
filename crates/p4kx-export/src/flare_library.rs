// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, CancelToken, Entry, Filter};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::loose_archive_path::Resolver;
use crate::catalog_read::original::{
    EntryCapture, EntryDiagnosis, EntryRead, EntrySourceXml, PhysicalEntryIdentity,
};
use crate::catalog_read::parsed_entry_json;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;
use crate::entry_source::EntrySource;
use crate::export_archive_health;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::schema_fields::every_field_required;
use crate::selection::starts_with_folder;
use crate::{CatalogError, CatalogOptions, Kind, starmap_index};

#[path = "flare_element.rs"]
mod element;
#[path = "flare_parameter.rs"]
mod parameter;

pub use element::FlareElement;
pub use parameter::{FlareField, FlareNodeRole, FlareValue};

const FOLDER: &str = "data/libs/flares/";
const ROOT: &str = "LensFlareLibrary";
pub(crate) const KIND: Kind = Kind::FlareLibraries;

/// One physical flare library and its original effect tree.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct FlareLibrary {
    identity: PhysicalEntryIdentity,
    elements: Vec<FlareElement>,
}

impl FlareLibrary {
    /// Borrows the physical identity of the original Archive Entry.
    #[must_use]
    pub const fn identity(&self) -> &PhysicalEntryIdentity {
        &self.identity
    }

    /// Lists the original elements in preorder, with physical occurrence indices.
    #[must_use]
    pub fn elements(&self) -> &[FlareElement] {
        &self.elements
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Selection<'query> {
    pub(crate) entries: &'query [Entry],
    pub(crate) path: Option<&'query str>,
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
        Selection {
            entries: archive.entries(),
            path: None,
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
    selection: Selection<'_>,
    ledger: &mut ExportLedger,
    mut emit: impl FnMut(FlareLibrary) -> Result<(), CatalogError>,
) -> Result<DatasetAvailability, CatalogError> {
    ledger.inventory(KIND);
    let mut available = false;
    let mut unknown = export_archive_health::inventory(archive).is_some();
    let mut resolver = Resolver::new(archive, capture, selection.cancel);
    for entry in selection.entries {
        parsed_entry_json::check_cancel(selection.cancel)?;
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
        match observe(
            archive,
            entry,
            &source,
            capture,
            selection.cancel,
            &mut resolver,
        )? {
            Observation::Projected(library) => {
                emit(library)?;
                available = true;
                source.record(ledger, KIND, SourceDisposition::Projected);
            }
            Observation::OtherRoot => {
                source.record(ledger, KIND, SourceDisposition::SkippedAfterRead);
            }
            Observation::Failed(disposition, code) => {
                unknown = true;
                source.diagnosed(
                    ledger,
                    KIND,
                    disposition,
                    EntryDiagnosis::technical(code, "content", ROOT),
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
    Projected(FlareLibrary),
    OtherRoot,
    Failed(SourceDisposition, ExportDiagnosticCode),
}

fn observe(
    archive: &Archive,
    entry: &Entry,
    source: &EntrySource,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    resolver: &mut Resolver<'_, '_, '_>,
) -> Result<Observation, CatalogError> {
    let Ok(read) = EntryRead::read_verified(source.receipt().clone(), archive, entry, cancel)?
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
    if root.name() != ROOT || root.namespace().is_some() {
        return Ok(Observation::OtherRoot);
    }
    Ok(Observation::Projected(FlareLibrary {
        identity: xml.identity()?,
        elements: element::read(&xml, cancel, resolver)?,
    }))
}

fn includes(path: &str) -> bool {
    starts_with_folder(path, FOLDER)
        && path
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("xml"))
}
#[cfg(test)]
#[path = "flare_library_tests.rs"]
mod tests;
