// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, DataForgeEntry, SourceInventory};
use serde::Serialize;

use super::unlocalized_record_query;
use super::{
    CatalogRecordCoverage, CatalogRecordSelection, CatalogRecordSource, DatasetAvailability,
};
use crate::archive_dataforge_query;
use crate::{CatalogError, Kind, blueprint_category};

/// One physical Blueprint Category with its eight original header fields.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct BlueprintCategory {
    /// Directly sourced physical identity of the category Record.
    #[serde(flatten)]
    pub source: CatalogRecordSource,
}

/// The selected canonical Blueprint Categories and their closed source inventory.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct BlueprintCategorySnapshot {
    /// The installed Archive build, when available.
    pub build: Option<Build>,
    /// Availability of the category Record declaration.
    pub availability: DatasetAvailability,
    /// Selected rows in canonical GUID order, then by physical index for tied GUIDs.
    pub rows: Vec<BlueprintCategory>,
    /// The original sources and uses retained by the category query.
    pub source_inventory: SourceInventory,
    /// Selected Record coverage and recoverable field diagnoses.
    pub coverage: CatalogRecordCoverage,
}

/// Reads canonical Blueprint Categories from the Archive's original DataForge.
///
/// # Errors
///
/// Returns input, source evidence or cancellation errors. Recoverable Record and field
/// issues are described in the returned coverage.
pub fn read_blueprint_categories(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategorySnapshot, CatalogError> {
    archive_dataforge_query::read(archive, None, cancel, |entry| {
        read_archive(archive, entry, selection, cancel)
    })
}

/// Reads canonical Blueprint Categories using an already decoded entry of the same Archive.
///
/// # Errors
///
/// Returns the errors of [`read_blueprint_categories`], or
/// [`CatalogError::SourceOriginMismatch`] for an entry from a different input.
pub fn read_blueprint_categories_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategorySnapshot, CatalogError> {
    archive_dataforge_query::read(archive, Some(entry), cancel, |entry| {
        read_archive(archive, entry, selection, cancel)
    })
}

fn read_archive(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategorySnapshot, CatalogError> {
    let projected = unlocalized_record_query::read(
        archive,
        entry,
        Kind::BlueprintCategories,
        selection,
        cancel,
        |tree, input, admission| {
            let read = blueprint_category::read(tree, input, admission, cancel)?;
            let row = BlueprintCategory {
                source: read.value.definition.source.into(),
            };
            Ok((row, read.notes))
        },
    )?;
    Ok(BlueprintCategorySnapshot {
        build: archive.build().cloned(),
        availability: projected.availability,
        rows: projected.rows,
        source_inventory: projected.source_inventory,
        coverage: projected.coverage,
    })
}
