// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote,
    SourceInventory,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::unlocalized_record_query::plan::{Owner, Plan};
use crate::catalog_queries::{
    CanonicalText, CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage,
    CatalogRecordSelection, DatasetAvailability, GameTextRef, ParsedCatalogQueries, PublicNameRef,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_v3::starmap_definition_kinds::{
    StarmapDefinitionPick, StarmapDefinitionTables, StarmapDefinitions,
};
use crate::record_observation::RecordObservation;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

pub use crate::catalog_v3::starmap_amenity_definition::StarmapAmenityDefinition;
pub use crate::catalog_v3::starmap_preset_definition::StarmapPresetDefinition;
pub use crate::catalog_v3::starmap_type_definition::StarmapTypeDefinition;

/// Selects the authored starmap definition family.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum StarmapDefinitionFamily {
    /// Physical starmap object type definitions.
    Types,
    /// Physical starmap presets and their owned settings.
    Presets,
    /// Physical starmap amenity definitions.
    Amenities,
}

impl StarmapDefinitionFamily {
    const fn kind(self) -> Kind {
        match self {
            Self::Types => Kind::StarmapTypes,
            Self::Presets => Kind::StarmapPresets,
            Self::Amenities => Kind::StarmapAmenities,
        }
    }

    fn rows(
        self,
        tables: StarmapDefinitionTables<'_>,
    ) -> (DatasetAvailability, StarmapDefinitionRows<'_>) {
        match self {
            Self::Types => (
                tables.types.availability,
                StarmapDefinitionRows::Types(tables.types.rows),
            ),
            Self::Presets => (
                tables.presets.availability,
                StarmapDefinitionRows::Presets(tables.presets.rows),
            ),
            Self::Amenities => (
                tables.amenities.availability,
                StarmapDefinitionRows::Amenities(tables.amenities.rows),
            ),
        }
    }
}

/// One authored family and a selection of physical Records within it.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct StarmapDefinitionSelection {
    /// Dataset selected for projection.
    pub family: StarmapDefinitionFamily,
    /// Physical Records selected within the dataset.
    pub records: CatalogRecordSelection,
}

impl StarmapDefinitionSelection {
    /// Creates a selection within one original starmap definition family.
    #[must_use]
    pub const fn new(family: StarmapDefinitionFamily, records: CatalogRecordSelection) -> Self {
        Self { family, records }
    }
}

/// Canonical starmap rows in GUID order, then by physical Record index.
#[derive(Debug)]
#[non_exhaustive]
pub enum StarmapDefinitionRows<'source> {
    /// Canonical object type definitions.
    Types(Vec<StarmapTypeDefinition<'source>>),
    /// Canonical preset definitions.
    Presets(Vec<StarmapPresetDefinition<'source>>),
    /// Canonical amenity definitions.
    Amenities(Vec<StarmapAmenityDefinition<'source>>),
}

/// One selected starmap definition dataset and its referenced source evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct StarmapDefinitionSnapshot<'source> {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of the selected source type declaration.
    pub availability: DatasetAvailability,
    /// Canonical rows from the selected definition family.
    pub rows: StarmapDefinitionRows<'source>,
    /// Public names referenced by the selected rows.
    pub public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    /// Protected texts referenced by the selected rows.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Locale policy including languages unavailable in the Archive.
    pub requested_languages: Vec<Language>,
    /// Requested languages whose localization entries were read.
    pub available_languages: Vec<Language>,
    /// Decoding and parsing notes from the consulted localization entries.
    pub localization_notes: Vec<LocalizationNote>,
    /// Original sources and uses referenced by this query result.
    pub source_inventory: SourceInventory,
    /// Coverage of selected Records and their typed fields.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies selected authored starmap definitions to a callback.
/// Publish consumer-owned changes after this function returns successfully.
///
/// # Errors
/// Returns input, source evidence or cancellation errors.
pub fn with_starmap_definitions<T>(
    archive: &Archive,
    selection: StarmapDefinitionSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(StarmapDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_starmap_definitions(selection, cancel, inspect)
}

/// Supplies starmap definitions using an already decoded entry of the same Archive.
///
/// # Errors
/// Returns the errors of [`with_starmap_definitions`] or an origin mismatch for a foreign entry.
pub fn with_starmap_definitions_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: StarmapDefinitionSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(StarmapDefinitionSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_starmap_definitions(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies starmap definitions using the session's Archive and locale policy.
    ///
    /// # Errors
    /// Returns the errors of [`with_starmap_definitions`].
    pub fn with_starmap_definitions<T>(
        &self,
        selection: StarmapDefinitionSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(StarmapDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_starmap_definitions(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies starmap definitions from this view's parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_starmap_definitions`].
    pub fn with_starmap_definitions<T>(
        &self,
        selection: StarmapDefinitionSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(StarmapDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            Query {
                selection,
                requested_languages: self.languages(),
                cancel,
            }
            .read(archive, entry, forge, inspect)
        })
    }
}

struct Query<'query> {
    selection: StarmapDefinitionSelection,
    requested_languages: &'query [Language],
    cancel: Option<&'query CancelToken>,
}

impl Query<'_> {
    fn read<T>(
        &self,
        archive: &Archive,
        entry: &DataForgeEntry,
        forge: &DataForge<'_>,
        inspect: impl FnOnce(StarmapDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        let StarmapDefinitionSelection { family, records } = self.selection;
        let kind = family.kind();
        let Plan {
            owners,
            mut coverage,
        } = Plan::read(forge, kind, records, self.cancel, None)?;
        let languages = if owners.is_empty() {
            &[]
        } else {
            self.requested_languages
        };
        let sources = TextSources::read_languages(archive, languages)?;
        checkpoint(self.cancel)?;
        let origins = CatalogOrigins::new(archive, entry, &sources)?;
        let input = OriginalCatalogInput::new(forge, &origins);
        let texts = Texts::parse(&sources)?;
        let registry = CatalogTextRegistry::default();
        let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(kind);
        let definitions = self.project(kind, owners, &projection, &mut coverage)?;
        let (availability, rows) = family.rows(definitions.into_tables(forge));
        let texts_used = projection.finish_rows()?;
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(StarmapDefinitionSnapshot {
            build: archive.build().cloned(),
            availability,
            rows,
            public_names: texts_used.public_names,
            game_texts: texts_used.game_texts,
            requested_languages: self.requested_languages.to_vec(),
            available_languages: languages
                .iter()
                .copied()
                .filter(|language| sources.entry(*language).is_some())
                .collect(),
            localization_notes: texts.notes().to_vec(),
            source_inventory,
            coverage,
        }))
    }

    fn project<'source>(
        &self,
        kind: Kind,
        owners: Vec<Owner<'source>>,
        projection: &CatalogProjection<'_, 'source>,
        coverage: &mut CatalogRecordCoverage,
    ) -> Result<StarmapDefinitions<'source>, CatalogError> {
        let mut definitions = StarmapDefinitions::default();
        for owner in owners {
            checkpoint(self.cancel)?;
            let admission = RecordObservation::capture(projection.original(), &owner.record)?;
            let (row, notes) = StarmapDefinitionPick::read(
                kind,
                &owner.tree,
                projection,
                &admission,
                self.cancel,
            )?;
            if notes
                .iter()
                .any(|note| note.description().issue.blocks_projection())
            {
                coverage.incomplete_fields.push(owner.record.index());
            }
            definitions.push(row);
        }
        definitions.order();
        Ok(definitions)
    }
}
