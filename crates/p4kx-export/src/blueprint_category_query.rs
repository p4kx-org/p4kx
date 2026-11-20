// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};
use schemars::JsonSchema;
use serde::Serialize;

use super::unlocalized_record_query;
use super::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    CatalogRecordSource, DatasetAvailability, ParsedCatalogQueries,
};
use crate::schema_fields::every_field_required;
use crate::{CatalogError, Kind, blueprint_category};

/// One physical Blueprint Category with its eight original header fields.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
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
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_blueprint_categories(selection, cancel)
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
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_blueprint_categories(selection, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads Blueprint Categories and their original physical identities.
    ///
    /// # Errors
    ///
    /// Returns input, source evidence or cancellation errors. Recoverable gaps appear in coverage.
    pub fn read_blueprint_categories(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<BlueprintCategorySnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_blueprint_categories(selection, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads Blueprint Categories and their original physical identities.
    ///
    /// # Errors
    ///
    /// Returns input, source evidence or cancellation errors. Recoverable gaps appear in coverage.
    pub fn read_blueprint_categories(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<BlueprintCategorySnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read_archive(archive, entry, forge, selection, cancel)
        })
    }
}

fn read_archive(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategorySnapshot, CatalogError> {
    let projected = unlocalized_record_query::read(
        archive,
        entry,
        forge,
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
