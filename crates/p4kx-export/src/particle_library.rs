// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, Entry, Filter};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{
    EntryCapture, EntryDiagnosis, EntryRead, EntryXml, PhysicalEntryIdentity,
};
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

const FOLDER: &str = "data/libs/particles/";
const ROOT: &str = "ParticleLibrary";
const KIND: Kind = Kind::ParticleLibraries;

pub(crate) type ParticleLibraryTable = CatalogTable<ParticleLibrary>;

/// One original particle-library Entry and its ordered XML elements.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ParticleLibrary {
    identity: PhysicalEntryIdentity,
    nodes: Vec<nodes::ParticleLibraryElement>,
}

pub(crate) fn collect(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    options: &CatalogOptions<'_>,
    collected: &mut Collected<'_>,
) -> Result<(), CatalogError> {
    collected.particle_libraries = read(
        archive,
        capture,
        &mut collected.ledger,
        options.filter,
        options.cancel,
    )?;
    Ok(())
}
fn read(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    ledger: &mut ExportLedger,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<ParticleLibraryTable, CatalogError> {
    let mut rows = Vec::new();
    let mut unknown = ledger.archive_inventory_problem.is_some();
    ledger.inventory(KIND);
    let candidates = archive
        .entries()
        .iter()
        .filter(|entry| in_scope(entry.path()));
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
                rows.push(row);
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
    let availability = if !rows.is_empty() {
        DatasetAvailability::Available
    } else if unknown {
        DatasetAvailability::Unavailable
    } else {
        DatasetAvailability::Absent
    };
    Ok(CatalogTable { availability, rows })
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
