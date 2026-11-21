// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original loose planet resource definitions.

use std::cell::OnceCell;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{
    Archive, ArchiveWatch, CancelToken, DataForge, DataForgeEntry, SourcedValue,
};

use super::lazy_loose_records::LazyLooseRecords;
use super::loose_definitions::{self, LooseDefinitionSnapshot};
use super::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::checkpoint;
pub use crate::catalog_read::loose_archive_path::{LooseArchivePath, LooseArchivePathState};
use crate::catalog_read::loose_definition::Selection;
pub use crate::catalog_read::loose_definition::{
    DefinitionIssue, LooseDefinitionSelection as PlanetResourceSelection,
};
pub use crate::catalog_read::loose_record_link::LooseRecordCandidate;
use crate::catalog_read::loose_record_link::LooseRecordLink;
use crate::catalog_read::original::{EntryCapture, with_entry_capture};
pub use crate::catalog_read::original::{PhysicalEntryIdentity, PhysicalEntryRef};
use crate::catalog_v3::catalog_definitions::planet_resource;
pub use crate::catalog_v3::catalog_definitions::planet_resource::{
    PlanetPaletteReference, PlanetResource,
};
use crate::export_archive_health;
pub use crate::export_entry_counts::ExportEntryCounts;
use crate::export_ledger::ExportLedger;
pub use crate::guid_text::CatalogGuid;
use crate::{CatalogError, CatalogOptions};

/// Selected planet resources with their canonical rows and source evidence.
pub type PlanetResourceSnapshot = LooseDefinitionSnapshot<PlanetResource>;

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
        let records = LazyLooseRecords::new(archive, capture.source_uses(), cancel, &loaded);
        read(archive, capture, selection, cancel, |path, authored| {
            records.with_input(|original| {
                planet_resource::resolve_tint_record(
                    path,
                    authored,
                    original.forge(),
                    original,
                    None,
                    cancel,
                )
            })
        })
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
            read(archive, &capture, selection, cancel, |path, authored| {
                planet_resource::resolve_tint_record(path, authored, forge, &original, None, cancel)
            })
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
    loose_definitions::finish(
        archive,
        capture,
        selection,
        cancel,
        planet_resource::KIND,
        table,
        ledger,
    )
}
