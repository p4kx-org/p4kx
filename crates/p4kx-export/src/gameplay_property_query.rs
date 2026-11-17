// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::BTreeMap;

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForgeEntry, Language, LocalizationNote, Record,
    SourceInventory, SourcedValue, TreeBuilder,
};
use serde::Serialize;

use super::{CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability, GameplayProperty};
use crate::archive_dataforge_query::{self, checkpoint};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_text_contract::{CanonicalText, PublicNameRef, TextForms};
use crate::catalog_v3::gameplay_property_catalog;
use crate::crafting::ledger;
use crate::entity_selection::Selected;
use crate::guid_text::sort_by_guid;
use crate::record_observation::RecordObservation;
use crate::texts::{CATALOG_LANGUAGES, TextSources, Texts};
use crate::{CatalogError, Kind};

/// Selects the localization payloads consulted by a catalog query.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum CatalogLanguageSelection<'query> {
    /// Consults every supported language with a source present in the Archive.
    AllAvailable,
    /// Consults these languages only; an empty slice returns public-name keys without translations.
    Selected(&'query [Language]),
}

impl CatalogLanguageSelection<'_> {
    fn includes(self, language: Language) -> bool {
        match self {
            Self::AllAvailable => true,
            Self::Selected(languages) => languages.contains(&language),
        }
    }
}

/// A public-name definition with its selected source and display forms.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CatalogPublicName {
    /// The canonical public-name identifier.
    pub id: SourcedValue<PublicNameRef>,
    /// The original unlocalized key.
    pub key: SourcedValue<String>,
    /// The queried available languages by canonical language code.
    pub translations: SourcedValue<BTreeMap<String, TextForms<'static>>>,
}

/// Selected canonical gameplay specifications and their closed source inventory.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct GameplayPropertySnapshot {
    /// The installed Archive build, when available.
    pub build: Option<Build>,
    /// Availability of the source definition family.
    pub availability: DatasetAvailability,
    /// Selected rows in canonical GUID order, then by physical index for tied GUIDs.
    pub rows: Vec<GameplayProperty>,
    /// Only the public-name keys referenced by the selected rows.
    pub public_names: Vec<CatalogPublicName>,
    /// Requested languages in canonical order, including languages absent in this Archive.
    pub requested_languages: Vec<Language>,
    /// The requested languages with available localization payloads.
    pub available_languages: Vec<Language>,
    /// Lossy decoding and skipped lines in localization payloads used by the query.
    pub localization_notes: Vec<LocalizationNote>,
    /// Original sources and uses retained by this query.
    pub source_inventory: SourceInventory,
    /// Coverage of the headers and bodies considered by the selection.
    pub coverage: CatalogRecordCoverage,
}

/// Reads the original gameplay-property declarations for selected physical Records.
///
/// # Errors
///
/// Returns input, source evidence or cancellation errors. Recoverable Record and field
/// issues are described in the returned coverage.
pub fn read_gameplay_properties(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<GameplayPropertySnapshot, CatalogError> {
    archive_dataforge_query::read(archive, None, cancel, |entry| {
        read_archive(archive, entry, selection, languages, cancel)
    })
}

/// Reads gameplay properties using an already decoded entry of the same Archive.
///
/// # Errors
///
/// Returns the errors of [`read_gameplay_properties`], or
/// [`CatalogError::SourceOriginMismatch`] for an entry from a different source.
pub fn read_gameplay_properties_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<GameplayPropertySnapshot, CatalogError> {
    archive_dataforge_query::read(archive, Some(entry), cancel, |entry| {
        read_archive(archive, entry, selection, languages, cancel)
    })
}

const KIND: Kind = Kind::GameplayProperties;

fn read_archive(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
) -> Result<GameplayPropertySnapshot, CatalogError> {
    let requested_languages: Vec<_> = CATALOG_LANGUAGES
        .into_iter()
        .filter(|language| languages.includes(*language))
        .collect();
    let text_sources = TextSources::read_languages(archive, &requested_languages)?;
    checkpoint(cancel)?;
    let origins = CatalogOrigins::new(archive, entry, &text_sources)?;
    let forge = entry.open().map_err(CatalogError::DataForge)?;
    let input = OriginalCatalogInput::new(&forge, &origins);
    let texts = Texts::parse(&text_sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(KIND);
    let mut state = SelectedProperties::default();
    let mut trees = TreeBuilder::new();
    selection.for_each(&forge, cancel, |record| {
        state.read(record, &projection, &mut trees, cancel)
    })?;
    sort_by_guid(&mut state.rows, |row| row.physical_guid);
    let table = gameplay_property_catalog::table(&forge, state.rows);
    let rows = table.rows.into_iter().map(GameplayProperty::from).collect();
    let public_names = projection
        .finish_public_names()?
        .into_iter()
        .map(CatalogPublicName::from)
        .collect();
    let available_languages = requested_languages
        .iter()
        .copied()
        .filter(|language| text_sources.entry(*language).is_some())
        .collect();
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    Ok(GameplayPropertySnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows,
        public_names,
        requested_languages,
        available_languages,
        localization_notes: texts.notes().to_vec(),
        source_inventory,
        coverage: state.coverage,
    })
}

#[derive(Default)]
struct SelectedProperties<'source> {
    rows: Vec<gameplay_property_catalog::GameplayPropertyDefinition<'source>>,
    coverage: CatalogRecordCoverage,
}

impl<'source> SelectedProperties<'source> {
    fn read(
        &mut self,
        record: &Record<'source>,
        projection: &CatalogProjection<'_, 'source>,
        trees: &mut TreeBuilder<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        match ledger::gameplay_property(record) {
            Err(_) => self.coverage.unreadable_headers.push(record.index()),
            Ok(Some(Selected::Leftover(KIND))) => self.coverage.skipped_leftovers += 1,
            Ok(Some(Selected::Entity(KIND))) => {
                checkpoint(cancel)?;
                let Ok(tree) = trees.build(record) else {
                    self.coverage.unreadable_bodies.push(record.index());
                    return Ok(());
                };
                let admission = RecordObservation::capture(projection.original(), record)?;
                let decoded =
                    gameplay_property_catalog::read(&tree, projection, &admission, cancel)?;
                if decoded
                    .notes
                    .iter()
                    .any(|note| note.description().issue.blocks_projection())
                {
                    self.coverage.incomplete_fields.push(record.index());
                }
                self.rows.push(decoded.value);
            }
            Ok(_) => {}
        }
        Ok(())
    }
}

impl From<CanonicalText<'_, PublicNameRef>> for CatalogPublicName {
    fn from(row: CanonicalText<'_, PublicNameRef>) -> Self {
        Self {
            id: row.id,
            key: row.key.map(str::to_owned),
            translations: row.translations.map(|forms| {
                forms
                    .into_iter()
                    .map(|(code, forms)| {
                        (
                            code,
                            TextForms {
                                source_text: forms
                                    .source_text
                                    .map(|text| Cow::Owned(text.into_owned())),
                                display_text: forms
                                    .display_text
                                    .map(|text| Cow::Owned(text.into_owned())),
                            },
                        )
                    })
                    .collect()
            }),
        }
    }
}
