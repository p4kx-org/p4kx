// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Filter, Language, LocalizationNote,
    SourceInventory,
};

use super::unlocalized_record_query::{
    self,
    plan::{Owner, Plan},
};
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::catalog_text_contract::{CanonicalText, GameTextRef};
use crate::catalog_v3::location_entity_declaration::{self, LocationEntityDeclaration};
use crate::catalog_v3::prepared_definition::PreparedDefinition;
use crate::record_observation::RecordObservation;
use crate::source_value_catalog::SourceRecordReference;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

const KIND: Kind = Kind::LocationEntityDeclarations;

/// Selected canonical location entity declarations and their source inventory.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct LocationEntityDeclarationSnapshot<'source> {
    /// The Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of this source family independently of selection.
    pub availability: DatasetAvailability,
    /// Selected rows in canonical GUID and physical Record order.
    pub rows: Vec<LocationEntityDeclaration<'source>>,
    /// Protected delivery descriptions referenced by these rows.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Requested locales, including those absent from the Archive.
    pub requested_languages: Vec<Language>,
    /// Requested locales whose payloads were available and read.
    pub available_languages: Vec<Language>,
    /// Notes from the selected localization inputs.
    pub localization_notes: Vec<LocalizationNote>,
    /// The original sources and uses retained by the query.
    pub source_inventory: SourceInventory,
    /// Coverage of Record selection, reading and projection.
    pub coverage: CatalogRecordCoverage,
}

impl<'source> LocationEntityDeclarationSnapshot<'source> {
    /// Joins a compatible resolved link from the same DataForge to a selected physical row.
    #[must_use]
    pub fn resolve(
        &self,
        reference: &SourceRecordReference<'_>,
    ) -> Option<&LocationEntityDeclaration<'source>> {
        let index = reference.resolved_index()?;
        let guid = reference.guid();
        let guid = guid.value()?;
        self.rows.iter().find(|row| {
            row.source().record_index().value() == Some(&index)
                && row.source().guid().value() == Some(guid)
                && reference.target_type().value().copied()
                    == Some(location_entity_declaration::SOURCE_TYPE)
        })
    }
}

/// Supplies canonical location entity declarations without building the Catalog.
///
/// # Errors
/// Returns input, source evidence or cancellation errors.
pub fn with_location_entity_declarations<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    filter: Option<&Filter>,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(LocationEntityDeclarationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_location_entity_declarations(selection, filter, cancel, inspect)
}

/// Supplies location entity declarations for a decoded entry from the same Archive.
///
/// # Errors
/// Returns query errors or an origin mismatch for a foreign entry.
pub fn with_location_entity_declarations_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    filter: Option<&Filter>,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(LocationEntityDeclarationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_location_entity_declarations(selection, filter, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies location entity declarations using this session's Archive and locale policy.
    ///
    /// # Errors
    /// Returns input, source evidence or cancellation errors.
    pub fn with_location_entity_declarations<T>(
        &self,
        selection: CatalogRecordSelection,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(LocationEntityDeclarationSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_location_entity_declarations(selection, filter, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies location entity declarations using this view's parsed DataForge.
    ///
    /// # Errors
    /// Returns input, source evidence or cancellation errors.
    pub fn with_location_entity_declarations<T>(
        &self,
        selection: CatalogRecordSelection,
        filter: Option<&Filter>,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(LocationEntityDeclarationSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            with_parsed(
                archive,
                entry,
                forge,
                selection,
                filter,
                self.languages(),
                cancel,
                inspect,
            )
        })
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "parsed source, query selection and callback"
)]
fn with_parsed<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    filter: Option<&Filter>,
    requested_languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(LocationEntityDeclarationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = Plan::read_with(forge, selection, cancel, None, |record, _, coverage| {
        let selected = unlocalized_record_query::selected(record, KIND, coverage);
        Ok(selected
            && filter
                .is_none_or(|filter| record.record_path().is_ok_and(|path| filter.matches(path))))
    })?;
    let selected_languages = if owners.is_empty() {
        &[]
    } else {
        requested_languages
    };
    let sources = TextSources::read_languages(archive, selected_languages)?;
    checkpoint(cancel)?;
    let origins = CatalogOrigins::new(archive, entry, &sources)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(KIND);
    let rows = read_rows(owners, &mut coverage, &input, &projection, cancel)?;
    let table = CatalogTable::new(forge, &[location_entity_declaration::SOURCE_TYPE], rows);
    let text_rows = projection.finish_rows()?;
    let available_languages = selected_languages
        .iter()
        .copied()
        .filter(|language| sources.entry(*language).is_some())
        .collect();
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(LocationEntityDeclarationSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        game_texts: text_rows.game_texts,
        requested_languages: requested_languages.to_vec(),
        available_languages,
        localization_notes: texts.notes().to_vec(),
        source_inventory,
        coverage,
    }))
}

fn read_rows<'source>(
    owners: Vec<Owner<'source>>,
    coverage: &mut CatalogRecordCoverage,
    input: &OriginalCatalogInput<'_, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<LocationEntityDeclaration<'source>>, CatalogError> {
    let mut prepared = Vec::with_capacity(owners.len());
    for owner in owners {
        checkpoint(cancel)?;
        let observation = RecordObservation::capture(input, &owner.record)?;
        let (row, notes) =
            location_entity_declaration::prepare(&owner.tree, projection, &observation, cancel)?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        prepared.push(row);
    }
    PreparedDefinition::order(&mut prepared);
    Ok(prepared
        .into_iter()
        .map(PreparedDefinition::into_definition)
        .collect())
}
