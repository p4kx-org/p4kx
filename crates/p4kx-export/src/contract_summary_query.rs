// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Build, CancelToken, Language, LocalizationNote, SourceInventory, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::{
    BlueprintReward, ContractCooldown, ContractKey, ContractQueryCoverage, ContractWithTitles,
    FixedReward, GiverFaction, LocationQuery, LocationQueryLink, OfferedAt, ProjectedMissionText,
    QueryOutput, QueryRequest, ReputationGain, StandingRequirement, Stated,
};
use super::{plan, read, read_parsed};
use crate::CatalogError;
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries};
use crate::catalog_table::DatasetAvailability;
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef};
use crate::guid_text::CatalogGuid;

/// Canonical contract identity, offer, run terms, texts and rewards.
#[derive(Debug, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ContractSummary<'source> {
    /// The physical Generator Record position in this Archive.
    pub source_record_index: u32,
    /// The original parent contract identity.
    pub guid: SourcedValue<CatalogGuid>,
    /// The original sub-contract identity, absent on a parent row.
    pub sub_contract_guid: SourcedValue<CatalogGuid>,
    /// The original contract or sub-contract debug name, when stated.
    pub debug_name: Option<&'source str>,
    /// The parent contract's release exclusion flag, when stated.
    pub not_for_release: Option<bool>,
    /// The parent contract's work-in-progress flag, when stated.
    pub work_in_progress: Option<bool>,
    /// Blueprint pool links and their stated chances.
    pub blueprint_rewards: Vec<BlueprintReward>,
    /// The ordered successful fixed payouts of the parent contract.
    pub fixed_rewards: SourcedValue<Vec<FixedReward<'source>>>,
    /// The faction of the contract and the source of that selection.
    pub giver_faction: Option<GiverFaction<'source>>,
    /// The authored offer locations, including the canonical broker fallback.
    pub offered_at: OfferedAt<'source>,
    /// The nearest stated shareability value and its original layer.
    pub can_be_shared: Stated<bool>,
    /// The resolved cooldown values and their original source layers.
    #[serde(flatten)]
    pub cooldown: ContractCooldown,
    /// The original buy-in amount of the parent contract.
    pub buy_in_amount: SourcedValue<i64>,
    /// Whether the contract can be accepted again after abandoning it and where that is stated.
    pub can_reaccept_after_abandoning: Stated<bool>,
    /// Whether the contract can be accepted again after failing it and where that is stated.
    pub can_reaccept_after_failing: Stated<bool>,
    /// The original minimum standing GUID of the parent contract, when stated.
    pub min_standing_guid: Option<CatalogGuid>,
    /// The original maximum standing GUID of the parent contract, when stated.
    pub max_standing_guid: Option<CatalogGuid>,
    /// Authored faction and scope requirements with standing bounds, in source layer order.
    pub standing_requirements: Vec<StandingRequirement>,
    /// Links to the authored location queries used by the contract.
    pub location_queries: Vec<LocationQueryLink>,
    /// The contract's ordered reputation amounts granted on success.
    pub reputation_gains: SourcedValue<Vec<ReputationGain<'source>>>,
    /// The protected canonical title key and localized title.
    #[schemars(extend("x-gameText" = true))]
    pub title: ProjectedMissionText<'source>,
    /// The protected canonical description key and localized description.
    #[schemars(extend("x-gameText" = true))]
    pub description: ProjectedMissionText<'source>,
}

impl<'source> ContractSummary<'source> {
    pub(super) fn from_row(
        ContractWithTitles { row, titles }: ContractWithTitles<'source>,
    ) -> Self {
        let base = row.base;
        Self {
            source_record_index: base.source.record_index(),
            guid: base.guid,
            sub_contract_guid: base.sub_contract_guid,
            debug_name: base.debug_name,
            not_for_release: base.not_for_release,
            work_in_progress: base.work_in_progress,
            blueprint_rewards: base.blueprint_rewards,
            fixed_rewards: base.fixed_rewards,
            giver_faction: base.giver_faction,
            offered_at: base.offered_at,
            can_be_shared: base.facts.layered.can_be_shared,
            cooldown: base.cooldown,
            buy_in_amount: base.facts.buy_in_amount,
            can_reaccept_after_abandoning: base.facts.layered.can_reaccept_after_abandoning,
            can_reaccept_after_failing: base.facts.layered.can_reaccept_after_failing,
            min_standing_guid: base.min_standing_guid,
            max_standing_guid: base.max_standing_guid,
            standing_requirements: base.standing_requirements,
            location_queries: base.location_queries,
            reputation_gains: base.reputation_gains,
            title: titles.title,
            description: titles.description,
        }
    }
}

/// Canonical contract summaries with their location queries and protected text lookups.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct ContractSummaries<'source> {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of the original Generator source family.
    pub availability: DatasetAvailability,
    /// Selected globally canonical contracts in ordinary catalog order.
    pub rows: Vec<ContractSummary<'source>>,
    /// Authored location queries projected from admitted source dependencies.
    pub location_queries: Vec<LocationQuery>,
    /// Public names published during canonical contract projection.
    pub public_names: Vec<CanonicalText<'source, PublicNameRef>>,
    /// Protected texts published during canonical contract projection.
    pub game_texts: Vec<CanonicalText<'source, GameTextRef>>,
    /// Requested localization policy, including unavailable locales.
    pub requested_languages: Vec<Language>,
    /// Requested localization payloads actually available and read.
    pub available_languages: Vec<Language>,
    /// Notes from the localization inputs actually consulted.
    pub localization_notes: Vec<LocalizationNote>,
    /// Original physical input evidence under the session's provenance policy.
    pub source_inventory: SourceInventory,
    /// Explicit root, dependency, failure and duplicate-selection coverage.
    pub coverage: ContractQueryCoverage<'source>,
}

impl<'source> ContractSummaries<'source> {
    fn from_query(query: QueryOutput<'source>) -> Self {
        Self {
            build: query.build,
            availability: query.availability,
            rows: query.projected.summaries,
            location_queries: query.projected.locations,
            public_names: query.public_names,
            game_texts: query.game_texts,
            requested_languages: query.requested_languages,
            available_languages: query.available_languages,
            localization_notes: query.localization_notes,
            source_inventory: query.source_inventory,
            coverage: query.projected.coverage,
        }
    }
}

impl CatalogQuerySession<'_> {
    /// Supplies canonical contract summaries for selected physical Generator owners.
    /// Publish consumer-owned changes after the call returns successfully.
    ///
    /// # Errors
    /// Returns input, source evidence, Archive change or cancellation errors.
    pub fn with_contract_summaries<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractSummaries<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_contract_summaries(selection, cancel, inspect)
    }

    /// Supplies the canonical summary for one logical parent or sub-contract identity.
    ///
    /// # Errors
    /// Returns the errors of `with_contract_summaries`.
    pub fn with_contract_summary<T>(
        &self,
        key: ContractKey,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractSummaries<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_contract_summary(key, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies summaries using this view's retained DataForge indexes.
    ///
    /// # Errors
    /// Returns the errors of `CatalogQuerySession::with_contract_summaries`.
    pub fn with_contract_summaries<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractSummaries<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.with_summaries(plan::Lookup::Owners(selection), cancel, inspect)
    }

    /// Supplies one logical summary using this view's retained DataForge indexes.
    ///
    /// # Errors
    /// Returns the errors of `CatalogQuerySession::with_contract_summary`.
    pub fn with_contract_summary<T>(
        &self,
        key: ContractKey,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractSummaries<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.with_summaries(plan::Lookup::Logical(key), cancel, inspect)
    }

    fn with_summaries<T>(
        &self,
        selection: plan::Lookup,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(ContractSummaries<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            read_parsed(
                archive,
                entry,
                forge,
                QueryRequest {
                    selection,
                    retention: read::Retention::Summaries,
                },
                self.languages(),
                cancel,
                |query| inspect(ContractSummaries::from_query(query)),
            )
        })
    }
}
