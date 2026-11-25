// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory, TAG_TYPE,
    TagTreeIssues,
};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordFailure, CatalogRecordSelection,
};
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::catalog_v3::tag_catalog::{self, TagConsumers};

pub use crate::catalog_v3::tag_catalog::{TagPath, TagPathParentHeader, TagPathRow};

/// Selected canonical tag paths and the observed original graph dependency coverage.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct TagPathSnapshot<'source> {
    /// The installed Archive Build, when known.
    pub build: Option<Build>,
    /// Availability of the Tag source family independently of selection.
    pub availability: DatasetAvailability,
    /// Selected winner rows in the same depth-first order as the Catalog.
    pub rows: Vec<TagPath>,
    /// Read and topology outcomes for all candidate Tag and `TagDatabase` Records.
    pub graph_issues: TagTreeIssues,
    /// Exact original Records whose unreadable metadata or body affected graph coverage.
    pub graph_read_failures: Vec<CatalogRecordFailure<'source>>,
    /// Original source observations of the tag graph, including selected lineage uses.
    pub source_inventory: SourceInventory,
}

impl TagPathSnapshot<'_> {
    /// Returns whether candidate classification, graph reads and family availability are known.
    #[must_use]
    pub const fn graph_inventory_complete(&self) -> bool {
        self.graph_read_failures.is_empty()
            && self.graph_issues.unclassified() == 0
            && self.graph_issues.unreadable() == 0
            && !matches!(self.availability, DatasetAvailability::Unavailable)
    }
}

/// Supplies selected Catalog tag paths from their original Archive graph.
/// Publish consumer-owned results after the query succeeds.
///
/// # Errors
/// Returns Archive, source-evidence or cancellation errors.
pub fn with_tag_paths<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(TagPathSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_tag_paths(selection, cancel, inspect)
}

/// Supplies selected Catalog tag paths from this Archive's decoded DataForge Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_tag_paths_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(TagPathSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_tag_paths(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies Catalog tag paths from this Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_tag_paths`].
    pub fn with_tag_paths<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(TagPathSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_tag_paths(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies Catalog tag paths using this parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_tag_paths`].
    pub fn with_tag_paths<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(TagPathSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            project(archive, entry, forge, selection, cancel, inspect)
        })
    }
}

fn project<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(TagPathSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let origins = entry.records(archive)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let (paths, issues, failures) = {
        let captured = tag_catalog::read(forge, &input, cancel, TagConsumers::default())?;
        (
            captured.rows.paths,
            captured.context.tree().issues(),
            captured.failures,
        )
    };
    let rows = CatalogTable::new(forge, &[TAG_TYPE], select(paths, selection, cancel)?);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(TagPathSnapshot {
        build: archive.build().cloned(),
        availability: rows.availability,
        rows: rows.rows,
        graph_issues: issues,
        graph_read_failures: failures,
        source_inventory,
    }))
}

fn select(
    paths: Vec<TagPath>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<Vec<TagPath>, CatalogError> {
    if matches!(selection, CatalogRecordSelection::All) {
        return Ok(paths);
    }
    let mut selected = Vec::new();
    for path in paths {
        checkpoint(cancel)?;
        let (guid, index) = path.record_identity()?;
        if matches!(selection, CatalogRecordSelection::Guid(value) if value == guid)
            || matches!(selection, CatalogRecordSelection::RecordIndex(value) if value == index)
        {
            selected.push(path);
        }
    }
    Ok(selected)
}
