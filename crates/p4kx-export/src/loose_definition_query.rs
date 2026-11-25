// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_extraction::ProvenanceMode;

use p4kx_extraction::{Archive, ArchiveWatch, Build, CancelToken, FieldOrigin, SourceInventory};

use crate::catalog_query_session::checkpoint;
use crate::catalog_read::loose_definition::Selection;
use crate::catalog_read::original::{EntryCapture, EntryReceipt, with_entry_capture_mode};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::export_archive_health;
use crate::export_entry_counts::ExportEntryCounts;
use crate::export_ledger::ExportLedger;
use crate::{CatalogError, CatalogOptions, Kind, export_health_entries};

/// Selected loose definitions with their canonical rows and physical source evidence.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct LooseDefinitionSnapshot<Row> {
    /// The Build identified by the Archive, when available.
    pub build: Option<Build>,
    /// Availability of eligible physical Entries within the selection.
    pub availability: DatasetAvailability,
    /// Selected rows in physical Archive directory order, including duplicate GUIDs.
    pub rows: Vec<Row>,
    /// The original directory coordinates used by the selection.
    pub directory: FieldOrigin,
    /// Required reads, projections and undecidable owner selection candidates.
    pub entries: ExportEntryCounts,
    /// Original sources and uses referenced by the returned values.
    pub source_inventory: SourceInventory,
}

impl<Row> LooseDefinitionSnapshot<Row> {
    /// Whether the selected owner scope can prove that no matching rows were omitted.
    #[must_use]
    pub fn is_selection_complete(&self) -> bool {
        self.directory.source_set_complete()
            && self.entries.failed_required_reads == 0
            && self.entries.failed_projections == 0
            && self.entries.failed_selection_candidates == 0
    }
}

pub(super) fn read_with_provenance_mode<Row>(
    archive: &Archive,
    selection: Selection<'_>,
    cancel: Option<&CancelToken>,
    kind: Kind,
    provenance_mode: ProvenanceMode,
    read_rows: impl FnOnce(
        &EntryCapture<'_>,
        &mut ExportLedger,
        &CatalogOptions<'_>,
    ) -> Result<CatalogTable<Row>, CatalogError>,
) -> Result<LooseDefinitionSnapshot<Row>, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let snapshot = with_entry_capture_mode(archive, provenance_mode, |capture| {
        let mut ledger = ExportLedger {
            archive_inventory_problem: export_archive_health::inventory(archive),
            ..ExportLedger::default()
        };
        let mut options = CatalogOptions::new(UNIX_EPOCH);
        options.cancel = cancel;
        let table = read_rows(capture, &mut ledger, &options)?;
        finish(archive, capture, selection, cancel, kind, table, ledger)
    });
    watch.check()?;
    checkpoint(cancel)?;
    snapshot
}

pub(super) fn finish<Row>(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    selection: Selection<'_>,
    cancel: Option<&CancelToken>,
    kind: Kind,
    table: CatalogTable<Row>,
    ledger: ExportLedger,
) -> Result<LooseDefinitionSnapshot<Row>, CatalogError> {
    let directory = directory_origin(
        archive,
        capture,
        selection,
        ledger.archive_inventory_problem.is_none(),
    )?;
    let observed = export_health_entries::compile(ledger.entries)?;
    let entries = export_health_entries::counts(kind, &observed.sources)?;
    let source_inventory = capture.source_uses().finish_with(|| checkpoint(cancel))?;
    Ok(LooseDefinitionSnapshot {
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
