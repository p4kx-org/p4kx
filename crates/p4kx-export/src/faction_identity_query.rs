// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selects physical faction identity declarations and their localized text.

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote,
    SourceInventory,
};

use super::unlocalized_record_query::plan::{Owner, Plan};
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_text_contract::{CanonicalText, GameTextRef};
use crate::catalog_v3::faction_identity_definition::{self, PreparedFactionIdentity};
use crate::record_observation::RecordObservation;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

pub use crate::catalog_v3::faction_identity_definition::{
    EncounterPresetWeight, FactionIdentityDefinition,
};

const KIND: Kind = Kind::FactionIdentities;

/// Selected physical faction identities and their original text and provenance.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct FactionIdentitySnapshot<'source> {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of the original `FactionIdentity` Record family.
    pub availability: DatasetAvailability,
    /// Canonical definitions in GUID order, then by physical Record index.
    pub rows: Vec<FactionIdentityDefinition<'source>>,
    /// Protected text rows referenced by the selected definitions.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Locale policy including languages unavailable in the Archive.
    pub requested_languages: Vec<Language>,
    /// Requested languages whose locale entries were read.
    pub available_languages: Vec<Language>,
    /// Decoding and parsing notes from the read locale entries.
    pub localization_notes: Vec<LocalizationNote>,
    /// Original sources and uses referenced by this query result.
    pub source_inventory: SourceInventory,
    /// Measured coverage of selected Records and their typed fields.
    pub coverage: CatalogRecordCoverage,
}

impl<'source> FactionIdentitySnapshot<'source> {
    /// Resolves a protected name or description among the selected text rows.
    #[must_use]
    pub fn game_text(
        &self,
        reference: &GameTextRef,
    ) -> Option<&CanonicalText<'source, GameTextRef>> {
        self.game_texts
            .iter()
            .find(|row| row.id().value() == Some(reference))
    }
}

/// Supplies canonical faction identity definitions selected within the original Archive.
///
/// # Errors
/// Returns source, accounting or cancellation errors.
pub fn with_faction_identities<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(FactionIdentitySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_faction_identities(selection, cancel, inspect)
}

/// Supplies canonical faction identities using an already decoded entry from the same Archive.
///
/// # Errors
/// Returns the errors of [`with_faction_identities`] or an input-origin mismatch.
pub fn with_faction_identities_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(FactionIdentitySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_faction_identities(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies canonical faction identities using the session's Archive and locale policy.
    ///
    /// # Errors
    /// Returns the errors of [`with_faction_identities`].
    pub fn with_faction_identities<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(FactionIdentitySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_faction_identities(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies canonical faction identities from the parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_faction_identities`].
    pub fn with_faction_identities<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(FactionIdentitySnapshot<'_>) -> T,
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
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    requested_languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(FactionIdentitySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let Plan {
        owners,
        mut coverage,
    } = Plan::read(forge, KIND, selection, cancel, None)?;
    let languages = if owners.is_empty() {
        &[]
    } else {
        requested_languages
    };
    let sources = TextSources::read_languages(archive, languages)?;
    checkpoint(cancel)?;
    let origins = CatalogOrigins::new(archive, entry, &sources)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(KIND);
    let rows = project(owners, &projection, &mut coverage, cancel)?;
    let table = faction_identity_definition::table(forge, rows);
    let canonical_texts = projection.finish_rows()?;
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(FactionIdentitySnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        game_texts: canonical_texts.game_texts,
        requested_languages: requested_languages.to_vec(),
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
    owners: Vec<Owner<'source>>,
    projection: &CatalogProjection<'_, 'source>,
    coverage: &mut CatalogRecordCoverage,
    cancel: Option<&CancelToken>,
) -> Result<Vec<PreparedFactionIdentity<'source>>, CatalogError> {
    let mut rows = Vec::with_capacity(owners.len());
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(projection.original(), &owner.record)?;
        let (row, notes) =
            faction_identity_definition::read(&owner.tree, projection, &admission, cancel)?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(row);
    }
    faction_identity_definition::order(&mut rows);
    Ok(rows)
}
