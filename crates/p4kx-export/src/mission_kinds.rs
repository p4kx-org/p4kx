// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, RecordTree};

use crate::Kind;
use crate::cargo_manifest::cargo_manifest;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::mission_scenario_definition::{
    self, MissionScenarioDefinition, PreparedMissionScenario,
};
use crate::catalog_v3::reputation_context_definition::{self, ContextDefinition, PreparedContext};
use crate::catalog_v3::reputation_faction_definition::{
    self, FactionReputationTables, PreparedFactionReputation,
};
use crate::catalog_v3::reputation_perk_list::{self, PreparedPerkList, ReputationPerkList};
use crate::catalog_v3::reputation_reward_amount_definition::{
    self, PreparedReputationRewardAmount, ReputationRewardAmountDefinition,
};
use crate::catalog_v3::reputation_scope_definition::{self, PreparedScope, ScopeDefinition};
use crate::catalog_v3::reputation_standing_definition::{
    self, PreparedStanding, StandingDefinition,
};
use crate::catalog_v3::sandbox_trigger_record_definition::{
    self, PreparedSandboxTriggerRecord, SandboxTriggerRecordDefinition,
};
use crate::contract::{CargoManifest, ContractScenario, MissionLocality, MissionType};
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::mission_data::{contract_scenario, mission_locality, mission_type};
use crate::texts::Texts;

/// An entity of one of the mission and reputation kinds.
pub(crate) enum MissionPick<'f> {
    /// A faction.
    Faction(Box<PreparedFactionReputation<'f>>),
    /// A reputation scope.
    Scope(Box<PreparedScope<'f>>),
    /// A reputation standing.
    Standing(Box<PreparedStanding<'f>>),
    /// A physical list of reputation perks.
    Perks(Box<PreparedPerkList<'f>>),
    /// A reusable sandbox trigger definition.
    SandboxTriggerRecord(Box<PreparedSandboxTriggerRecord<'f>>),
    /// A reusable reputation reward amount.
    RewardAmount(Box<PreparedReputationRewardAmount<'f>>),
    /// A reputation context.
    Context(Box<PreparedContext<'f>>),
    /// A mission type.
    Type(MissionType<'f>),
    /// A mission locality.
    Locality(MissionLocality<'f>),
    /// A contract scenario.
    Scenario(ContractScenario<'f>),
    /// An authored scenario definition.
    ScenarioDefinition(Box<PreparedMissionScenario<'f>>),
    /// A cargo manifest.
    Manifest(CargoManifest<'f>),
}

impl<'f> MissionPick<'f> {
    /// Returns the entity of `kind` the Record tree `tree` describes, `None` for a kind that is
    /// not a mission or reputation kind.
    pub(crate) fn entity(kind: Kind, tree: &RecordTree<'f>, texts: &Texts<'f>) -> Option<Self> {
        Some(match kind {
            Kind::MissionTypes => Self::Type(mission_type(tree, texts)),
            Kind::MissionLocalities => Self::Locality(mission_locality(tree)),
            Kind::ContractScenarios => Self::Scenario(contract_scenario(tree)),
            Kind::CargoManifests => Self::Manifest(cargo_manifest(tree)),
            _ => return None,
        })
    }
}

/// The entities of the mission and reputation kinds, each ordered by the text of the GUID.
#[derive(Default)]
pub(crate) struct MissionKinds<'f> {
    pub(crate) factions: Vec<PreparedFactionReputation<'f>>,
    pub(crate) scopes: Vec<PreparedScope<'f>>,
    pub(crate) standings: Vec<PreparedStanding<'f>>,
    pub(crate) perks: Vec<PreparedPerkList<'f>>,
    pub(crate) trigger_records: Vec<PreparedSandboxTriggerRecord<'f>>,
    pub(crate) reward_amounts: Vec<PreparedReputationRewardAmount<'f>>,
    pub(crate) contexts: Vec<PreparedContext<'f>>,
    pub(crate) types: Vec<MissionType<'f>>,
    pub(crate) localities: Vec<MissionLocality<'f>>,
    pub(crate) scenarios: Vec<ContractScenario<'f>>,
    pub(crate) scenario_definitions: Vec<PreparedMissionScenario<'f>>,
    pub(crate) manifests: Vec<CargoManifest<'f>>,
}

pub(crate) struct ReputationTables<'source> {
    pub(crate) factions: FactionReputationTables<'source>,
    pub(crate) contexts: CatalogTable<ContextDefinition<'source>>,
    pub(crate) scopes: CatalogTable<ScopeDefinition<'source>>,
    pub(crate) standings: CatalogTable<StandingDefinition<'source>>,
    pub(crate) perks: CatalogTable<ReputationPerkList<'source>>,
    pub(crate) trigger_records: CatalogTable<SandboxTriggerRecordDefinition<'source>>,
    pub(crate) reward_amounts: CatalogTable<ReputationRewardAmountDefinition<'source>>,
}

impl<'f> MissionKinds<'f> {
    /// Stores `pick` under its kind.
    pub(crate) fn push(&mut self, pick: MissionPick<'f>) {
        match pick {
            MissionPick::Faction(entity) => self.factions.push(*entity),
            MissionPick::Scope(entity) => self.scopes.push(*entity),
            MissionPick::Standing(entity) => self.standings.push(*entity),
            MissionPick::Perks(entity) => self.perks.push(*entity),
            MissionPick::SandboxTriggerRecord(entity) => self.trigger_records.push(*entity),
            MissionPick::RewardAmount(entity) => self.reward_amounts.push(*entity),
            MissionPick::Context(entity) => self.contexts.push(*entity),
            MissionPick::Type(entity) => self.types.push(entity),
            MissionPick::Locality(entity) => self.localities.push(entity),
            MissionPick::Scenario(entity) => self.scenarios.push(entity),
            MissionPick::ScenarioDefinition(entity) => self.scenario_definitions.push(*entity),
            MissionPick::Manifest(entity) => self.manifests.push(entity),
        }
    }

    /// Orders every kind by the text of the GUID.
    pub(crate) fn order(&mut self) {
        self.factions
            .sort_by_cached_key(|row| (CatalogGuid(row.physical_guid).text(), row.physical_index));
        self.scopes
            .sort_by_cached_key(|row| (CatalogGuid(row.physical_guid).text(), row.physical_index));
        self.standings
            .sort_by_cached_key(|row| (CatalogGuid(row.physical_guid).text(), row.physical_index));
        self.perks
            .sort_by_cached_key(|row| (CatalogGuid(row.physical_guid).text(), row.physical_index));
        self.contexts
            .sort_by_cached_key(|row| (CatalogGuid(row.physical_guid).text(), row.physical_index));
        self.trigger_records
            .sort_by_cached_key(|row| (CatalogGuid(row.physical_guid).text(), row.physical_index));
        self.reward_amounts
            .sort_by_cached_key(|row| (CatalogGuid(row.physical_guid).text(), row.physical_index));
        sort_by_guid(&mut self.types, |entity| entity.guid);
        sort_by_guid(&mut self.localities, |entity| entity.guid);
        sort_by_guid(&mut self.scenarios, |entity| entity.guid);
        sort_by_guid(&mut self.manifests, |entity| entity.guid);
    }

    /// Takes the physical mission scenario definitions into their catalog table.
    pub(crate) fn take_scenario_definitions(
        &mut self,
        forge: &DataForge<'_>,
    ) -> CatalogTable<MissionScenarioDefinition<'f>> {
        mission_scenario_definition::table(forge, std::mem::take(&mut self.scenario_definitions))
    }

    /// Takes the physical reputation definitions into their canonical catalog tables.
    pub(crate) fn take_reputation_tables(&mut self, forge: &DataForge<'_>) -> ReputationTables<'f> {
        ReputationTables {
            factions: reputation_faction_definition::tables(
                forge,
                std::mem::take(&mut self.factions),
            ),
            contexts: reputation_context_definition::table(
                forge,
                std::mem::take(&mut self.contexts),
            ),
            scopes: reputation_scope_definition::table(forge, std::mem::take(&mut self.scopes)),
            standings: reputation_standing_definition::table(
                forge,
                std::mem::take(&mut self.standings),
            ),
            perks: reputation_perk_list::table(forge, std::mem::take(&mut self.perks)),
            trigger_records: sandbox_trigger_record_definition::table(
                forge,
                std::mem::take(&mut self.trigger_records),
            ),
            reward_amounts: reputation_reward_amount_definition::table(
                forge,
                std::mem::take(&mut self.reward_amounts),
            ),
        }
    }

    /// Returns original numeric receipts attached to emitted reputation definitions.
    pub(crate) fn reputation_number_groups(
        &self,
    ) -> impl Iterator<Item = (Kind, &[CapturedNumber])> {
        self.factions
            .iter()
            .map(|row| (Kind::FactionReputations, row.numbers.as_slice()))
            .chain(
                self.scopes
                    .iter()
                    .map(|row| (Kind::ReputationScopes, row.numbers.as_slice())),
            )
            .chain(
                self.standings
                    .iter()
                    .map(|row| (Kind::ReputationStandings, row.numbers.as_slice())),
            )
            .chain(
                self.perks
                    .iter()
                    .map(|row| (Kind::ReputationPerks, row.numbers.as_slice())),
            )
            .chain(
                self.contexts
                    .iter()
                    .map(|row| (Kind::ReputationContexts, row.numbers.as_slice())),
            )
            .chain(
                self.trigger_records
                    .iter()
                    .map(|row| (Kind::ReputationTriggerRecords, row.numbers.as_slice())),
            )
            .chain(
                self.reward_amounts
                    .iter()
                    .map(|row| (Kind::ReputationRewardAmounts, row.numbers.as_slice())),
            )
    }

    /// Returns how many entities `kind` holds, 0 for a kind that is not one of these.
    pub(crate) const fn count(&self, kind: Kind) -> usize {
        match kind {
            Kind::FactionReputations => self.factions.len(),
            Kind::ReputationScopes => self.scopes.len(),
            Kind::ReputationStandings => self.standings.len(),
            Kind::ReputationPerks => self.perks.len(),
            Kind::ReputationTriggerRecords => self.trigger_records.len(),
            Kind::ReputationRewardAmounts => self.reward_amounts.len(),
            Kind::ReputationContexts => self.contexts.len(),
            Kind::MissionTypes => self.types.len(),
            Kind::MissionLocalities => self.localities.len(),
            Kind::ContractScenarios => self.scenarios.len(),
            Kind::MissionScenarios => self.scenario_definitions.len(),
            Kind::CargoManifests => self.manifests.len(),
            _ => 0,
        }
    }
}
