// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, ArchiveWatch, CancelToken, ProvenanceMode};

use super::{
    NameInput, ShopRegistrySelection, ShopRowSnapshot, ShopRowsSnapshot, project, project_rows,
};
use crate::catalog_query_session::checkpoint;
use crate::{CatalogError, StarmapTraversalBudget};

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
    read_shop_with_provenance_mode(
        archive,
        selection,
        budget,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads one shop with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_shop`.
pub fn read_shop_with_provenance_mode(
    archive: &Archive,
    selection: ShopRegistrySelection,
    budget: StarmapTraversalBudget,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<ShopRowSnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let snapshot = project(
        archive,
        selection,
        budget,
        NameInput::Standalone,
        provenance_mode,
        cancel,
    )?;
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
    read_shops_with_provenance_mode(
        archive,
        selections,
        budget,
        ProvenanceMode::MarkersOnly,
        cancel,
    )
}

/// Reads selected shops with the selected source detail.
///
/// # Errors
/// Returns the errors of `read_shops`.
pub fn read_shops_with_provenance_mode(
    archive: &Archive,
    selections: &[ShopRegistrySelection],
    budget: StarmapTraversalBudget,
    provenance_mode: ProvenanceMode,
    cancel: Option<&CancelToken>,
) -> Result<ShopRowsSnapshot, CatalogError> {
    checkpoint(cancel)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let snapshot = project_rows(
        archive,
        selections,
        budget,
        NameInput::Standalone,
        provenance_mode,
        cancel,
    )?;
    watch.check()?;
    checkpoint(cancel)?;
    Ok(snapshot)
}
