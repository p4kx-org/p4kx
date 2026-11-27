// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote, Record,
    SourceInventory, TreeBuilder,
};

use super::unlocalized_record_query;
use super::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef};
use crate::guid_text::sort_by_guid;
use crate::manufacturer::{self, Manufacturer, ManufacturerDefinition};
use crate::record_observation::RecordObservation;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

const KIND: Kind = Kind::Manufacturers;

/// Selected canonical manufacturer definitions with their texts and source inventory.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ManufacturerSnapshot<'source> {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of the manufacturer definition family.
    pub availability: DatasetAvailability,
    /// Definitions in canonical GUID order, retaining the physical positions for equal GUIDs.
    pub rows: Vec<ManufacturerDefinition<'source>>,
    /// Public names referenced by the selected definitions.
    pub public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    /// Protected game texts referenced by the selected definitions.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Requested locales including those absent from the Archive.
    pub requested_languages: Vec<Language>,
    /// Requested locales whose payloads were available and read.
    pub available_languages: Vec<Language>,
    /// Decoding and parsing notes from the localization inputs.
    pub localization_notes: Vec<LocalizationNote>,
    /// Sources and uses referenced by this result.
    pub source_inventory: SourceInventory,
    /// Coverage of Record selection, reading and projection.
    pub coverage: CatalogRecordCoverage,
}

impl<'source> ManufacturerSnapshot<'source> {
    /// Resolves a public name among the texts referenced by this snapshot.
    #[must_use]
    pub fn public_name(
        &self,
        reference: &PublicNameRef,
    ) -> Option<&CanonicalText<'source, PublicNameRef>> {
        self.public_names.iter().find(|text| {
            text.id
                .value()
                .is_some_and(|id| id.as_str() == reference.as_str())
        })
    }

    /// Resolves a protected text among the texts referenced by this snapshot.
    #[must_use]
    pub fn game_text(
        &self,
        reference: &GameTextRef,
    ) -> Option<&CanonicalText<'source, GameTextRef>> {
        self.game_texts
            .iter()
            .find(|text| text.id.value() == Some(reference))
    }
}

/// Supplies selected manufacturer definitions and their canonical texts to a callback.
/// Publish consumer-owned changes after this function returns successfully.
///
/// # Errors
/// Returns input, source evidence or cancellation errors.
pub fn with_manufacturers<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ManufacturerSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_manufacturers(selection, cancel, inspect)
}

/// Supplies manufacturer definitions using an already decoded entry of the same Archive.
///
/// # Errors
/// Returns the errors of [`with_manufacturers`], or an origin mismatch for a foreign entry.
pub fn with_manufacturers_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ManufacturerSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_manufacturers(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected manufacturer definitions using this session's Archive and locale policy.
    ///
    /// # Errors
    /// Returns the errors of [`with_manufacturers`].
    pub fn with_manufacturers<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ManufacturerSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_manufacturers(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected manufacturer definitions using this view's parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_manufacturers`].
    pub fn with_manufacturers<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ManufacturerSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            with_parsed(
                archive,
                entry,
                forge,
                selection,
                self.languages(),
                cancel,
                inspect,
            )
        })
    }
}

fn with_parsed<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    requested_languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ManufacturerSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let mut coverage = CatalogRecordCoverage::default();
    let mut records = Vec::new();
    selection.for_each(forge, cancel, |record| {
        if unlocalized_record_query::selected(record, KIND, &mut coverage) {
            records.push(*record);
        }
        Ok(())
    })?;
    checkpoint(cancel)?;
    let selected_languages = if records.is_empty() {
        &[]
    } else {
        requested_languages
    };
    let sources = TextSources::read_languages(archive, selected_languages)?;
    checkpoint(cancel)?;
    let origins = entry.with_texts(archive, &sources)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(KIND);
    let rows = read_selected(records, &projection, &mut coverage, cancel)?;
    let table = manufacturer::table(forge, rows);
    let canonical_texts = projection.finish_rows()?;
    let available_languages = selected_languages
        .iter()
        .copied()
        .filter(|language| sources.entry(*language).is_some())
        .collect();
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(ManufacturerSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        public_names: canonical_texts.public_names,
        game_texts: canonical_texts.game_texts,
        requested_languages: requested_languages.to_vec(),
        available_languages,
        localization_notes: texts.notes().to_vec(),
        source_inventory,
        coverage,
    }))
}

fn read_selected<'source>(
    records: Vec<Record<'source>>,
    projection: &CatalogProjection<'_, 'source>,
    coverage: &mut CatalogRecordCoverage,
    cancel: Option<&CancelToken>,
) -> Result<Vec<Manufacturer<'source>>, CatalogError> {
    let mut trees = TreeBuilder::new();
    let mut rows = Vec::new();
    for record in records {
        checkpoint(cancel)?;
        let Ok(tree) = trees.build(&record) else {
            coverage.unreadable_bodies.push(record.index());
            continue;
        };
        let admission = RecordObservation::capture(projection.original(), &record)?;
        let decoded = manufacturer::read(&tree, projection, &admission, cancel)?;
        if decoded
            .notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(record.index());
        }
        rows.push(decoded.value);
    }
    sort_by_guid(&mut rows, |row| row.guid);
    Ok(rows)
}
