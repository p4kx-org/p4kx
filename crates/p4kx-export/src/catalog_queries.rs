// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to canonical catalog data in an opened game Archive.

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForgeEntry, Guid, Record, SourceInventory, SourcedValue,
    TreeBuilder,
};
use serde::Serialize;

use crate::archive_dataforge_query::{self, checkpoint};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::blueprint_category_list_read;
use crate::entity_selection::{self, Selected};
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

pub use crate::canonical_record_header::CatalogRecordSource;
pub use crate::catalog_record_reference::CatalogRecordReference;
pub use crate::catalog_table::DatasetAvailability;
pub use crate::definition_reference::DefinitionStatus;

/// Chooses physical category-list Records in the supplied DataForge.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum BlueprintCategoryListSelection {
    /// All category-list Records admitted by the canonical catalog policy.
    All,
    /// All admitted Records with this GUID, including duplicates.
    Guid(Guid),
    /// The Record with this physical index in the supplied DataForge.
    RecordIndex(u32),
}

/// A category-list declaration with its original ordered reference slots.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
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
    pub coverage: BlueprintCategoryListCoverage,
}

/// Physical Records that could not be read fully, or were skipped by selection policy.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BlueprintCategoryListCoverage {
    /// Physical indices whose headers could not be classified.
    pub unreadable_headers: Vec<u32>,
    /// Selected physical indices whose bodies could not be read.
    pub unreadable_bodies: Vec<u32>,
    /// Selected physical indices with recoverable field projection issues.
    pub incomplete_fields: Vec<u32>,
    /// The number of development leftovers excluded by catalog policy.
    pub skipped_leftovers: usize,
}

/// Reads category-list declarations from the Archive's original DataForge.
///
/// # Errors
///
/// Returns input, source evidence or cancellation errors. Recoverable Record and field
/// issues are described in the returned coverage.
pub fn read_blueprint_category_lists(
    archive: &Archive,
    selection: BlueprintCategoryListSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
    archive_dataforge_query::read(archive, None, cancel, |entry| {
        read_archive(archive, entry, selection, cancel)
    })
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
    selection: BlueprintCategoryListSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
    archive_dataforge_query::read(archive, Some(entry), cancel, |entry| {
        read_archive(archive, entry, selection, cancel)
    })
}

const KIND: Kind = Kind::BlueprintCategoryLists;

fn read_archive(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: BlueprintCategoryListSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let forge = entry.open().map_err(CatalogError::DataForge)?;
    let input = OriginalCatalogInput::new(&forge, &origins);
    let mut out = SelectedLists::default();
    let mut trees = TreeBuilder::new();
    match selection {
        BlueprintCategoryListSelection::RecordIndex(index) => {
            if let Some(record) = forge.record(index) {
                out.read(&record, &input, &mut trees, cancel)?;
            }
        }
        BlueprintCategoryListSelection::All | BlueprintCategoryListSelection::Guid(_) => {
            for record in forge.records() {
                checkpoint(cancel)?;
                if matches!(selection, BlueprintCategoryListSelection::Guid(guid) if record.guid() != guid)
                {
                    continue;
                }
                out.read(&record, &input, &mut trees, cancel)?;
            }
        }
    }
    sort_by_guid(&mut out.rows, |row| row.guid);
    let rows = out.rows.into_iter().map(|row| row.data).collect();
    let table = CatalogTable::new(&forge, &[KIND.records().type_name], rows);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    Ok(BlueprintCategoryListSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        source_inventory,
        coverage: out.coverage,
    })
}

struct CategoryListWithIdentity {
    guid: CatalogGuid,
    data: BlueprintCategoryList,
}

#[derive(Default)]
struct SelectedLists {
    rows: Vec<CategoryListWithIdentity>,
    coverage: BlueprintCategoryListCoverage,
}

impl SelectedLists {
    fn read<'source>(
        &mut self,
        record: &Record<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        trees: &mut TreeBuilder<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let classified = record.type_name().and_then(|source_type| {
            if source_type == KIND.records().type_name {
                entity_selection::select(record, None)
            } else {
                Ok(None)
            }
        });
        match classified {
            Err(_) => self.coverage.unreadable_headers.push(record.index()),
            Ok(Some(Selected::Leftover(KIND))) => self.coverage.skipped_leftovers += 1,
            Ok(Some(Selected::Entity(KIND))) => {
                checkpoint(cancel)?;
                let Ok(tree) = trees.build(record) else {
                    self.coverage.unreadable_bodies.push(record.index());
                    return Ok(());
                };
                let admission = RecordObservation::capture(input, record)?;
                let decoded = blueprint_category_list_read::read(&tree, input, &admission, cancel)?;
                if decoded
                    .notes
                    .iter()
                    .any(|note| note.description().issue.blocks_projection())
                {
                    self.coverage.incomplete_fields.push(record.index());
                }
                self.rows.push(CategoryListWithIdentity {
                    guid: decoded.value.guid,
                    data: BlueprintCategoryList {
                        source: decoded.value.declaration.source.into(),
                        categories: decoded.value.declaration.categories,
                    },
                });
            }
            Ok(_) => {}
        }
        Ok(())
    }
}
