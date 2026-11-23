// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, ArchiveWatch, Build, CancelToken, SourceInventory};

use super::shop_list::PreparedShop;
use crate::catalog_queries::starmap_name_owners::NameInput;
use crate::catalog_queries::{CatalogQuerySession, CatalogRecordCoverage, ParsedCatalogQueries};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::with_entry_capture;
use crate::catalog_table::DatasetAvailability;
use crate::contract::Shop;
use crate::export_entry_counts::ExportEntryCounts;
use crate::{CatalogError, StarmapTraversalBudget};

#[path = "shop_row_input.rs"]
mod input;
#[path = "shop_row_placement.rs"]
mod placement;

/// A physical `ShopBackup` node in the original shop registry selected by the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ShopRegistrySelection {
    /// Physical registry Entry index in the Archive directory.
    pub entry_index: u32,
    /// XML node position recorded by the Shop key's original attribute source.
    pub node_index: u32,
}

impl ShopRegistrySelection {
    /// Selects a `ShopBackup` by its original Entry and XML node position.
    #[must_use]
    pub const fn new(entry_index: u32, node_index: u32) -> Self {
        Self {
            entry_index,
            node_index,
        }
    }
}

/// Coverage of physical name owners and system routes for a shop location.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ShopNameCoverage {
    /// Failed or skipped Record admissions in the global Starmap name survey.
    pub records: CatalogRecordCoverage,
    /// Whether all required physical descriptors and placement routes were resolved conclusively.
    pub routes_complete: bool,
}

/// Observed source scope for one physical shop lookup.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ShopRowCoverage {
    /// Whether selection among original registry rows was conclusive.
    pub registry_complete: bool,
    /// Whether all eligible shop declarations were scanned, if a shop was admitted.
    pub declarations_complete: Option<bool>,
    /// Count of compatibility inventory payload reads, including a failed selected read.
    pub inventory_payloads: usize,
    /// Whether the name join was globally enumerable, when the shop supplies a location.
    pub names: Option<ShopNameCoverage>,
    /// Read/projection counts for shop registry, declarations and selected inventory inputs.
    pub entries: ExportEntryCounts,
}

/// One physical Shop with its canonical fields and original input evidence.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct ShopRowSnapshot {
    /// Build identified by the Archive.
    pub build: Option<Build>,
    /// Original physical registry Entry and node selected by the caller.
    pub selection: ShopRegistrySelection,
    /// Availability of the original shop registry, independently of selection.
    pub availability: DatasetAvailability,
    /// Canonical row, when the selected node is the first admitted occurrence of its key.
    pub row: Option<Shop>,
    /// Original references used by this lookup.
    pub source_inventory: SourceInventory,
    /// Coverage of the selected row and the required surveys.
    pub coverage: ShopRowCoverage,
}

/// One ordered position in a physical Shop batch lookup.
#[derive(Debug)]
#[non_exhaustive]
pub struct ShopRowResult {
    /// Physical registry selection supplied at this position.
    pub selection: ShopRegistrySelection,
    /// Availability of the original shop registry.
    pub availability: DatasetAvailability,
    /// Canonical row, when the selected node is the first admitted occurrence of its key.
    pub row: Option<Shop>,
    /// Original input coverage for this position.
    pub coverage: ShopRowCoverage,
}

/// Ordered physical Shop results sharing their original source inventory.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct ShopRowsSnapshot {
    /// Build identified by the Archive.
    pub build: Option<Build>,
    /// One result for every requested position, including repeats and missing nodes.
    pub rows: Vec<ShopRowResult>,
    /// Original references used by any result in the batch.
    pub source_inventory: SourceInventory,
    /// Name and placement surveys performed for the batch.
    pub name_surveys: usize,
}

/// Reads one physical shop using the normal catalog's registry, compatibility inventory and name joins.
/// A selected row without a registry location needs no DataForge or system traversal.
///
/// # Errors
/// Returns source-accounting, original input, cancellation or Archive change errors.
pub fn read_shop(
    archive: &Archive,
    selection: ShopRegistrySelection,
    budget: StarmapTraversalBudget,
    cancel: Option<&CancelToken>,
) -> Result<ShopRowSnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let snapshot = project(archive, selection, budget, NameInput::Standalone, cancel)?;
    watch.check()?;
    checkpoint(cancel)?;
    Ok(snapshot)
}

/// Reads physical Shops in request order using a shared original name and placement survey.
///
/// # Errors
/// Returns source-accounting, original input, cancellation or Archive change errors.
pub fn read_shops(
    archive: &Archive,
    selections: &[ShopRegistrySelection],
    budget: StarmapTraversalBudget,
    cancel: Option<&CancelToken>,
) -> Result<ShopRowsSnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let snapshot = project_rows(archive, selections, budget, NameInput::Standalone, cancel)?;
    watch.check()?;
    checkpoint(cancel)?;
    Ok(snapshot)
}

impl CatalogQuerySession<'_> {
    /// Reads physical Shops in request order with at most one location survey.
    ///
    /// # Errors
    /// Returns source, format, cancellation or Archive change errors.
    pub fn read_shops(
        &self,
        selections: &[ShopRegistrySelection],
        budget: StarmapTraversalBudget,
        cancel: Option<&CancelToken>,
    ) -> Result<ShopRowsSnapshot, CatalogError> {
        self.project(cancel, |archive, entry| {
            project_rows(
                archive,
                selections,
                budget,
                NameInput::Decoded(entry),
                cancel,
            )
        })
    }

    /// Reads a physical shop using this session's decoded DataForge when a location needs it.
    ///
    /// # Errors
    /// Returns source, format, cancellation or Archive change errors.
    pub fn read_shop(
        &self,
        selection: ShopRegistrySelection,
        budget: StarmapTraversalBudget,
        cancel: Option<&CancelToken>,
    ) -> Result<ShopRowSnapshot, CatalogError> {
        self.project(cancel, |archive, entry| {
            project(
                archive,
                selection,
                budget,
                NameInput::Decoded(entry),
                cancel,
            )
        })
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads physical Shops in request order with at most one location survey.
    ///
    /// # Errors
    /// Returns source, cancellation or Archive change errors.
    pub fn read_shops(
        &self,
        selections: &[ShopRegistrySelection],
        budget: StarmapTraversalBudget,
        cancel: Option<&CancelToken>,
    ) -> Result<ShopRowsSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            project_rows(
                archive,
                selections,
                budget,
                NameInput::Parsed(entry, forge),
                cancel,
            )
        })
    }

    /// Reads a physical shop using the session's already opened DataForge view.
    ///
    /// # Errors
    /// Returns source, cancellation or Archive change errors.
    pub fn read_shop(
        &self,
        selection: ShopRegistrySelection,
        budget: StarmapTraversalBudget,
        cancel: Option<&CancelToken>,
    ) -> Result<ShopRowSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            project(
                archive,
                selection,
                budget,
                NameInput::Parsed(entry, forge),
                cancel,
            )
        })
    }
}

fn project(
    archive: &Archive,
    selection: ShopRegistrySelection,
    budget: StarmapTraversalBudget,
    names_input: NameInput<'_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<ShopRowSnapshot, CatalogError> {
    let (selected, source_inventory) =
        in_capture(archive, budget, names_input, cancel, |lookup| {
            read_selected(archive, selection, lookup, cancel)
        })?;
    Ok(ShopRowSnapshot {
        build: archive.build().cloned(),
        selection: selected.selection,
        availability: selected.availability,
        row: selected.row,
        source_inventory,
        coverage: selected.coverage,
    })
}

fn project_rows(
    archive: &Archive,
    selections: &[ShopRegistrySelection],
    budget: StarmapTraversalBudget,
    names_input: NameInput<'_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<ShopRowsSnapshot, CatalogError> {
    let ((rows, name_surveys), source_inventory) =
        in_capture(archive, budget, names_input, cancel, |lookup| {
            let mut rows = Vec::new();
            rows.try_reserve_exact(selections.len())
                .map_err(CatalogError::SourceUseAllocation)?;
            for &selection in selections {
                rows.push(read_selected(archive, selection, lookup, cancel)?);
            }
            Ok((rows, lookup.surveys()))
        })?;
    Ok(ShopRowsSnapshot {
        build: archive.build().cloned(),
        rows,
        source_inventory,
        name_surveys,
    })
}

fn in_capture<'query, 'data, T>(
    archive: &'query Archive,
    budget: StarmapTraversalBudget,
    names_input: NameInput<'query, 'data>,
    cancel: Option<&'query CancelToken>,
    project: impl FnOnce(&mut placement::Lookup<'query, '_, 'data>) -> Result<T, CatalogError>,
) -> Result<(T, SourceInventory), CatalogError> {
    with_entry_capture(archive, |capture| {
        let mut lookup = placement::Lookup::new(archive, capture, budget, names_input, cancel);
        let result = project(&mut lookup)?;
        drop(lookup);
        let inventory = capture.source_uses().finish_with(|| checkpoint(cancel))?;
        Ok((result, inventory))
    })
}

fn read_selected(
    archive: &Archive,
    selection: ShopRegistrySelection,
    lookup: &mut placement::Lookup<'_, '_, '_>,
    cancel: Option<&CancelToken>,
) -> Result<ShopRowResult, CatalogError> {
    checkpoint(cancel)?;
    let Input {
        shop,
        availability,
        mut coverage,
    } = input::read(archive, selection, lookup.capture(), cancel)?;
    let row = match shop {
        Some(shop) => {
            let (link, names) = lookup.resolve(&shop.registry.location)?;
            coverage.names = names;
            Some(shop.with_location_link(link))
        }
        None => None,
    };
    Ok(ShopRowResult {
        selection,
        availability,
        row,
        coverage,
    })
}

struct Input {
    shop: Option<PreparedShop>,
    availability: DatasetAvailability,
    coverage: ShopRowCoverage,
}
