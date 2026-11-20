// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original tag declarations.

#[path = "tag_declaration_query.rs"]
mod declarations;

pub use declarations::{
    TagDeclarationFamily, TagDeclarationRows, TagDeclarationSnapshot, TagDefinition, TagRoots,
    with_tag_declarations, with_tag_declarations_from,
};

use p4kx_assembly::ENTITY_TYPE;
use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use super::unlocalized_record_query::plan::Plan;
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::catalog_v3::tag_catalog;
use crate::record_observation::RecordObservation;

pub use crate::catalog_v3::tag_catalog::EntityTags;

/// Selected original Entity tag declarations with physical coverage.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct EntityTagSnapshot<'source> {
    /// The installed Archive Build, when known.
    pub build: Option<Build>,
    /// Availability of the original Entity source family.
    pub availability: DatasetAvailability,
    /// Declarations ordered by GUID then physical Record index.
    pub rows: Vec<EntityTags<'source>>,
    /// Original sources and uses backing the returned values.
    pub source_inventory: SourceInventory,
    /// Unreadable and skipped Records within the requested scope.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies original Entity tag declarations by their physical Record identity.
/// Publish consumer-owned results after this function succeeds.
///
/// # Errors
/// Returns Archive, accounting, source-evidence or cancellation errors.
pub fn with_entity_tags<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(EntityTagSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_entity_tags(selection, cancel, inspect)
}

/// Supplies original Entity tag declarations from a decoded Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_entity_tags_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(EntityTagSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_entity_tags(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected Entity tag declarations through this Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_entity_tags`].
    pub fn with_entity_tags<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(EntityTagSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_entity_tags(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected Entity tag declarations from the parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_entity_tags`].
    pub fn with_entity_tags<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(EntityTagSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            project(archive, entry, forge, selection, cancel, inspect)
        })
    }
}

fn project<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(EntityTagSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = Plan::read_entities(forge, selection, cancel, None)?;
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut rows = Vec::with_capacity(owners.len());
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let (row, notes) = tag_catalog::entity(&owner.tree, &input, &admission, cancel)?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(row);
    }
    rows.sort_unstable_by_key(EntityTags::order_key);
    let table = CatalogTable::new(forge, &[ENTITY_TYPE], rows);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(EntityTagSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        source_inventory,
        coverage,
    }))
}
