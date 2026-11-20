// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote,
    SourceInventory,
};

use super::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability,
};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef};
use crate::catalog_v3::reputation_faction_definition::FactionReputationDefinition;
use crate::catalog_v3::reputation_scope_definition::ScopeDefinition;
use crate::catalog_v3::reputation_standing_definition::StandingDefinition;
use crate::source_value_catalog::SourceRecordReference;
use crate::texts::{TextSources, Texts};

pub use crate::catalog_v3::reputation_context_definition::ContextDefinition as ReputationContextDefinition;
pub use crate::catalog_v3::reputation_perk::ReputationPerk;
pub use crate::catalog_v3::reputation_perk_list::{ReputationPerkEntry, ReputationPerkList};
pub use crate::catalog_v3::sandbox_trigger::SandboxTrigger;
pub use crate::catalog_v3::sandbox_trigger_record_definition::SandboxTriggerRecordDefinition;

#[path = "reputation_query_source.rs"]
mod source;
use source::SelectedReputation;

/// Selects any combination of the original reputation definition families.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct ReputationSelection {
    /// Selects faction reputation Records when present.
    pub faction_reputations: Option<CatalogRecordSelection>,
    /// Selects original reputation scopes when present.
    pub scopes: Option<CatalogRecordSelection>,
    /// Selects original reputation standings when present.
    pub standings: Option<CatalogRecordSelection>,
    /// Selects the authored reputation display contexts.
    pub contexts: Option<CatalogRecordSelection>,
    /// Selects the authored reputation perk lists.
    pub perks: Option<CatalogRecordSelection>,
    /// Selects standalone manual reputation trigger Records.
    pub trigger_records: Option<CatalogRecordSelection>,
}

impl ReputationSelection {
    /// Selects every admitted row of the families supported by this query.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            faction_reputations: Some(CatalogRecordSelection::All),
            scopes: Some(CatalogRecordSelection::All),
            standings: Some(CatalogRecordSelection::All),
            contexts: Some(CatalogRecordSelection::All),
            perks: Some(CatalogRecordSelection::All),
            trigger_records: Some(CatalogRecordSelection::All),
        }
    }

    /// Selects every scope and every standing to support a complete rank index.
    #[must_use]
    pub const fn ranks() -> Self {
        Self {
            faction_reputations: None,
            scopes: Some(CatalogRecordSelection::All),
            standings: Some(CatalogRecordSelection::All),
            contexts: None,
            perks: None,
            trigger_records: None,
        }
    }
}

/// One requested reputation family and its original selection coverage.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ReputationRows<Row> {
    /// Availability measured from the original source family.
    pub availability: DatasetAvailability,
    /// Definitions in canonical GUID and physical Record order.
    pub rows: Vec<Row>,
    /// Unreadable and skipped records within the selected request.
    pub coverage: CatalogRecordCoverage,
}

/// Selected reputation definitions, text rows and their result-local source inventory.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ReputationSnapshot<'source> {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// The faction reputation definitions, when requested.
    pub faction_reputations: Option<ReputationRows<FactionReputationDefinition<'source>>>,
    /// The scope definitions, when requested.
    pub scopes: Option<ReputationRows<ScopeDefinition<'source>>>,
    /// The standing definitions, when requested.
    pub standings: Option<ReputationRows<StandingDefinition<'source>>>,
    /// The authored reputation display contexts, when requested.
    pub contexts: Option<ReputationRows<ReputationContextDefinition<'source>>>,
    /// The authored reputation perk lists, when requested.
    pub perks: Option<ReputationRows<ReputationPerkList<'source>>>,
    /// The standalone manual trigger Records, when requested.
    pub trigger_records: Option<ReputationRows<SandboxTriggerRecordDefinition<'source>>>,
    /// Canonical public-name entries referenced by the result.
    pub public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    /// Canonical protected text entries referenced by the result.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Requested locales including those absent from the Archive.
    pub requested_languages: Vec<Language>,
    /// Requested locales whose payloads were available and read.
    pub available_languages: Vec<Language>,
    /// Decoding and parsing notes from the localization payloads read by the query.
    pub localization_notes: Vec<LocalizationNote>,
    /// Original sources and uses retained by the selected projections.
    pub source_inventory: SourceInventory,
}

impl<'source> ReputationSnapshot<'source> {
    /// Resolves a protected text among entries referenced by this result.
    #[must_use]
    pub fn game_text(
        &self,
        reference: &GameTextRef,
    ) -> Option<&CanonicalText<'source, GameTextRef>> {
        self.game_texts
            .iter()
            .find(|row| row.id().value() == Some(reference))
    }

    /// Resolves a compatible standing link from this Archive among the selected standings.
    #[must_use]
    pub fn standing(
        &self,
        link: &SourceRecordReference<'_>,
    ) -> Option<&StandingDefinition<'source>> {
        let index = link.resolved_index()?;
        let guid = link.guid.value()?;
        self.standings.as_ref()?.rows.iter().find(|standing| {
            standing.source().record_index().value() == Some(&index)
                && standing.source().guid.value() == Some(guid)
        })
    }
}

/// Supplies selected authored reputation definitions and their texts to a callback.
/// Publish consumer-owned results after this function succeeds.
///
/// # Errors
/// Returns source, accounting or cancellation errors.
pub fn with_reputation<T>(
    archive: &Archive,
    selection: ReputationSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ReputationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_reputation(selection, cancel, inspect)
}

/// Supplies reputation definitions using an already decoded Entry from this Archive.
///
/// # Errors
/// Returns the errors of [`with_reputation`], or a foreign input error.
pub fn with_reputation_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: ReputationSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ReputationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_reputation(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected reputation definitions using the session's Archive and locale policy.
    ///
    /// # Errors
    /// Returns source, accounting or cancellation errors.
    pub fn with_reputation<T>(
        &self,
        selection: ReputationSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ReputationSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_reputation(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected reputation definitions using this view's parsed DataForge.
    ///
    /// # Errors
    /// Returns source, accounting or cancellation errors.
    pub fn with_reputation<T>(
        &self,
        selection: ReputationSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ReputationSnapshot<'_>) -> T,
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
    selection: ReputationSelection,
    requested_languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(ReputationSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let selected = SelectedReputation::read(forge, selection, cancel)?;
    let selected_languages = if selected.has_localized_records() {
        requested_languages
    } else {
        &[]
    };
    checkpoint(cancel)?;
    let sources = TextSources::read_languages(archive, selected_languages)?;
    checkpoint(cancel)?;
    let origins = CatalogOrigins::new(archive, entry, &sources)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry);
    let rows = selected.project(forge, &projection, cancel)?;
    let text_rows = projection.finish_rows()?;
    let available_languages = selected_languages
        .iter()
        .copied()
        .filter(|language| sources.entry(*language).is_some())
        .collect();
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(ReputationSnapshot {
        build: archive.build().cloned(),
        faction_reputations: rows.faction_reputations,
        scopes: rows.scopes,
        standings: rows.standings,
        contexts: rows.contexts,
        perks: rows.perks,
        trigger_records: rows.trigger_records,
        public_names: text_rows.public_names,
        game_texts: text_rows.game_texts,
        requested_languages: requested_languages.to_vec(),
        available_languages,
        localization_notes: texts.notes().to_vec(),
        source_inventory,
    }))
}
