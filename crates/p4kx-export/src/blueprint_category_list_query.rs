// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::unlocalized_record_query;
use super::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordReference,
    CatalogRecordSelection, CatalogRecordSource, DatasetAvailability, ParsedCatalogQueries,
};
use crate::catalog_query_session::CatalogQuerySource;
use crate::catalog_v3::blueprint_category_list_read;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, Kind};

/// A category-list declaration with its original ordered reference slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct BlueprintCategoryList {
    /// The original physical Record identity and header fields.
    #[serde(flatten)]
    pub source: CatalogRecordSource,
    /// The authored slots, including duplicates, nulls and unresolved references.
    pub categories: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
}

/// The selected category lists and the source inventory needed to resolve their uses.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct BlueprintCategoryListSnapshot {
    /// The installed Archive build, when available.
    pub build: Option<Build>,
    /// Availability of the category-list source family.
    pub availability: DatasetAvailability,
    /// Selected rows in canonical GUID order, then by physical index for tied GUIDs.
    pub rows: Vec<BlueprintCategoryList>,
    /// Original sources and uses retained by this query.
    pub source_inventory: SourceInventory,
    /// Coverage of the headers and bodies considered by the selection.
    pub coverage: CatalogRecordCoverage,
}

/// Reads category-list declarations from the Archive's original DataForge.
///
/// # Errors
///
/// Returns input, source evidence or cancellation errors. Recoverable Record and field
/// issues are described in the returned coverage.
pub fn read_blueprint_category_lists(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .read_blueprint_category_lists(selection, cancel)
}

/// Reads category-list declarations from an already decoded entry of the same Archive.
///
/// # Errors
///
/// Returns the errors of [`read_blueprint_category_lists`], or [`CatalogError::SourceOriginMismatch`]
/// if the entry's proven origin does not match the supplied Archive.
pub fn read_blueprint_category_lists_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .read_blueprint_category_lists(selection, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads category-list declarations and their original ordered references.
    ///
    /// # Errors
    ///
    /// Returns input, source evidence or cancellation errors. Recoverable gaps appear in coverage.
    pub fn read_blueprint_category_lists(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_blueprint_category_lists(selection, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads category-list declarations and their original ordered references.
    ///
    /// # Errors
    ///
    /// Returns input, source evidence or cancellation errors. Recoverable gaps appear in coverage.
    pub fn read_blueprint_category_lists(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read_archive(archive, entry, forge, selection, cancel)
        })
    }
}

fn read_archive(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
    let projected = unlocalized_record_query::read(
        archive,
        entry,
        forge,
        Kind::BlueprintCategoryLists,
        selection,
        cancel,
        |tree, input, admission| {
            let read = blueprint_category_list_read::read(tree, input, admission, cancel)?;
            let row = BlueprintCategoryList {
                source: read.value.declaration.source.into(),
                categories: read.value.declaration.categories,
            };
            Ok((row, read.notes))
        },
    )?;
    Ok(BlueprintCategoryListSnapshot {
        build: archive.build().cloned(),
        availability: projected.availability,
        rows: projected.rows,
        source_inventory: projected.source_inventory,
        coverage: projected.coverage,
    })
}
