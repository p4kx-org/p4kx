// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selective access to original location vocabulary definitions.

use p4kx_extraction::{Archive, Build, CancelToken, DataForge, DataForgeEntry, SourceInventory};

use super::unlocalized_record_query::plan::Plan;
use super::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, ParsedCatalogQueries,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::{CatalogQuerySource, checkpoint};
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::location_vocabulary::{
    LocationVocabulary, LocationVocabularyPick, LocationVocabularyTables,
};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

pub use super::equipment::contract_scopes::{
    ContractScopeGlobalParams, ContractScopeItem, ContractScopeSnapshot, with_contract_scopes,
    with_contract_scopes_from,
};
pub use crate::catalog_v3::location_vocabulary::LocationResourceSlot;
pub use crate::catalog_v3::location_vocabulary_definition::{
    LocationClassDefinition, LocationDictionaryEntry, LocationRegistry,
};

/// Selects the original family of location vocabulary Records.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum LocationVocabularyFamily {
    /// Mission location classes.
    Classes,
    /// Mission location functions.
    Functions,
    /// Mission location sizes.
    Sizes,
    /// Resource slots used by mission modules.
    ResourceSlots,
    /// Registers of original location classes.
    ClassRegistries,
    /// Registers of original location functions.
    FunctionRegistries,
    /// Registers of original location sizes.
    SizeRegistries,
}

impl LocationVocabularyFamily {
    const fn kind(self) -> Kind {
        match self {
            Self::Classes => Kind::LocationClasses,
            Self::Functions => Kind::LocationFunctions,
            Self::Sizes => Kind::LocationSizes,
            Self::ResourceSlots => Kind::LocationResourceSlots,
            Self::ClassRegistries => Kind::LocationClassRegistries,
            Self::FunctionRegistries => Kind::LocationFunctionRegistries,
            Self::SizeRegistries => Kind::LocationSizeRegistries,
        }
    }
}

/// The selected family's typed original rows in GUID and physical Record order.
#[derive(Debug)]
#[non_exhaustive]
pub enum LocationVocabularyRows<'source> {
    /// Selected location class definitions.
    Classes(Vec<LocationClassDefinition<'source>>),
    /// Selected location function definitions.
    Functions(Vec<LocationDictionaryEntry<'source>>),
    /// Selected location size definitions.
    Sizes(Vec<LocationDictionaryEntry<'source>>),
    /// Selected location resource slot definitions.
    ResourceSlots(Vec<LocationResourceSlot<'source>>),
    /// Selected location class registers.
    ClassRegistries(Vec<LocationRegistry<'source>>),
    /// Selected location function registers.
    FunctionRegistries(Vec<LocationRegistry<'source>>),
    /// Selected location size registers.
    SizeRegistries(Vec<LocationRegistry<'source>>),
}

impl<'source> LocationVocabularyRows<'source> {
    fn select(
        family: LocationVocabularyFamily,
        tables: LocationVocabularyTables<'source>,
    ) -> (DatasetAvailability, Self) {
        match family {
            LocationVocabularyFamily::Classes => split(tables.classes, Self::Classes),
            LocationVocabularyFamily::Functions => split(tables.functions, Self::Functions),
            LocationVocabularyFamily::Sizes => split(tables.sizes, Self::Sizes),
            LocationVocabularyFamily::ResourceSlots => {
                split(tables.resource_slots, Self::ResourceSlots)
            }
            LocationVocabularyFamily::ClassRegistries => {
                split(tables.registries.classes, Self::ClassRegistries)
            }
            LocationVocabularyFamily::FunctionRegistries => {
                split(tables.registries.functions, Self::FunctionRegistries)
            }
            LocationVocabularyFamily::SizeRegistries => {
                split(tables.registries.sizes, Self::SizeRegistries)
            }
        }
    }
}

fn split<Row, Selected>(
    table: CatalogTable<Row>,
    select: impl FnOnce(Vec<Row>) -> Selected,
) -> (DatasetAvailability, Selected) {
    (table.availability, select(table.rows))
}

/// A selected family of original location definitions and their physical source evidence.
#[derive(Debug)]
#[must_use]
#[non_exhaustive]
pub struct LocationVocabularySnapshot<'source> {
    /// The installed Archive Build, when known.
    pub build: Option<Build>,
    /// Availability of the selected source family.
    pub availability: DatasetAvailability,
    /// Selected rows in canonical order.
    pub rows: LocationVocabularyRows<'source>,
    /// Original sources and uses backing the returned values.
    pub source_inventory: SourceInventory,
    /// Unreadable, incomplete and skipped Records within the requested scope.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies original location vocabulary by physical identity.
/// Publish consumer-owned results after this function succeeds.
///
/// # Errors
/// Returns Archive, accounting, source-evidence or cancellation errors.
pub fn with_location_vocabulary<T>(
    archive: &Archive,
    family: LocationVocabularyFamily,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(LocationVocabularySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_location_vocabulary(family, selection, cancel, inspect)
}

/// Supplies selected location vocabulary from a decoded Entry of the same Archive.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_location_vocabulary_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    family: LocationVocabularyFamily,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(LocationVocabularySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_location_vocabulary(family, selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected location vocabulary through this Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_location_vocabulary`].
    pub fn with_location_vocabulary<T>(
        &self,
        family: LocationVocabularyFamily,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(LocationVocabularySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_location_vocabulary(family, selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected location vocabulary from the already parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_location_vocabulary`].
    pub fn with_location_vocabulary<T>(
        &self,
        family: LocationVocabularyFamily,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(LocationVocabularySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            project(archive, entry, forge, family, selection, cancel, inspect)
        })
    }
}

fn project<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    family: LocationVocabularyFamily,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(LocationVocabularySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let kind = family.kind();
    let Plan {
        owners,
        mut coverage,
    } = Plan::read(forge, kind, selection, cancel, None)?;
    let origins = entry.records(archive)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut definitions = LocationVocabulary::default();
    for owner in owners {
        checkpoint(cancel)?;
        let observed = RecordObservation::capture(&input, &owner.record)?;
        let (pick, notes) =
            LocationVocabularyPick::read_original(kind, &owner.tree, &input, &observed, cancel)?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        definitions.push(pick);
    }
    definitions.order();
    let (availability, rows) =
        LocationVocabularyRows::select(family, definitions.into_tables(forge));
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(LocationVocabularySnapshot {
        build: archive.build().cloned(),
        availability,
        rows,
        source_inventory,
        coverage,
    }))
}
