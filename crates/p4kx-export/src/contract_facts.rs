// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

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
pub(crate) enum FactLayer {
    /// The parameter overrides of the sub contract.
    SubContract,
    /// The parameter overrides of the contract.
    Contract,
    /// The parameter overrides or the `defaultAvailability` of the handler.
    Handler,
    /// The `ContractTemplate` Record.
    Template,
}

/// A value as stored, with the level that states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required, rename = "Stated_{T}")]
pub(crate) struct Stated<T> {
    /// The value.
    pub(crate) value: T,
    /// The first level of sub contract, contract, handler and template that states the value.
    pub(crate) layer: FactLayer,
}

/// A completion tag a contract requires and the intro contracts that grant it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RequiredIntro {
    /// Where the prerequisite is stated.
    pub(crate) level: StandingLevel,
    /// The `Tag` Record the prerequisite requires among the completed contracts.
    pub(crate) tag_guid: CatalogGuid,
    /// The `id` of every intro contract (an element of `introContracts` of a handler) whose success
    /// result grants the tag, ordered by `id`; empty when none does.
    pub(crate) intro_contracts: Vec<CatalogGuid>,
}

/// The contract facts of an entry. The `Stated` values are read from the nearest of the
/// sub contract, the contract and the handler that overrides the parameter of the same name (case
/// is ignored), else from the base value named on the field; a sub contract states its
/// parameters directly. `null` means no level states the value.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContractFacts {
    /// The `timeToComplete` of the contract's `contractResults`, as stored, in floating point;
    /// `null` when absent. A sub contract shares the contract's value. The Archive states no unit.
    pub(crate) time_to_complete: Option<f32>,
    /// The `contractBuyInAmount` of the contract's `contractResults`, as stored; `null` when
    /// absent. A sub contract shares the contract's value.
    pub(crate) buy_in_amount: Option<i64>,
    /// Parameter `CanBeShared`; base: `canBeShared` of the template's contract class.
    pub(crate) can_be_shared: Option<Stated<bool>>,
    /// Parameter `Illegal`; base: `illegal` of the template's display info.
    pub(crate) illegal: Option<Stated<bool>>,
    /// Parameter `OnceOnly`; base: `onceOnly` of the handler's `defaultAvailability`.
    pub(crate) once_only: Option<Stated<bool>>,
    /// Parameter `MaxPlayersPerInstance`; base: `maxPlayersPerInstance` of the handler's
    /// `defaultAvailability`. A negative value is as stored.
    pub(crate) max_players_per_instance: Option<Stated<i64>>,
    /// Parameter `AvailableInPrison`; base: `availableInPrison` of the handler's
    /// `defaultAvailability`.
    pub(crate) available_in_prison: Option<Stated<bool>>,
    /// Parameter `CanReacceptAfterAbandoning`; base: `canReacceptAfterAbandoning` of the handler's
    /// `defaultAvailability`.
    pub(crate) can_reaccept_after_abandoning: Option<Stated<bool>>,
    /// Parameter `CanReacceptAfterFailing`; base: `canReacceptAfterFailing` of the handler's
    /// `defaultAvailability`.
    pub(crate) can_reaccept_after_failing: Option<Stated<bool>>,
    /// Parameter `HideInMobiGlas`; base: `hideInMobiGlas` of the handler's `defaultAvailability`.
    pub(crate) hide_in_mobi_glas: Option<Stated<bool>>,
    /// The `ContractPrerequisite_CompletedContractTags` tags of the handler, the contract and the
    /// sub contract, in that order and as stored, each with the intro contracts that grant the tag.
    /// The catalog does not decide how the levels combine.
    pub(crate) required_intros: Vec<RequiredIntro>,
}
