// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote,
    SourceInventory,
};

use super::unlocalized_record_query::plan::Plan;
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_text_contract::{CanonicalText, GameTextRef};
use crate::mission_data;
use crate::record_observation::RecordObservation;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, Kind};

pub use crate::contract_mission_data::MissionType;

/// Selected mission types and their protected text and source evidence.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct MissionTypeSnapshot<'source> {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of the original `MissionType` family.
    pub availability: DatasetAvailability,
    /// Canonical definitions in GUID order with physical ties preserved.
    pub rows: Vec<MissionType<'source>>,
    /// Protected names referenced by the selected definitions.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Requested languages, including those absent from the Archive.
    pub requested_languages: Vec<Language>,
    /// Requested languages whose locale entries were read.
    pub available_languages: Vec<Language>,
    /// Decoding and parsing notes from the localization inputs.
    pub localization_notes: Vec<LocalizationNote>,
    /// Original sources and uses referenced by this query result.
    pub source_inventory: SourceInventory,
    /// Coverage of selected Records and their typed fields.
    pub coverage: CatalogRecordCoverage,
}

impl<'source> MissionTypeSnapshot<'source> {
    /// Resolves a protected name within the selected text rows.
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

/// Supplies selected authored mission types to a callback.
/// Publish consumer-owned changes after this function returns successfully.
///
/// # Errors
/// Returns input, source evidence or cancellation errors.
pub fn with_mission_types<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MissionTypeSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_mission_types(selection, cancel, inspect)
}

/// Supplies mission types using an already decoded entry of the same Archive.
///
/// # Errors
/// Returns the errors of [`with_mission_types`] or an origin mismatch for a foreign entry.
pub fn with_mission_types_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MissionTypeSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_mission_types(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies mission types using the session's Archive and locale policy.
    ///
    /// # Errors
    /// Returns the errors of [`with_mission_types`].
    pub fn with_mission_types<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MissionTypeSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_mission_types(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies mission types from this view's parsed DataForge.
    ///
    /// # Errors
    /// Returns the errors of [`with_mission_types`].
    pub fn with_mission_types<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MissionTypeSnapshot<'_>) -> T,
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
    inspect: impl FnOnce(MissionTypeSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let kind = Kind::MissionTypes;
    let Plan { owners, coverage } = Plan::read(forge, kind, selection, cancel, None)?;
    let languages = if owners.is_empty() {
        &[]
    } else {
        requested_languages
    };
    let sources = TextSources::read_languages(archive, languages)?;
    checkpoint(cancel)?;
    let origins = entry.with_texts(archive, &sources)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(kind);
    let mut rows = Vec::with_capacity(owners.len());
    for owner in owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(&input, &owner.record)?;
        rows.push(mission_data::mission_type(
            &owner.tree,
            &projection,
            &admission,
            cancel,
        )?);
    }
    let table = MissionType::table(forge, rows);
    let canonical = projection.finish_rows()?;
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(MissionTypeSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        game_texts: canonical.game_texts,
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
