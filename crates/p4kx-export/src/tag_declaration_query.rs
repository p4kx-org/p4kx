// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory, TAG_DATABASE_TYPE,
    TAG_TYPE,
};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::unlocalized_record_query::plan::Plan;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection,
};
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::catalog_v3::tag_catalog::{self, Declaration};
use crate::record_observation::RecordObservation;

pub use crate::catalog_v3::tag_catalog::{TagDefinition, TagRoots};

/// Selects the original Record family used for tag declarations.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum TagDeclarationFamily {
    /// Includes physical Tag and `TagDatabase` Records.
    All,
    /// Includes the physical Tag definitions.
    Definitions,
    /// Includes the original `TagDatabase` root arrays.
    Roots,
}

impl TagDeclarationFamily {
    fn includes(self, source_type: &str) -> bool {
        matches!(
            (self, source_type),
            (Self::All | Self::Definitions, TAG_TYPE)
                | (Self::All | Self::Roots, TAG_DATABASE_TYPE)
        )
    }

    fn table<Row>(
        self,
        forge: &DataForge<'_>,
        source_type: &str,
        rows: Vec<Row>,
    ) -> Option<TagDeclarationRows<Row>> {
        self.includes(source_type).then(|| {
            let table = CatalogTable::new(forge, &[source_type], rows);
            TagDeclarationRows {
                availability: table.availability,
                rows: table.rows,
            }
        })
    }
}

/// Selected rows of one authored tag family.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct TagDeclarationRows<Row> {
    /// Availability of the source family independently of the selected owner.
    pub availability: DatasetAvailability,
    /// Selected rows in canonical Catalog order.
    pub rows: Vec<Row>,
}

/// Selected tag definitions and root arrays backed by one original DataForge Entry.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct TagDeclarationSnapshot<'source> {
    /// The installed Archive Build, when known.
    pub build: Option<Build>,
    /// Physical Tag definitions when requested, ordered by GUID and Record index.
    pub definitions: Option<TagDeclarationRows<TagDefinition<'source>>>,
    /// `TagDatabase` root arrays when requested, ordered by physical Record index.
    pub roots: Option<TagDeclarationRows<TagRoots<'source>>>,
    /// Original source evidence for the returned fields and resolved target headers.
    pub source_inventory: SourceInventory,
    /// Unreadable and incomplete original Records within the requested scope.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies selected Tag or `TagDatabase` declarations using their physical Record identity.
/// Publish consumer-owned results after the query succeeds.
///
/// # Errors
/// Returns Archive, source-evidence or cancellation errors.
pub fn with_tag_declarations<T>(
    archive: &Archive,
    family: TagDeclarationFamily,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(TagDeclarationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_tag_declarations(family, selection, cancel, inspect)
}

/// Supplies selected tag declarations from this Archive's decoded DataForge Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_tag_declarations_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    family: TagDeclarationFamily,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(TagDeclarationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_tag_declarations(family, selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected tag declarations through this Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_tag_declarations`].
    pub fn with_tag_declarations<T>(
        &self,
        family: TagDeclarationFamily,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(TagDeclarationSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_tag_declarations(family, selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected tag declarations from the already parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_tag_declarations`].
    pub fn with_tag_declarations<T>(
        &self,
        family: TagDeclarationFamily,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(TagDeclarationSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            project(archive, entry, forge, (family, selection), cancel, inspect)
        })
    }
}

fn project<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    (family, selection): (TagDeclarationFamily, CatalogRecordSelection),
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(TagDeclarationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = Plan::read_with(forge, selection, cancel, None, |record, _, coverage| {
        let Ok(source_type) = record.type_name() else {
            coverage.unreadable_headers.push(record.index());
            return Ok(false);
        };
        Ok(family.includes(source_type))
    })?;
    let origins = entry.records(archive)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut definitions = Vec::new();
    let mut roots = Vec::new();
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let result = tag_catalog::read_declaration(&owner.tree, &input, &admission, None, cancel)?;
        if result
            .notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        match result.row {
            Declaration::Definition(row) => definitions.push(row),
            Declaration::Roots(row) => roots.push(row),
        }
    }
    definitions.sort_unstable_by_key(TagDefinition::order_key);
    roots.sort_unstable_by_key(TagRoots::order_key);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(TagDeclarationSnapshot {
        build: archive.build().cloned(),
        definitions: family.table(forge, TAG_TYPE, definitions),
        roots: family.table(forge, TAG_DATABASE_TYPE, roots),
        source_inventory,
        coverage,
    }))
}
