// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeEntry, RecordTree};

use super::Item;
use super::read;
use super::rows::{Family, Projector, ReadOptions};
use super::snapshot::{EquipmentSnapshot, ItemSnapshot};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_item_components::definition_read;
use crate::record_observation::RecordObservation;
use crate::variant_use_sets::VariantSourceUseSets;
use crate::{CatalogError, ExportAccountingError, item, item_variant_survey};

/// Supplies selected canonical Item rows, including variants from the unfiltered Item candidate scope.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_items<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<Items, _>(
        archive,
        None,
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        |snapshot| inspect_item(snapshot, inspect),
    )?
}

/// Supplies selected canonical Items from a decoded Entry of this Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_items_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ItemSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    read::with_archive::<Items, _>(
        archive,
        Some(entry),
        selection,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
        |snapshot| inspect_item(snapshot, inspect),
    )?
}

impl CatalogQuerySession<'_> {
    /// Supplies selected canonical Items while reusing this session.
    ///
    /// # Errors
    /// Returns query, source-evidence, Archive-change or cancellation errors.
    pub fn with_items<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ItemSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?.with_items(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected canonical Items while reusing this parsed DataForge.
    ///
    /// # Errors
    /// Returns query, source-evidence, Archive-change or cancellation errors.
    pub fn with_items<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ItemSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read::with_parsed::<Items, _>(
                archive,
                entry,
                forge,
                selection,
                &[],
                cancel,
                |snapshot| inspect_item(snapshot, inspect),
            )?
        })
    }
}

fn inspect_item<T>(
    snapshot: EquipmentSnapshot<'_, Item<'_>>,
    inspect: impl FnOnce(ItemSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let sets =
        VariantSourceUseSets::from_decisions(snapshot.rows().iter().map(|item| &item.variant_of))?;
    Ok(inspect(snapshot.with_variant_sets(sets)))
}

struct Items;

impl Projector for Items {
    type Row<'source> = Item<'source>;
    const FAMILY: Family = Family::Items;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let read = definition_read::with_definition(
            tree,
            projection,
            admission,
            cancel,
            |definition, originals| item::item(tree, definition, originals, projection),
        )?;
        let (_, (item, _)) = read.value;
        let complete = item.source.source_set_complete();
        Ok(ScopedSourceRead::new(vec![item], Vec::new(), complete))
    }

    fn finish<'source>(
        input: &OriginalCatalogInput<'source, 'source>,
        rows: &mut [Self::Row<'source>],
        coverage: &mut CatalogRecordCoverage,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        item_variant_survey::project(input, rows, cancel)?;
        for row in rows.iter() {
            checkpoint(cancel)?;
            if !row.variant_of().origin().source_set_complete() {
                let index = row
                    .source
                    .source()
                    .record_index()
                    .value()
                    .copied()
                    .ok_or(ExportAccountingError::MissingFieldInspection)?;
                coverage.incomplete_fields.push(index);
            }
        }
        coverage.incomplete_fields.sort_unstable();
        coverage.incomplete_fields.dedup();
        Ok(())
    }
}
