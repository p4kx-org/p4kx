// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to physical shops, offline inventory definitions and resolution declarations.

use std::slice;

use p4kx_extraction::{
    Archive, ArchiveWatch, Build, CancelToken, FieldOrigin, ProvenanceMode, SourceInventory,
};

use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::{EntryCapture, EntryReceipt, with_entry_capture_mode};
pub use crate::catalog_read::original::{PhysicalEntryIdentity, PhysicalEntryRef};
use crate::catalog_table::DatasetAvailability;
pub use crate::contract_shops::{Shop, ShopItemGuids, ShopRegistryFields};
pub use crate::export_entry_counts::ExportEntryCounts;
pub use crate::shop_inventory_fields::{
    InventoryCollection, InventoryId, InventoryLine, InventoryLines, ShopInventoryFields,
};
pub use crate::shop_inventory_file::{ShopInventoryDefinition, ShopInventoryDocument};
pub use crate::shop_inventory_resolution::{
    InventoryCandidate, ResolutionStatus, ShopInventoryResolution,
};
pub use crate::shop_inventory_source::{
    JsonMember as ShopInventoryMember, JsonString as ShopInventoryString,
    JsonValue as ShopInventoryToken, ShopInventorySource,
};
pub use crate::shops::resolution_scope::{
    ShopInventoryResolutionCoverage, ShopInventoryResolutionSnapshot, ShopResolutionDiscovery,
    ShopResolutionSelection, read_shop_inventory_resolutions,
    read_shop_inventory_resolutions_with_provenance_mode,
};
pub use crate::shops::row_query::{
    ShopNameCoverage, ShopRegistrySelection, ShopRowCoverage, ShopRowResult, ShopRowSnapshot,
    ShopRowsSnapshot, read_shop, read_shop_with_provenance_mode, read_shops,
    read_shops_with_provenance_mode,
};
use crate::{CatalogError, Kind, export_health_entries, shop_inventories, starmap_index};

/// Selects original shop inventory Entries in one Archive.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum ShopInventorySelection<'query> {
    /// All admitted offline inventory Entries.
    All,
    /// The Entry at this physical directory index.
    EntryIndex(u32),
    /// Every admitted physical Entry with the normalized Archive-relative path.
    ArchivePath(&'query str),
}

/// Observed coverage of the selected physical Entries and root directory.
#[derive(Debug)]
#[non_exhaustive]
pub struct ShopInventoryCoverage {
    /// Original directory evidence for the selected Entries and their discovery scope.
    pub directory: FieldOrigin,
    /// Read and projection outcomes for the selected Entries.
    pub entries: ExportEntryCounts,
}

/// Shop inventory definitions and their scoped source inventory.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct ShopInventorySnapshot {
    /// The Build identified by this Archive, when available.
    pub build: Option<Build>,
    /// Availability of the inventory definitions within the selection.
    pub availability: DatasetAvailability,
    /// Canonical rows in normalized path and physical Entry order.
    pub rows: Vec<ShopInventoryDefinition>,
    /// Source references used by these rows and their coverage.
    pub source_inventory: SourceInventory,
    /// Exact coverage of the selected roots and bodies.
    pub coverage: ShopInventoryCoverage,
}

/// Reads current offline inventory definitions directly from the original Archive.
///
/// Entry indices are local to the opened Archive. Archive paths ignore ASCII case and accept
/// either path separator. Unreadable or malformed selected Entries retain a row with a typed
/// document state. This query does not open DataForge or the shop registry.
///
/// # Errors
/// Returns an Archive-change, source-accounting or cancellation error.
pub fn read_shop_inventories(
    archive: &Archive,
    selection: ShopInventorySelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<ShopInventorySnapshot, CatalogError> {
    read_shop_inventories_with_provenance_mode(
        archive,
        selection,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads shop inventories with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_shop_inventories`.
pub fn read_shop_inventories_with_provenance_mode(
    archive: &Archive,
    selection: ShopInventorySelection<'_>,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<ShopInventorySnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let (candidates, path) = match selection {
        ShopInventorySelection::All => (archive.entries(), None),
        ShopInventorySelection::EntryIndex(index) => {
            let entry = usize::try_from(index)
                .ok()
                .and_then(|index| archive.entries().get(index));
            (entry.map_or(&[][..], slice::from_ref), None)
        }
        ShopInventorySelection::ArchivePath(path) => {
            (archive.entries(), Some(starmap_index::key(path)))
        }
    };
    let snapshot = with_entry_capture_mode(archive, provenance_mode, |capture| {
        let (mut table, ledger, complete) = shop_inventories::read_definitions(
            archive,
            candidates,
            path.as_deref(),
            capture,
            cancel,
        )?;
        let registry = capture.source_uses();
        let directory = selection_origin(archive, selection, candidates, capture, complete)?;
        if table.rows.is_empty() && !directory.source_set_complete() {
            table.availability = DatasetAvailability::Unavailable;
        }
        let evidence = export_health_entries::compile(ledger.entries)?;
        let entries = export_health_entries::counts(Kind::ShopInventories, &evidence.sources)?;
        let source_inventory = registry.finish_with(|| checkpoint(cancel))?;
        Ok(ShopInventorySnapshot {
            build: archive.build().cloned(),
            availability: table.availability,
            rows: table.rows,
            source_inventory,
            coverage: ShopInventoryCoverage { directory, entries },
        })
    });
    watch.check()?;
    checkpoint(cancel)?;
    snapshot
}

fn selection_origin(
    archive: &Archive,
    selection: ShopInventorySelection<'_>,
    candidates: &[p4kx_extraction::Entry],
    capture: &EntryCapture<'_>,
    complete: bool,
) -> Result<FieldOrigin, CatalogError> {
    let registry = capture.source_uses();
    let directory = match selection {
        ShopInventorySelection::EntryIndex(_) => {
            if let [entry] = candidates {
                let receipt = EntryReceipt::outer(capture, archive, entry)?;
                let reference = receipt.physical_reference(capture)?;
                let (sources, complete) = reference.contributions();
                return Ok(registry.origin(sources, complete));
            }
            registry.observe_root_directory_entry_count(archive)?
        }
        ShopInventorySelection::All | ShopInventorySelection::ArchivePath(_) => {
            registry.observe_root_directory_paths(archive)?
        }
    };
    Ok(registry.origin(directory.as_slice(), complete && directory.is_some()))
}
