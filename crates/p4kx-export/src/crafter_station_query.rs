// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::unlocalized_record_query::plan::{Owner, Plan};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection,
};
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::industry_modules::crafter_definition;
use crate::record_observation::RecordObservation;

pub use crafter_definition::{
    CrafterAttachment, CrafterAttachmentDefinition, CrafterCategoryRule, CrafterCategoryRuleKind,
    CrafterComponent, CrafterDefinition, CrafterModifier, CrafterModifierKind, CrafterModifierType,
    CrafterQueue,
};

/// Selected original crafting stations and their physical source evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct CrafterSnapshot<'source> {
    /// The original Archive Build, when known.
    pub build: Option<Build>,
    /// Availability of the original component family.
    pub availability: DatasetAvailability,
    /// Selected stations ordered by GUID and physical Record index.
    pub rows: Vec<CrafterDefinition<'source>>,
    /// Original sources and uses behind the selected values.
    pub source_inventory: SourceInventory,
    /// Unreadable, incomplete and skipped Records in this selection.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies selected crafting stations from an Archive.
/// Publish consumer-owned results after this function succeeds.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_crafters<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(CrafterSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_crafters(selection, cancel, inspect)
}

/// Supplies crafting stations through an already decoded Entry of this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_crafters_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(CrafterSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_crafters(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected original crafting stations through this session.
    ///
    /// # Errors
    /// Returns the errors of `with_crafters`.
    pub fn with_crafters<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(CrafterSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_crafters(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected original crafting stations from the parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of `with_crafters`.
    pub fn with_crafters<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(CrafterSnapshot<'_>) -> T,
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
    inspect: impl FnOnce(CrafterSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = Plan::read_crafters(forge, selection, cancel)?;
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut rows = Vec::new();
    for owner in owners {
        checkpoint(cancel)?;
        if let Some(row) = read_station(&owner, &input, cancel, &mut coverage)? {
            rows.push(row);
        }
    }
    crafter_definition::order(&mut rows);
    let table = crafter_definition::table(forge, rows);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(CrafterSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        source_inventory,
        coverage,
    }))
}

pub(in crate::build::catalog_queries) fn read_station<'source>(
    owner: &Owner<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
    coverage: &mut CatalogRecordCoverage,
) -> Result<Option<crafter_definition::PreparedCrafter<'source>>, CatalogError> {
    let admission = RecordObservation::capture(input, &owner.record)?;
    let Some((row, notes)) = crafter_definition::prepare(&owner.tree, input, &admission, cancel)?
    else {
        let uncertain = match owner.tree.field(crate::scan::CRAFTER_FIELD) {
            Some(p4kx_extraction::Value::Array(slots)) => slots
                .iter()
                .any(|slot| matches!(slot, p4kx_extraction::Value::Error(_))),
            None | Some(p4kx_extraction::Value::Null) => false,
            Some(_) => true,
        };
        if uncertain {
            coverage.incomplete_fields.push(owner.record.index());
        }
        return Ok(None);
    };
    if crafter_definition::selection_incomplete(&row)
        || notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
    {
        coverage.incomplete_fields.push(owner.record.index());
    }
    Ok(Some(row))
}
