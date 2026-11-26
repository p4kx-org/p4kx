// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, ArchiveWatch, Build, CancelToken, DataForgeEntry, ProvenanceMode, SourceInventory,
};

use super::ShopRowCoverage;
use super::placement::Lookup;
use crate::catalog_queries::starmap_name_owners::NameInput;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, DatasetAvailability, ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::{EntryCapture, with_entry_capture_mode};
use crate::contract::Shop;
use crate::{CatalogError, StarmapTraversalBudget};

#[path = "shop_row_visit_input.rs"]
mod input;

/// Build, source coverage and retained evidence from the complete Shop visit.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct ShopRowVisitSummary {
    /// Build identified by the Archive.
    pub build: Option<Build>,
    /// Availability of the original registry, independently of the number of rows.
    pub availability: DatasetAvailability,
    /// Number of canonical Shop rows emitted.
    pub row_count: usize,
    /// Original references used by the visit.
    pub source_inventory: SourceInventory,
    /// Scope of the shared registry, declaration, inventory and placement reads.
    pub coverage: ShopRowCoverage,
}

/// Visits every canonical Shop in shop-key order using marker-only provenance.
/// Commit callback results only after checking the terminal coverage on success.
///
/// # Errors
/// Returns visitor errors directly and converts input and cancellation errors from `CatalogError`.
pub fn visit_shops<E: From<CatalogError>>(
    archive: &Archive,
    budget: StarmapTraversalBudget,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&Shop) -> Result<(), E>,
) -> Result<ShopRowVisitSummary, E> {
    visit_shops_with_provenance_mode(archive, budget, ProvenanceMode::MarkersOnly, cancel, visit)
}

/// Visits every canonical Shop using the selected level of source detail.
///
/// # Errors
/// Returns the errors of `visit_shops`.
pub fn visit_shops_with_provenance_mode<E: From<CatalogError>>(
    archive: &Archive,
    budget: StarmapTraversalBudget,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&Shop) -> Result<(), E>,
) -> Result<ShopRowVisitSummary, E> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check().map_err(CatalogError::from)?;
    let result = project(
        archive,
        budget,
        NameInput::Standalone,
        provenance_mode,
        cancel,
        visit,
    );
    watch.check().map_err(CatalogError::from)?;
    checkpoint(cancel)?;
    result
}

/// Visits every canonical Shop using decoded DataForge bytes from the same Archive.
///
/// # Errors
/// Returns the errors of `visit_shops`, including a foreign DataForge Entry.
pub fn visit_shops_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    budget: StarmapTraversalBudget,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&Shop) -> Result<(), E>,
) -> Result<ShopRowVisitSummary, E> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .visit_shops(budget, cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits every canonical Shop through this reusable Archive session.
    ///
    /// # Errors
    /// Returns the errors of `visit_shops`.
    pub fn visit_shops<E: From<CatalogError>>(
        &self,
        budget: StarmapTraversalBudget,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&Shop) -> Result<(), E>,
    ) -> Result<ShopRowVisitSummary, E> {
        self.project(cancel, |archive, entry| {
            Ok(project(
                archive,
                budget,
                NameInput::Decoded(entry.entry()),
                entry.provenance_mode(),
                cancel,
                visit,
            ))
        })?
    }
}

impl ParsedCatalogQueries<'_> {
    /// Visits every canonical Shop through this parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of `visit_shops`.
    pub fn visit_shops<E: From<CatalogError>>(
        &self,
        budget: StarmapTraversalBudget,
        cancel: Option<&CancelToken>,
        visit: impl FnMut(&Shop) -> Result<(), E>,
    ) -> Result<ShopRowVisitSummary, E> {
        self.project_fallible(cancel, |archive, entry, forge| {
            project(
                archive,
                budget,
                NameInput::Parsed(entry.entry(), forge),
                entry.provenance_mode(),
                cancel,
                visit,
            )
        })
    }
}

fn project<E: From<CatalogError>>(
    archive: &Archive,
    budget: StarmapTraversalBudget,
    names: NameInput<'_, '_>,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&Shop) -> Result<(), E>,
) -> Result<ShopRowVisitSummary, E> {
    with_entry_capture_mode(archive, provenance_mode, |capture| {
        Ok(visit_captured(
            archive, budget, names, capture, cancel, visit,
        ))
    })?
}

fn visit_captured<E: From<CatalogError>>(
    archive: &Archive,
    budget: StarmapTraversalBudget,
    names: NameInput<'_, '_>,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    mut visit: impl FnMut(&Shop) -> Result<(), E>,
) -> Result<ShopRowVisitSummary, E> {
    let mut lookup = Lookup::new(archive, capture, budget, names, cancel);
    let completed = input::visit(archive, capture, cancel, |prepared| {
        let link = lookup.link(&prepared.registry.location)?;
        checkpoint(cancel)?;
        visit(&prepared.with_location_link(link))?;
        checkpoint(cancel).map_err(E::from)
    })?;
    let mut coverage = completed.coverage;
    coverage.names = lookup.coverage();
    drop(lookup);
    let source_inventory = capture.source_uses().finish_with(|| checkpoint(cancel))?;
    Ok(ShopRowVisitSummary {
        build: archive.build().cloned(),
        availability: completed.availability,
        row_count: completed.row_count,
        source_inventory,
        coverage,
    })
}
