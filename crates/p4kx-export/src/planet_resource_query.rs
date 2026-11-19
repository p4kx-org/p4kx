// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original loose planet resource definitions.

use std::cell::OnceCell;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{
    Archive, ArchiveWatch, Build, CancelToken, DataForge, DataForgeEntry, DataForgeLoadError,
    FieldOrigin, Guid, SourceInventory, SourcedValue,
};

use super::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::checkpoint;
pub use crate::catalog_read::loose_archive_path::{LooseArchivePath, LooseArchivePathState};
pub use crate::catalog_read::loose_definition::DefinitionIssue;
use crate::catalog_read::loose_definition::Selection;
pub use crate::catalog_read::loose_record_link::LooseRecordCandidate;
use crate::catalog_read::loose_record_link::LooseRecordLink;
use crate::catalog_read::original::{EntryCapture, EntryReceipt, with_entry_capture};
pub use crate::catalog_read::original::{PhysicalEntryIdentity, PhysicalEntryRef};
use crate::catalog_v3::catalog_definitions::planet_resource;
pub use crate::catalog_v3::catalog_definitions::planet_resource::{
    PlanetPaletteReference, PlanetResource,
};
use crate::export_archive_health;
pub use crate::export_entry_counts::ExportEntryCounts;
use crate::export_ledger::ExportLedger;
pub use crate::guid_text::CatalogGuid;
use crate::{CatalogError, CatalogOptions, export_health_entries};

/// Selects original planet resource definitions in one Archive.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum PlanetResourceSelection<'path> {
    /// Every admitted resource Entry in physical directory order.
    All,
    /// One physical root directory index in the opened Archive.
    EntryIndex(u32),
    /// Every admitted physical Entry at this Archive-relative path.
    ArchivePath(&'path str),
    /// Every admitted physical Entry declaring this nonzero authored GUID.
    Guid(Guid),
}

impl<'path> From<PlanetResourceSelection<'path>> for Selection<'path> {
    fn from(selection: PlanetResourceSelection<'path>) -> Self {
        match selection {
            PlanetResourceSelection::All => Self::All,
            PlanetResourceSelection::EntryIndex(index) => Self::EntryIndex(index),
            PlanetResourceSelection::ArchivePath(path) => Self::Path(path),
            PlanetResourceSelection::Guid(guid) => Self::Guid(guid),
        }
    }
}

/// Selected planet resources with their canonical rows and source evidence.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct PlanetResourceSnapshot {
    /// The Build identified by the Archive, when available.
    pub build: Option<Build>,
    /// Availability of eligible physical Entries within the selection.
    pub availability: super::DatasetAvailability,
    /// Selected rows in physical Archive directory order, including duplicate GUIDs.
    pub rows: Vec<PlanetResource>,
    /// The original directory coordinates used by the selection.
    pub directory: FieldOrigin,
    /// Required reads, projections and undecidable selection candidates.
    pub entries: ExportEntryCounts,
    /// Original sources and uses referenced by the returned values.
    pub source_inventory: SourceInventory,
}

impl PlanetResourceSnapshot {
    /// Whether the selected scope can prove that no further matching rows were omitted.
    #[must_use]
    pub fn is_selection_complete(&self) -> bool {
        self.directory.source_set_complete()
            && self.entries.failed_required_reads == 0
            && self.entries.failed_projections == 0
            && self.entries.failed_selection_candidates == 0
    }
}

/// Reads original planet resources and their selected tint palette references.
///
/// Paths ignore ASCII case and accept either separator. Exact Entry and path selection
/// do not open unrelated resource bodies. GUID lookup reads the eligible resource
/// documents and resolves tint references only for matching rows.
///
/// # Errors
/// Returns a DataForge, Archive-change, source-accounting or cancellation error.
pub fn read_planet_resources(
    archive: &Archive,
    selection: PlanetResourceSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<PlanetResourceSnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let snapshot = with_entry_capture(archive, |capture| {
        let loaded = OnceCell::new();
        let records = OnceCell::new();
        read(
            archive,
            capture,
            selection.into(),
            cancel,
            |path, authored| {
                let (forge, origins) = if let Some(records) = records.get() {
                    records
                } else {
                    let entry = DataForgeEntry::read_until(archive, &|| {
                        cancel.is_some_and(CancelToken::is_cancelled)
                    })
                    .map_err(|error| match error {
                        DataForgeLoadError::Cancelled => CatalogError::Cancelled,
                        other => CatalogError::Load(other),
                    })?;
                    let entry = loaded.get_or_init(|| entry);
                    let forge = entry.open().map_err(CatalogError::DataForge)?;
                    let origins = CatalogOrigins::for_archive_records_with_uses(
                        archive,
                        entry,
                        capture.source_uses().clone(),
                    )?;
                    records.get_or_init(|| (forge, origins))
                };
                let original = OriginalCatalogInput::new(forge, origins);
                planet_resource::resolve_tint_record(path, authored, forge, &original, None, cancel)
            },
        )
    });
    watch.check()?;
    checkpoint(cancel)?;
    snapshot
}

/// Reads the same selected rows with an already decoded DataForge Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn read_planet_resources_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: PlanetResourceSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<PlanetResourceSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_planet_resources(selection, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads selected original planet resources through this Archive session.
    ///
    /// # Errors
    /// Returns a DataForge, Archive-change, source-accounting or cancellation error.
    pub fn read_planet_resources(
        &self,
        selection: PlanetResourceSelection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<PlanetResourceSnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_planet_resources(selection, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads selected original planet resources using this parsed DataForge when needed.
    ///
    /// # Errors
    /// Returns an Archive-change, source-accounting or cancellation error.
    pub fn read_planet_resources(
        &self,
        selection: PlanetResourceSelection<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<PlanetResourceSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge: &DataForge<'_>| {
            let origins = CatalogOrigins::for_archive_records(archive, entry)?;
            let original = OriginalCatalogInput::new(forge, &origins);
            let capture = EntryCapture::for_input(&original);
            read(
                archive,
                &capture,
                selection.into(),
                cancel,
                |path, authored| {
                    planet_resource::resolve_tint_record(
                        path, authored, forge, &original, None, cancel,
                    )
                },
            )
        })
    }
}

fn read(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    selection: Selection<'_>,
    cancel: Option<&CancelToken>,
    palette_record: impl FnMut(&SourcedValue<String>, &str) -> Result<LooseRecordLink, CatalogError>,
) -> Result<PlanetResourceSnapshot, CatalogError> {
    let inventory_problem = export_archive_health::inventory(archive);
    let mut ledger = ExportLedger {
        archive_inventory_problem: inventory_problem,
        ..ExportLedger::default()
    };
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.cancel = cancel;
    let table = planet_resource::read_selected(
        archive,
        capture,
        &mut ledger,
        &options,
        selection,
        palette_record,
    )?;
    let directory = directory_origin(archive, capture, selection, inventory_problem.is_none())?;
    let observed = export_health_entries::compile(ledger.entries)?;
    let entries = export_health_entries::counts(planet_resource::KIND, &observed.sources)?;
    let source_inventory = capture.source_uses().finish_with(|| checkpoint(cancel))?;
    Ok(PlanetResourceSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        directory,
        entries,
        source_inventory,
    })
}

fn directory_origin(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    selection: Selection<'_>,
    inventory_complete: bool,
) -> Result<FieldOrigin, CatalogError> {
    let registry = capture.source_uses();
    let proof = if matches!(selection, Selection::EntryIndex(_)) {
        if let [entry] = selection.entries(archive) {
            let receipt = EntryReceipt::outer(capture, archive, entry)?;
            let reference = receipt.physical_reference(capture)?;
            let (uses, complete) = reference.contributions();
            return Ok(registry.origin(uses, complete));
        }
        registry.observe_root_directory_entry_count(archive)?
    } else {
        registry.observe_root_directory_paths(archive)?
    };
    Ok(registry.origin(proof.as_slice(), proof.is_some() && inventory_complete))
}
