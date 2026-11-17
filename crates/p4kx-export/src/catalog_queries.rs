// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to canonical catalog data in an opened game Archive.

#[path = "blueprint_category_query.rs"]
mod blueprint_categories;
#[path = "unlocalized_record_query.rs"]
mod unlocalized_record_query;

#[path = "gameplay_property_query.rs"]
mod gameplay_properties;

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Guid, Record, SourceInventory,
    SourcedValue,
};
use serde::Serialize;

use crate::archive_dataforge_query::{self, checkpoint};
use crate::catalog_v3::blueprint_category_list_read;
use crate::{CatalogError, Kind};

pub use crate::canonical_record_header::CatalogRecordSource;
pub use crate::catalog_record_reference::CatalogRecordReference;
pub use crate::catalog_table::DatasetAvailability;
pub use crate::catalog_text_contract::{PublicNameRef, TextForms};
pub use crate::catalog_v3::gameplay_property_catalog::{
    DisplayTransformationNodeRef, DisplayTransformationNodeSource, DisplayTransformationPayload,
    DisplayTransformationSource, GameplayProperty, GameplayPropertyFields,
    PropertyNameOverrideConditionPayload, PropertyNameOverrideConditionSource,
    PropertyNameOverrideFields, PropertyNameOverrideSource,
};
pub use crate::definition_reference::DefinitionStatus;
pub use crate::resource_number::ResourceNumber;
pub use blueprint_categories::{
    BlueprintCategory, BlueprintCategorySnapshot, read_blueprint_categories,
    read_blueprint_categories_from,
};
pub use gameplay_properties::{
    CatalogLanguageSelection, CatalogPublicName, GameplayPropertySnapshot,
    read_gameplay_properties, read_gameplay_properties_from,
};

/// Selects physical Records within the queried dataset and supplied DataForge.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum CatalogRecordSelection {
    /// All Records admitted to the queried dataset by canonical catalog policy.
    All,
    /// All admitted Records with this GUID, including duplicates.
    Guid(Guid),
    /// The Record with this physical index in the supplied DataForge.
    RecordIndex(u32),
}

impl CatalogRecordSelection {
    fn for_each<'source>(
        self,
        forge: &'source DataForge<'_>,
        cancel: Option<&CancelToken>,
        mut read: impl FnMut(&Record<'source>) -> Result<(), CatalogError>,
    ) -> Result<(), CatalogError> {
        match self {
            Self::RecordIndex(index) => {
                if let Some(record) = forge.record(index) {
                    read(&record)?;
                }
            }
            Self::All | Self::Guid(_) => {
                for record in forge.records() {
                    checkpoint(cancel)?;
                    if matches!(self, Self::Guid(guid) if record.guid() != guid) {
                        continue;
                    }
                    read(&record)?;
                }
            }
        }
        Ok(())
    }
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
    pub coverage: CatalogRecordCoverage,
}

/// Physical Records that could not be read fully, or were skipped by selection policy.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct CatalogRecordCoverage {
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
    selection: CatalogRecordSelection,
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
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
    archive_dataforge_query::read(archive, Some(entry), cancel, |entry| {
        read_archive(archive, entry, selection, cancel)
    })
}

fn read_archive(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<BlueprintCategoryListSnapshot, CatalogError> {
    let projected = unlocalized_record_query::read(
        archive,
        entry,
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
