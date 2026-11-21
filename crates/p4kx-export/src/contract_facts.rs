// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::contract_missions::StandingLevel;
use crate::guid_text::CatalogGuid;

/// The level that states a value of a contract; the variants run from the nearest level to the
/// farthest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
#[non_exhaustive]
pub enum FactLayer {
    /// The parameter overrides of the sub contract.
    SubContract,
    /// The parameter overrides of the contract.
    Contract,
    /// The parameter overrides or the `defaultAvailability` of the handler.
    Handler,
    /// The `ContractTemplate` Record.
    Template,
}

/// A selected value and its independently sourced level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required, rename = "Stated_{T}")]
#[non_exhaustive]
pub struct Stated<T> {
    /// The value.
    pub value: SourcedValue<T>,
    /// The first level of sub contract, contract, handler and template that states the value.
    pub layer: SourcedValue<FactLayer>,
}

/// A completion tag a contract requires and the intro contracts that grant it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct RequiredIntro {
    /// Where the prerequisite is stated.
    pub level: SourcedValue<StandingLevel>,
    /// The `Tag` Record the prerequisite requires among the completed contracts.
    pub tag_guid: SourcedValue<CatalogGuid>,
    /// The `id` of every intro contract (an element of `introContracts` of a handler) whose success
    /// result grants the tag, ordered by `id`; empty when none does.
    pub intro_contracts: SourcedValue<Vec<SourcedValue<CatalogGuid>>>,
}

/// The run terms and layered facts of one contract row.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ContractFacts {
    /// The `timeToComplete` of the contract's `contractResults`, as stored, in floating point;
    /// `null` when absent. A sub contract shares the contract's value. The Archive states no unit.
    pub time_to_complete: SourcedValue<f32>,
    /// The `contractBuyInAmount` of the contract's `contractResults`, as stored; `null` when
    /// absent. A sub contract shares the contract's value.
    pub buy_in_amount: SourcedValue<i64>,
    /// Independently sourced values and the levels that select them.
    #[serde(flatten)]
    pub layered: LayeredFacts,
}

/// The nearest usable overrides and original fallback facts of one contract row.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct LayeredFacts {
    /// Parameter `CanBeShared`; base: `canBeShared` of the template's contract class.
    pub can_be_shared: Stated<bool>,
    /// Parameter `Illegal`; base: `illegal` of the template's display info.
    pub illegal: Stated<bool>,
    /// Parameter `OnceOnly`; base: `onceOnly` of the handler's `defaultAvailability`.
    pub once_only: Stated<bool>,
    /// Parameter `MaxPlayersPerInstance`; base: `maxPlayersPerInstance` of the handler's
    /// `defaultAvailability`. A negative value is as stored.
    pub max_players_per_instance: Stated<i64>,
    /// Parameter `AvailableInPrison`; base: `availableInPrison` of the handler's
    /// `defaultAvailability`.
    pub available_in_prison: Stated<bool>,
    /// Parameter `CanReacceptAfterAbandoning`; base: `canReacceptAfterAbandoning` of the handler's
    /// `defaultAvailability`.
    pub can_reaccept_after_abandoning: Stated<bool>,
    /// Parameter `CanReacceptAfterFailing`; base: `canReacceptAfterFailing` of the handler's
    /// `defaultAvailability`.
    pub can_reaccept_after_failing: Stated<bool>,
    /// Parameter `HideInMobiGlas`; base: `hideInMobiGlas` of the handler's `defaultAvailability`.
    pub hide_in_mobi_glas: Stated<bool>,
}
