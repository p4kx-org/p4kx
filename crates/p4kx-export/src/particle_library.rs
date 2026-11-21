// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, CancelToken, Entry, Filter};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{
    EntryCapture, EntryDiagnosis, EntryRead, EntryXml, PhysicalEntryIdentity,
};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::collected::Collected;
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::schema_fields::every_field_required;
use crate::selection::starts_with_folder;
use crate::starmap_index;
use crate::starmap_xml::Node;
use crate::{CatalogError, CatalogOptions, Kind};

#[path = "particle_library_nodes.rs"]
mod nodes;

pub use self::nodes::ParticleLibraryElement;

const FOLDER: &str = "data/libs/particles/";
const ROOT: &str = "ParticleLibrary";
const KIND: Kind = Kind::ParticleLibraries;

/// One original particle-library Entry and its ordered XML elements.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ParticleLibrary {
    identity: PhysicalEntryIdentity,
    nodes: Vec<ParticleLibraryElement>,
}

impl ParticleLibrary {
    /// Borrows the original physical Entry identity.
    #[must_use]
    pub const fn identity(&self) -> &PhysicalEntryIdentity {
        &self.identity
    }

    /// Borrows the elements in authored depth-first order.
    #[must_use]
    pub fn nodes(&self) -> &[ParticleLibraryElement] {
        &self.nodes
    }
}

pub(crate) fn collect(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    options: &CatalogOptions<'_>,
    collected: &mut Collected<'_>,
) -> Result<(), CatalogError> {
    collected.particle_libraries = stage(
        archive,
        capture,
        &mut collected.ledger,
        options.filter,
        options.cancel,
        &std::env::temp_dir(),
    )?;
    Ok(())
}

fn stage(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    directory: &Path,
) -> Result<StagedCatalogTable, CatalogError> {
    let mut rows = StagedCatalogRows::new(directory, cancel);
    let availability = visit(
        archive,
        archive.entries(),
        capture,
        ledger,
        filter,
        cancel,
        |row| rows.append(&row).map_err(Into::into),
    )?;
    rows.finish(availability).map_err(Into::into)
}

pub(crate) fn read_selected(
    archive: &Archive,
    entries: &[Entry],
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<CatalogTable<ParticleLibrary>, CatalogError> {
    let mut rows = Vec::new();
    let availability = visit(archive, entries, capture, ledger, filter, cancel, |row| {
        rows.push(row);
        Ok(())
    })?;
    Ok(CatalogTable { availability, rows })
}

#[cfg(test)]
fn read(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<CatalogTable<ParticleLibrary>, CatalogError> {
    read_selected(archive, archive.entries(), capture, ledger, filter, cancel)
}

fn visit(
    archive: &Archive,
    entries: &[Entry],
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    mut emit: impl FnMut(ParticleLibrary) -> Result<(), CatalogError>,
) -> Result<DatasetAvailability, CatalogError> {
    let mut available = false;
    let mut unknown = ledger.archive_inventory_problem.is_some();
    ledger.inventory(KIND);
    let candidates = entries.iter().filter(|entry| in_scope(entry.path()));
    for entry in candidates {
        check_cancel(cancel)?;
        if filter.is_some_and(|filter| !filter.matches(entry.path())) {
            unknown = true;
            continue;
        }
        let source =
            EntrySource::outer(capture, archive, entry)?.operation(EntryOperation::SourceXml);
        match observe(archive, entry, &source, capture, cancel)? {
            Observation::Projected(row) => {
                emit(row)?;
                available = true;
                source.record(ledger, KIND, SourceDisposition::Projected);
            }
            Observation::OtherRoot => {
                source.record(ledger, KIND, SourceDisposition::SkippedAfterRead);
            }
            Observation::Failed(disposition, diagnosis) => {
                unknown = true;
                source.diagnosed(ledger, KIND, disposition, diagnosis);
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
    Projected(ParticleLibrary),
    OtherRoot,
    Failed(SourceDisposition, EntryDiagnosis),
}

fn observe(
    archive: &Archive,
    entry: &Entry,
    source: &EntrySource,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
) -> Result<Observation, CatalogError> {
    let Ok(read) = EntryRead::read_verified(source.receipt().clone(), archive, entry, cancel)?
    else {
        return Ok(Observation::Failed(
            SourceDisposition::ReadFailed,
            EntryDiagnosis::technical(ExportDiagnosticCode::SourceReadFailure, "content", ROOT),
        ));
    };
    let xml = match EntryXml::parse(&read, capture)? {
        Ok(xml) if !xml.is_lossy() => xml,
        Ok(_) | Err(_) => {
            return Ok(Observation::Failed(
                SourceDisposition::ProjectionFailed,
                EntryDiagnosis::technical(ExportDiagnosticCode::MalformedSource, "content", ROOT),
            ));
        }
    };
    if !xml.root().name().eq_ignore_ascii_case(ROOT) {
        return Ok(Observation::OtherRoot);
    }
    Ok(Observation::Projected(ParticleLibrary {
        identity: source.receipt().physical_payload_file(starmap_index::key)?,
        nodes: nodes::read(&xml, cancel)?,
    }))
}

fn in_scope(path: &str) -> bool {
    starts_with_folder(path, FOLDER)
        && path
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("xml"))
}

fn check_cancel(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
#[path = "particle_library_tests.rs"]
mod tests;
