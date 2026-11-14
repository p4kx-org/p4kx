// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::contract_missions::{ContractFaction, PickupPlace};
use crate::guid_text::CatalogGuid;

/// Everywhere a contract is offered, as its prerequisites state it and without a cap.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct OfferedAt<'a> {
    /// The `MissionLocality` Records of the `ContractPrerequisite_Locality` prerequisites, the
    /// `guid`s of entries of `missionLocalities`, in stored order, none twice. A locality names
    /// its places through `starmapObjectGuids`.
    pub(crate) locality_guids: Vec<CatalogGuid>,
    /// The objects of the `ContractPrerequisite_Location` prerequisites, in stored order, none
    /// twice, a star and an object that is not in `starmapObjects` included (its `objectType` and
    /// `place` are then `null`). When the contract states no place at all, the
    /// `locationMissionAvailable` of its `MissionBrokerEntry`, and `brokerFallback` is `true`.
    pub(crate) locations: Vec<PickupPlace<'a>>,
    /// Whether `locations` is the location of the broker entry because the contract's
    /// prerequisites state no place, not a location the prerequisites state.
    pub(crate) broker_fallback: bool,
}

/// Where the faction of a contract was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum GiverSource {
    /// The `factionReputation` of the handler.
    Handler,
    /// The faction of the first reputation amount the contract grants on success whose
    /// `reputationAmount` is positive.
    ReputationResult,
    /// The faction of the first `ContractPrerequisite_Reputation` that is not an exclusion.
    ReputationPrerequisite,
}

/// The faction a contract is stated for, with the place that states it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GiverFaction<'a> {
    /// Where the faction was found.
    pub(crate) source: GiverSource,
    /// The faction.
    pub(crate) faction: ContractFaction<'a>,
}

/// The struct that states a blueprint reward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
pub(crate) enum BlueprintSource {
    /// A `BlueprintRewards` result that fires on success.
    Result,
    /// An enabled `MissionModifier_TemporaryBlueprint` of the parameter overrides.
    TemporaryModifier,
}

/// A blueprint pool a contract hands out.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintReward {
    /// The `guid` of the entry of `blueprintPools` the reward draws from.
    pub(crate) pool_guid: CatalogGuid,
    /// The struct that states the reward.
    pub(crate) source: BlueprintSource,
    /// The `chance` of the reward as stored, `null` when it states none (a modifier has none).
    pub(crate) chance: Option<f32>,
}
