// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to authored lens flare libraries.

use std::slice;

use p4kx_extraction::{
    Archive, ArchiveWatch, Build, CancelToken, Entry, FieldOrigin, ProvenanceMode, SourceInventory,
};

use crate::catalog_query_session::checkpoint;
pub use crate::catalog_read::authored_xml_tree::{
    AuthoredXmlDeclaration, AuthoredXmlElement, AuthoredXmlText,
};
pub use crate::catalog_read::loose_archive_path::{LooseArchivePath, LooseArchivePathState};
use crate::catalog_read::original::{EntryCapture, EntryReceipt, with_entry_capture_mode};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::flare_library::{self, Selection};
pub use crate::catalog_v3::flare_library::{
    FlareElement, FlareField, FlareLibrary, FlareNodeRole, FlareValue,
};
use crate::export_archive_health;
use crate::export_entry_counts::ExportEntryCounts;
use crate::export_ledger::ExportLedger;
use crate::{CatalogError, export_health_entries, starmap_index};

/// Selects original flare library Entries in one Archive.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum FlareLibrarySelection<'query> {
    /// Every admitted library in physical directory order.
    All,
    /// The Entry at this physical root directory index.
    EntryIndex(u32),
    /// Every admitted physical Entry with this Archive-relative path.
    ArchivePath(&'query str),
}

/// Selected authored flare libraries and their source inventory.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct FlareLibrarySnapshot {
    /// The Build identified by the Archive, when available.
    pub build: Option<Build>,
    /// Source-family availability within the selection.
    pub availability: DatasetAvailability,
    /// Selected libraries in physical directory order.
    pub rows: Vec<FlareLibrary>,
    /// The exact directory evidence for the selection.
    pub directory: FieldOrigin,
    /// Read outcomes for the selected source Entries.
    pub entries: ExportEntryCounts,
    /// Original sources and uses referenced by the query.
    pub source_inventory: SourceInventory,
}

/// Reads original flare libraries and directory-only texture links from the Archive.
///
/// Archive paths ignore ASCII case and accept either path separator. Entry indices are local
/// to the opened root Archive. Failed selected library reads and projections are counted in
/// `entries`; the returned rows contain the libraries that projected successfully.
///
/// # Errors
/// Returns an Archive-change, source-accounting or cancellation error.
pub fn read_flare_libraries(
    archive: &Archive,
    selection: FlareLibrarySelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<FlareLibrarySnapshot, CatalogError> {
    read_flare_libraries_with_provenance_mode(
        archive,
        selection,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads flare libraries with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_flare_libraries`.
pub fn read_flare_libraries_with_provenance_mode(
    archive: &Archive,
    selection: FlareLibrarySelection<'_>,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<FlareLibrarySnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let snapshot = with_entry_capture_mode(archive, provenance_mode, |capture| {
        let inventory_problem = export_archive_health::inventory(archive);
        let mut ledger = ExportLedger {
            archive_inventory_problem: inventory_problem,
            ..ExportLedger::default()
        };
        let (entries, path) = candidates(archive, selection);
        let mut rows = Vec::new();
        let availability = flare_library::visit(
            archive,
            capture,
            Selection {
                entries,
                path: path.as_deref(),
                filter: None,
                cancel,
            },
            &mut ledger,
            |row| {
                rows.push(row);
                Ok(())
            },
        )?;
        let registry = capture.source_uses();
        let directory = directory_origin(
            archive,
            capture,
            selection,
            entries,
            inventory_problem.is_none(),
        )?;
        let observed = export_health_entries::compile(ledger.entries)?;
        let entries = export_health_entries::counts(flare_library::KIND, &observed.sources)?;
        let source_inventory = registry.finish_with(|| checkpoint(cancel))?;
        Ok(FlareLibrarySnapshot {
            build: archive.build().cloned(),
            availability,
            rows,
            directory,
            entries,
            source_inventory,
        })
    });
    watch.check()?;
    checkpoint(cancel)?;
    snapshot
}

fn directory_origin(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    selection: FlareLibrarySelection<'_>,
    entries: &[Entry],
    inventory_complete: bool,
) -> Result<FieldOrigin, CatalogError> {
    let registry = capture.source_uses();
    let proof = match selection {
        FlareLibrarySelection::EntryIndex(_) => {
            if let [entry] = entries {
                let receipt = EntryReceipt::outer(capture, archive, entry)?;
                let reference = receipt.physical_reference(capture)?;
                let (sources, complete) = reference.contributions();
                return Ok(registry.origin(sources, complete));
            }
            registry.observe_root_directory_entry_count(archive)?
        }
        FlareLibrarySelection::All | FlareLibrarySelection::ArchivePath(_) => {
            registry.observe_root_directory_paths(archive)?
        }
    };
    Ok(registry.origin(proof.as_slice(), proof.is_some() && inventory_complete))
}

fn candidates<'archive>(
    archive: &'archive Archive,
    selection: FlareLibrarySelection<'_>,
) -> (&'archive [Entry], Option<String>) {
    match selection {
        FlareLibrarySelection::All => (archive.entries(), None),
        FlareLibrarySelection::EntryIndex(index) => {
            let entry = usize::try_from(index)
                .ok()
                .and_then(|index| archive.entries().get(index));
            (entry.map_or(&[][..], slice::from_ref), None)
        }
        FlareLibrarySelection::ArchivePath(path) => {
            (archive.entries(), Some(starmap_index::key(path)))
        }
    }
}
