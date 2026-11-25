// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Physical and authored-ID access to Subsumption event declarations.

use std::slice;

use p4kx_extraction::{
    Archive, ArchiveWatch, Build, CancelToken, Entry, FieldOrigin, Guid, ProvenanceMode,
    SourceInventory,
};

use crate::catalog_query_session::checkpoint;
pub use crate::catalog_read::authored_xml_tree::{
    AuthoredXmlDeclaration, AuthoredXmlElement, AuthoredXmlText,
};
use crate::catalog_read::original::{EntryCapture, EntryReceipt, with_entry_capture_mode};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::subsumption_event::{self, Selection};
pub use crate::catalog_v3::subsumption_event::{
    SubsumptionEvent, SubsumptionEventControl, SubsumptionEventDocument, SubsumptionEventField,
    SubsumptionEventFormat, SubsumptionEventValue,
};
use crate::export_archive_health;
use crate::export_entry_counts::ExportEntryCounts;
use crate::export_ledger::ExportLedger;
use crate::{CatalogError, export_health_entries, starmap_index};

/// Selection within the original root Archive's event documents.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum SubsumptionEventSelection<'query> {
    /// Every admitted Subsumption document in physical directory order.
    All,
    /// One physical root directory index.
    EntryIndex(u32),
    /// Every candidate with this normalized Archive-relative path.
    ArchivePath(&'query str),
    /// Every physical document owning a direct declaration of the authored ID.
    EventId(Guid),
}

/// Result of an exact physical or authored identifier selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SubsumptionEventLookup {
    /// One matching physical occurrence was established.
    Unique,
    /// More than one matching physical occurrence was established.
    Ambiguous,
    /// A complete inspected scope contained no match.
    NotFound,
    /// A complete exact result could not be established.
    Unavailable,
}

/// Selected event documents with original coverage and source evidence.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct SubsumptionEventSnapshot {
    /// Build of the open Archive, when known.
    pub build: Option<Build>,
    /// Availability of the dataset within this selection.
    pub availability: DatasetAvailability,
    /// Physical documents with their complete original event arrays.
    pub rows: Vec<SubsumptionEventDocument>,
    /// Exact lookup outcome, if requested.
    pub lookup: Option<SubsumptionEventLookup>,
    /// Original directory evidence for the selection.
    pub directory: FieldOrigin,
    /// Measured Entry reads and projections, including failures.
    pub entries: ExportEntryCounts,
    /// Original sources and uses for this query.
    pub source_inventory: SourceInventory,
}

/// Reads Subsumption event declarations from the root Archive.
///
/// Case and separators are normalized in paths. Event-ID lookup reads the event XML family.
///
/// # Errors
/// Returns Archive-change, source-accounting or cancellation errors.
pub fn read_subsumption_events(
    archive: &Archive,
    selection: SubsumptionEventSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<SubsumptionEventSnapshot, CatalogError> {
    read_subsumption_events_with_provenance_mode(
        archive,
        selection,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads Subsumption events with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_subsumption_events`.
pub fn read_subsumption_events_with_provenance_mode(
    archive: &Archive,
    selection: SubsumptionEventSelection<'_>,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<SubsumptionEventSnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let snapshot = with_entry_capture_mode(archive, provenance_mode, |capture| {
        read(archive, capture, selection, cancel)
    });
    watch.check()?;
    checkpoint(cancel)?;
    snapshot
}

fn read(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    selection: SubsumptionEventSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<SubsumptionEventSnapshot, CatalogError> {
    let inventory_problem = export_archive_health::inventory(archive);
    let mut ledger = ExportLedger {
        archive_inventory_problem: inventory_problem,
        ..ExportLedger::default()
    };
    let (entries, path, id) = candidates(archive, selection);
    let mut rows = Vec::new();
    let availability = subsumption_event::visit(
        archive,
        capture,
        &Selection {
            entries,
            path: path.as_deref(),
            id,
            filter: None,
            cancel,
        },
        &mut ledger,
        |row| {
            rows.push(row);
            Ok(())
        },
    )?;
    let directory = directory_origin(
        archive,
        capture,
        selection,
        entries,
        inventory_problem.is_none(),
    )?;
    let observed = export_health_entries::compile(ledger.entries)?;
    let entries = export_health_entries::counts(subsumption_event::KIND, &observed.sources)?;
    let complete = inventory_problem.is_none()
        && entries.failed_required_reads == 0
        && entries.failed_projections == 0
        && entries.failed_selection_candidates == 0;
    Ok(SubsumptionEventSnapshot {
        build: archive.build().cloned(),
        availability,
        lookup: lookup(selection, &rows, complete),
        rows,
        directory,
        entries,
        source_inventory: capture.source_uses().finish_with(|| checkpoint(cancel))?,
    })
}

fn candidates<'archive>(
    archive: &'archive Archive,
    selection: SubsumptionEventSelection<'_>,
) -> (&'archive [Entry], Option<String>, Option<Guid>) {
    match selection {
        SubsumptionEventSelection::All => (archive.entries(), None, None),
        SubsumptionEventSelection::EntryIndex(index) => {
            let entry = usize::try_from(index)
                .ok()
                .and_then(|index| archive.entries().get(index));
            (entry.map_or(&[][..], slice::from_ref), None, None)
        }
        SubsumptionEventSelection::ArchivePath(path) => {
            (archive.entries(), Some(starmap_index::key(path)), None)
        }
        SubsumptionEventSelection::EventId(id) => (archive.entries(), None, Some(id)),
    }
}

fn directory_origin(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    selection: SubsumptionEventSelection<'_>,
    entries: &[Entry],
    inventory_complete: bool,
) -> Result<FieldOrigin, CatalogError> {
    let registry = capture.source_uses();
    let proof = if matches!(selection, SubsumptionEventSelection::EntryIndex(_)) {
        if let [entry] = entries {
            let receipt = EntryReceipt::outer(capture, archive, entry)?;
            let reference = receipt.physical_reference(capture)?;
            let (sources, complete) = reference.contributions();
            return Ok(registry.origin(sources, complete));
        }
        registry.observe_root_directory_entry_count(archive)?
    } else {
        registry.observe_root_directory_paths(archive)?
    };
    Ok(registry.origin(proof.as_slice(), proof.is_some() && inventory_complete))
}

fn lookup(
    selection: SubsumptionEventSelection<'_>,
    rows: &[SubsumptionEventDocument],
    complete: bool,
) -> Option<SubsumptionEventLookup> {
    let count = match selection {
        SubsumptionEventSelection::All => return None,
        SubsumptionEventSelection::EntryIndex(_) | SubsumptionEventSelection::ArchivePath(_) => {
            rows.len()
        }
        SubsumptionEventSelection::EventId(id) => rows
            .iter()
            .flat_map(SubsumptionEventDocument::events)
            .filter(|event| event.id().value().is_some_and(|value| value.get() == id))
            .take(2)
            .count(),
    };
    Some(match count {
        2.. => SubsumptionEventLookup::Ambiguous,
        _ if !complete => SubsumptionEventLookup::Unavailable,
        1 => SubsumptionEventLookup::Unique,
        _ => SubsumptionEventLookup::NotFound,
    })
}
