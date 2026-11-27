// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::sound::radar_audio::RadarAudioElement;
use p4kx_extraction::{Archive, CancelToken, Entry, Filter};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{
    EntryCapture, EntryDiagnosis, EntryRead, EntrySourceXml, PhysicalEntryIdentity,
};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::entry_source::EntrySource;
use crate::export_archive_health;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::schema_fields::every_field_required;
use crate::selection::starts_with_folder;
use crate::{CatalogError, Kind};

#[path = "radar_audio_node.rs"]
mod node;

pub use node::{RadarAudioBindingStatus, RadarAudioCell, RadarAudioDatum, RadarAudioNode};

pub(super) const PATH: &str = "Data/Libs/GameAudio/RadarAudioData.xml";
pub(crate) const KIND: Kind = Kind::RadarAudio;

/// One physical radar-audio configuration and its ordered original elements.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct RadarAudioConfiguration {
    identity: PhysicalEntryIdentity,
    elements: Vec<RadarAudioNode>,
}

impl RadarAudioConfiguration {
    /// Borrows the physical Archive Entry identity.
    #[must_use]
    pub const fn identity(&self) -> &PhysicalEntryIdentity {
        &self.identity
    }

    /// Lists original elements in preorder, including unknown markup.
    #[must_use]
    pub fn elements(&self) -> &[RadarAudioNode] {
        &self.elements
    }
}

pub(crate) fn read(
    archive: &Archive,
    entries: &[Entry],
    capture: &EntryCapture<'_>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    ledger: &mut ExportLedger,
) -> Result<CatalogTable<RadarAudioConfiguration>, CatalogError> {
    ledger.inventory(KIND);
    let mut rows = Vec::new();
    let mut unknown = export_archive_health::inventory(archive).is_some();
    for entry in entries.iter().filter(|entry| includes(entry.path())) {
        crate::catalog_query_session::checkpoint(cancel)?;
        if filter.is_some_and(|filter| !filter.matches(entry.path())) {
            unknown = true;
            continue;
        }
        let source =
            EntrySource::outer(capture, archive, entry)?.operation(EntryOperation::SourceXml);
        match observe(archive, entry, capture, &source, cancel)? {
            Observation::Projected(row) => {
                source.record(ledger, KIND, SourceDisposition::Projected);
                rows.push(row);
            }
            Observation::Failed(disposition, code) => {
                unknown = true;
                source.diagnosed(
                    ledger,
                    KIND,
                    disposition,
                    EntryDiagnosis::technical(code, "content", "RadarAudioData"),
                );
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
    Projected(RadarAudioConfiguration),
    Failed(SourceDisposition, ExportDiagnosticCode),
}

fn observe(
    archive: &Archive,
    entry: &Entry,
    capture: &EntryCapture<'_>,
    source: &EntrySource,
    cancel: Option<&CancelToken>,
) -> Result<Observation, CatalogError> {
    let Ok(read) = EntryRead::read_verified(source.receipt().clone(), archive, entry, cancel)?
    else {
        return Ok(Observation::Failed(
            SourceDisposition::ReadFailed,
            ExportDiagnosticCode::SourceReadFailure,
        ));
    };
    let xml = match EntrySourceXml::parse(&read, capture)? {
        Ok(xml) if !xml.is_lossy() && RadarAudioElement::root(xml.root()).is_some() => xml,
        Ok(_) | Err(_) => {
            return Ok(Observation::Failed(
                SourceDisposition::ProjectionFailed,
                ExportDiagnosticCode::MalformedSource,
            ));
        }
    };
    Ok(Observation::Projected(RadarAudioConfiguration {
        identity: xml.identity()?,
        elements: node::read(&xml, cancel)?,
    }))
}

pub(crate) fn includes(path: &str) -> bool {
    path.len() == PATH.len() && starts_with_folder(path, PATH)
}

#[cfg(test)]
#[path = "radar_audio_catalog_tests.rs"]
mod tests;
