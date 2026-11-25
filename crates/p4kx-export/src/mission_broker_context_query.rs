// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Scoped original mission Broker owners and their canonical contextual projections.

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Language, LocalizationNote,
    SourceInventory,
};

use super::unlocalized_record_query::plan::{Owner, Plan};
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::broker_canonical_read::{self, BrokerEmission, CanonicalBroker, Sources};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_query_session::{
    CatalogQuerySession, CatalogQuerySource, ParsedCatalogQueries, checkpoint,
};
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::catalog_text_contract::{CanonicalText, GameTextRef};
use crate::export_diagnostic::ExportSourceRole;
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::mission_index::BrokerRewardReads;
use crate::record_observation::RecordObservation;
use crate::template_issues::Unknowns;
use crate::template_tag_cache::{DeferredTemplateTags, TemplateTagInput};
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, CatalogReadFailure, Kind};

pub use crate::broker_definition::BrokerDefinition;
pub use crate::broker_entry::BrokerProjection;
pub use crate::broker_reputation_read::{BrokerReputationAmount, BrokerReputationResult};
pub use crate::mission_flow_owner::MissionFlowOwnerRef;
pub use crate::resource_integer::ResourceInt64;

/// Canonical context associated with the queried mission Broker Records.
#[non_exhaustive]
pub struct MissionBrokerContextSnapshot<'source> {
    /// The installed Archive Build, when known.
    pub build: Option<Build>,
    /// Availability of the declared Broker source family.
    pub availability: DatasetAvailability,
    /// Successfully interpreted canonical contextual rows.
    pub rows: Vec<BrokerProjection<'source>>,
    /// Original owners, including those whose required contextual interpretation failed.
    pub owners: Vec<BrokerDefinition<'source>>,
    /// Protected text rows registered while interpreting these owners.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Requested locales actually read for these owners.
    pub available_languages: Vec<Language>,
    /// Decoding and parsing notes for the locale inputs.
    pub localization_notes: Vec<LocalizationNote>,
    /// Original inputs and exact uses backing the rows.
    pub source_inventory: SourceInventory,
    /// Physical selection, omissions, and incomplete fields.
    pub coverage: CatalogRecordCoverage,
    /// Actual failures while interpreting the selected Broker context.
    pub contextual_failures: Vec<CatalogReadFailure<'source>>,
    /// Actual failures reading the bodies of reached reward Records.
    pub reward_failures: Vec<CatalogReadFailure<'source>>,
    /// Reached reward Records whose decoded fields had projection issues.
    pub incomplete_reward_fields: Vec<u32>,
    /// Whether the selected context required a Tag ancestry survey.
    pub tag_ancestry_surveyed: bool,
    /// Tag source failures from that survey, when it ran.
    pub tag_failures: Vec<CatalogReadFailure<'source>>,
}

impl<'source> MissionBrokerContextSnapshot<'source> {
    /// Resolves the physical Broker owner and its original source arena.
    #[must_use]
    pub fn owner(&self, reference: &MissionFlowOwnerRef) -> Option<&BrokerDefinition<'source>> {
        let MissionFlowOwnerRef::MissionBrokerEntry(reference) = reference else {
            return None;
        };
        self.owners.iter().find(|owner| {
            owner.input_ref() == reference.input_ref()
                && owner.header().record_index().value() == Some(&reference.record_index())
                && owner.header().guid().value() == Some(&reference.record_guid().get())
        })
    }
}

/// Supplies canonical Broker context and original owners for one physical selection.
///
/// # Errors
/// Returns Archive, accounting, source evidence or cancellation errors.
pub fn with_mission_broker_context<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MissionBrokerContextSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, languages, cancel)?
        .with_mission_broker_context(selection, cancel, inspect)
}

/// Supplies canonical Broker context using an already decoded DataForge Entry.
///
/// # Errors
/// Returns query errors or an origin mismatch for a foreign Entry.
pub fn with_mission_broker_context_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    languages: CatalogLanguageSelection<'_>,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MissionBrokerContextSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(archive, entry, languages, cancel)?
        .with_mission_broker_context(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies canonical Broker context through this Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_mission_broker_context`].
    pub fn with_mission_broker_context<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MissionBrokerContextSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_mission_broker_context(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies canonical Broker context through this parsed DataForge view.
    ///
    /// # Errors
    /// Returns the errors of [`with_mission_broker_context`].
    pub fn with_mission_broker_context<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MissionBrokerContextSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            project(
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

fn project<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MissionBrokerContextSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let kind = Kind::MissionBrokerEntries;
    let Plan {
        mut owners,
        coverage,
    } = Plan::read_rule(forge, kind.records(), selection, cancel, None)?;
    sort_by_guid(&mut owners, |owner| CatalogGuid(owner.record.guid()));
    let languages = if owners.is_empty() { &[] } else { languages };
    let text_sources = TextSources::read_languages(archive, languages)?;
    checkpoint(cancel)?;
    let origins = entry.with_texts(archive, &text_sources)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&text_sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(kind);
    let tags = DeferredTemplateTags::new(forge, &input, cancel);
    let mut rewards = BrokerRewardReads::new();
    let mut read = Selected {
        coverage,
        ..Selected::default()
    };
    read.project(
        owners,
        &projection,
        TemplateTagInput::OnDemand(&tags),
        &mut rewards,
        cancel,
    )?;
    let table = CatalogTable::new(forge, &[kind.records().type_name], read.rows);
    let text_rows = projection.finish_rows()?;
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    let available_languages = languages
        .iter()
        .copied()
        .filter(|language| text_sources.entry(*language).is_some())
        .collect();
    let (reward_failures, incomplete_reward_fields) = rewards.finish();
    Ok(inspect(MissionBrokerContextSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        owners: read.owners,
        game_texts: text_rows.game_texts,
        available_languages,
        localization_notes: texts.notes().to_vec(),
        source_inventory,
        coverage: read.coverage,
        contextual_failures: read.failures,
        reward_failures,
        incomplete_reward_fields,
        tag_ancestry_surveyed: tags.was_read(),
        tag_failures: tags.into_failures(),
    }))
}

#[derive(Default)]
struct Selected<'source> {
    rows: Vec<BrokerProjection<'source>>,
    owners: Vec<BrokerDefinition<'source>>,
    failures: Vec<CatalogReadFailure<'source>>,
    coverage: CatalogRecordCoverage,
}

impl<'source> Selected<'source> {
    fn project(
        &mut self,
        owners: Vec<Owner<'source>>,
        projection: &CatalogProjection<'_, 'source>,
        tags: TemplateTagInput<'_, 'source>,
        rewards: &mut BrokerRewardReads<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let unknowns = Unknowns::default();
        let input = projection.original();
        for Owner { record, tree } in owners {
            checkpoint(cancel)?;
            let admission = RecordObservation::capture(input, &record)?;
            let original = crate::export_field_walk::inspect_observed(
                Some(input),
                &tree,
                &admission,
                Some(Kind::MissionBrokerEntries),
                ExportSourceRole::Selected,
                cancel,
            )?;
            let mut lookup =
                |reference: &crate::catalog_read::original::OriginalReferenceObservation<
                    '_,
                    '_,
                    '_,
                    'source,
                >| { rewards.retain(input, reference, cancel) };
            let mut context = Sources {
                projection,
                tags,
                rewards: &mut lookup,
                unknowns: &unknowns,
            };
            let piece = broker_canonical_read::read(&mut context, &record, &tree, cancel)?;
            let complete = self.append(piece);
            if !original.field_projection_complete(0, 0) || !complete {
                self.coverage.incomplete_fields.push(record.index());
            }
        }
        Ok(())
    }

    fn append(&mut self, piece: CanonicalBroker<'source>) -> bool {
        self.owners.push(piece.definition);
        match piece.projection {
            Ok(BrokerEmission {
                projection,
                ledger,
                failures,
                ..
            }) => {
                let complete = ledger.field_projection_complete(0, 0) && failures.is_empty();
                self.rows.push(projection);
                self.failures.extend(failures);
                complete
            }
            Err(failure) => {
                self.failures.push(failure.failure);
                false
            }
        }
    }
}
