// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::BTreeMap;

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote, Record,
    SourceInventory, SourcedValue, TreeBuilder,
};
use serde::Serialize;

use super::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    DatasetAvailability, GameplayProperty, ParsedCatalogQueries,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::checkpoint;
use crate::catalog_text_contract::{CanonicalText, PublicNameRef, TextForms};
use crate::catalog_v3::gameplay_property_catalog;
use crate::crafting::ledger;
use crate::entity_selection::Selected;
use crate::guid_text::sort_by_guid;
use crate::record_observation::RecordObservation;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

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
    /// The requested localization languages whose payloads this query read successfully.
    /// Empty when no gameplay-property Record was admitted.
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
    CatalogQuerySession::open(archive, languages, cancel)?
        .read_gameplay_properties(selection, cancel)
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
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .read_gameplay_properties(selection, cancel)
}

impl CatalogQuerySession<'_> {
    /// Reads gameplay properties and public names in the session languages.
    ///
    /// # Errors
    ///
    /// Returns input, source evidence or cancellation errors. Recoverable gaps appear in coverage.
    pub fn read_gameplay_properties(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<GameplayPropertySnapshot, CatalogError> {
        self.parsed(cancel)?
            .read_gameplay_properties(selection, cancel)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Reads gameplay properties and public names in the session languages.
    ///
    /// # Errors
    ///
    /// Returns input, source evidence or cancellation errors. Recoverable gaps appear in coverage.
    pub fn read_gameplay_properties(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
    ) -> Result<GameplayPropertySnapshot, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read_archive(archive, entry, forge, selection, self.languages(), cancel)
        })
    }
}

const KIND: Kind = Kind::GameplayProperties;

fn read_archive(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    requested_languages: &[Language],
    cancel: Option<&CancelToken>,
) -> Result<GameplayPropertySnapshot, CatalogError> {
    let mut coverage = CatalogRecordCoverage::default();
    let records = select_records(forge, selection, &mut coverage, cancel)?;
    checkpoint(cancel)?;
    let text_sources = if records.is_empty() {
        None
    } else {
        Some(TextSources::read_languages(archive, requested_languages)?)
    };
    checkpoint(cancel)?;
    let origins = match &text_sources {
        Some(sources) => CatalogOrigins::new(archive, entry, sources)?,
        None => CatalogOrigins::for_archive_records(archive, entry)?,
    };
    let projected = match &text_sources {
        Some(sources) => read_selected(
            forge,
            records,
            &mut coverage,
            sources,
            requested_languages,
            &origins,
            cancel,
        )?,
        None => ProjectedProperties::default(),
    };
    let table = gameplay_property_catalog::table(forge, projected.rows);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    Ok(GameplayPropertySnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        public_names: projected.public_names,
        requested_languages: requested_languages.to_vec(),
        available_languages: projected.available_languages,
        localization_notes: projected.localization_notes,
        source_inventory,
        coverage,
    })
}

fn select_records<'source>(
    forge: &'source DataForge<'_>,
    selection: CatalogRecordSelection,
    coverage: &mut CatalogRecordCoverage,
    cancel: Option<&CancelToken>,
) -> Result<Vec<Record<'source>>, CatalogError> {
    let mut records = Vec::new();
    selection.for_each(forge, cancel, |record| {
        match ledger::gameplay_property(record) {
            Err(_) => coverage.unreadable_headers.push(record.index()),
            Ok(Some(Selected::Leftover(KIND))) => coverage.skipped_leftovers += 1,
            Ok(Some(Selected::Entity(KIND))) => records.push(*record),
            Ok(_) => {}
        }
        Ok(())
    })?;
    Ok(records)
}

#[derive(Default)]
struct ProjectedProperties {
    rows: Vec<GameplayProperty>,
    public_names: Vec<CatalogPublicName>,
    available_languages: Vec<Language>,
    localization_notes: Vec<LocalizationNote>,
}

fn read_selected<'source>(
    forge: &'source DataForge<'source>,
    records: Vec<Record<'source>>,
    coverage: &mut CatalogRecordCoverage,
    text_sources: &'source TextSources,
    requested_languages: &[Language],
    origins: &CatalogOrigins,
    cancel: Option<&CancelToken>,
) -> Result<ProjectedProperties, CatalogError> {
    let input = OriginalCatalogInput::new(forge, origins);
    let texts = Texts::parse(text_sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(KIND);
    let mut trees = TreeBuilder::new();
    let mut rows = Vec::new();
    for record in records {
        checkpoint(cancel)?;
        let Ok(tree) = trees.build(&record) else {
            coverage.unreadable_bodies.push(record.index());
            continue;
        };
        let admission = RecordObservation::capture(projection.original(), &record)?;
        let decoded = gameplay_property_catalog::read(&tree, &projection, &admission, cancel)?;
        if decoded
            .notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(record.index());
        }
        rows.push(decoded.value);
    }
    sort_by_guid(&mut rows, |row| row.physical_guid);
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
    Ok(ProjectedProperties {
        rows: rows.into_iter().map(GameplayProperty::from).collect(),
        public_names,
        available_languages,
        localization_notes: texts.notes().to_vec(),
    })
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
