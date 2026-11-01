// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::contract_hauling::HaulingOrder;
use crate::guid_text::CatalogGuid;
use crate::spawn_model::{LocationQueryLink, SpawnPoolLink};
use crate::template_extras::{EndComms, PartialPayout, StartComms, TemplateOwner};
use crate::template_flow::FlowTrigger;
use crate::template_modifier::TemplateModifier;
use crate::template_objective::{ObjectiveState, TemplateObjective};

/// A contract template: a `ContractTemplate` Record under `contracts/contracttemplates`, the
/// structure a contract names in `templateGuid` and lays its own overrides over.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ContractTemplate<'a> {
    /// The GUID of the Record; the `templateGuid` of the contracts that name it.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `AcePilot` for `ContractTemplate.AcePilot`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The `notForRelease` flag as stored, `null` when absent.
    pub(crate) not_for_release: Option<bool>,
    /// The `workInProgress` flag as stored, `null` when absent.
    pub(crate) work_in_progress: Option<bool>,
    /// The first display string of the template; `null` text for a placeholder.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) title: LocalisedText<'a>,
    /// The `MissionType` Record of the display info, an entry of `missionTypes`; the GUID is kept
    /// for a Record the Archive lacks.
    pub(crate) mission_type_guid: Option<CatalogGuid>,
    /// The `illegal` flag of the display info, `null` when absent.
    pub(crate) illegal: Option<bool>,
    /// The `showLifeTimeInMobiGlas` flag of the display info, `null` when absent.
    pub(crate) show_life_time_in_mobi_glas: Option<bool>,
    /// The `preShowObjectives` flag of the display info, `null` when absent.
    pub(crate) pre_show_objectives: Option<bool>,
    /// The struct of the `contractClass`, e.g. `ContractClass_Contract`; `null` when absent.
    pub(crate) class_type: Option<String>,
    /// The `ActiveContractSettings` of the contract class, `null` when it has none.
    pub(crate) settings: Option<ActiveSettings>,
    /// The `autoFinishSettings` of the contract class, `null` when it has none.
    pub(crate) auto_finish: Option<AutoFinish>,
    /// The mission properties of `contractProperties` in stored order: the variables the
    /// contracts, handlers and sub contracts override.
    pub(crate) contract_properties: Vec<TemplateProperty>,
    /// The ship spawn descriptions the template states, one per variable, ordered by variable;
    /// `level` is always `template`.
    pub(crate) ship_spawns: Vec<SpawnPoolLink>,
    /// The NPC spawn descriptions the template states, resolved like `shipSpawns`.
    pub(crate) npc_spawns: Vec<SpawnPoolLink>,
    /// The entity spawn descriptions the template states, resolved like `shipSpawns`.
    pub(crate) entity_spawns: Vec<SpawnPoolLink>,
    /// The location queries the template states, resolved like `shipSpawns`.
    pub(crate) location_queries: Vec<LocationQueryLink>,
    /// The hauling orders of the template: the hauling properties (`template`) and the orders of
    /// its hauling objectives (`templateObjective`).
    pub(crate) hauling_orders: Vec<HaulingOrder<'a>>,
    /// The objective tokens in depth-first order, each phase after its parent.
    pub(crate) objectives: Vec<TemplateObjective<'a>>,
    /// The `partialRewardPayout`, `null` when the template has none.
    pub(crate) partial_payout: Option<PartialPayout>,
    /// The `modifiers` in stored order.
    pub(crate) modifiers: Vec<TemplateModifier>,
    /// The `owner` of the Record, `null` when it names none.
    pub(crate) owner: Option<TemplateOwner>,
    /// The `missionFlow` triggers in stored order.
    pub(crate) mission_flow: Vec<FlowTrigger>,
    /// The `contractStartCommsNotification`, `null` when absent.
    pub(crate) start_comms: Option<StartComms>,
    /// The `contractEndCommsNotifications` in stored order.
    pub(crate) end_comms: Vec<EndComms>,
}

/// The `ActiveContractSettings` of a template, each flag as stored.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ActiveSettings {
    /// The `canBeShared` flag.
    pub(crate) can_be_shared: Option<bool>,
    /// The `hasCompleteButton` flag.
    pub(crate) has_complete_button: Option<bool>,
    /// The `handlesAbandonRequest` flag.
    pub(crate) handles_abandon_request: Option<bool>,
    /// The `displayAlliedMarkers` flag.
    pub(crate) display_allied_markers: Option<bool>,
    /// The `onlyOwnerCanComplete` flag.
    pub(crate) only_owner_can_complete: Option<bool>,
}

/// The `ContractAutoFinishSettings` of a template, each value as stored.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AutoFinish {
    /// The `failIfSentToPrison` flag.
    pub(crate) fail_if_sent_to_prison: Option<bool>,
    /// The `failIfBecameCriminal` flag.
    pub(crate) fail_if_became_criminal: Option<bool>,
    /// The `failIfLeavePrison` flag.
    pub(crate) fail_if_leave_prison: Option<bool>,
    /// The `missionCompletionTime` of the deadline, as stored; the Archive does not state its
    /// unit. `null` when the template has no deadline.
    pub(crate) completion_time: Option<i64>,
    /// The `missionAutoEnd` flag of the deadline.
    pub(crate) auto_end: Option<bool>,
    /// The `missionResultAfterTimerEnd` of the deadline: the state the mission ends in.
    pub(crate) result_after_timer_end: Option<ObjectiveState>,
    /// The `remainingTimeToShowTimer` of the deadline, as stored.
    pub(crate) remaining_time_to_show_timer: Option<i64>,
}

/// A mission property of a template: a variable and the kind of value it holds.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateProperty {
    /// The `missionVariableName`, `null` when empty.
    pub(crate) variable: Option<String>,
    /// The `extendedTextToken`, `null` when empty.
    pub(crate) token: Option<String>,
    /// The struct of the value without its `MissionPropertyValue_` start, e.g. `Location` or
    /// `ShipSpawnDescriptions`; `null` when the property has no value.
    pub(crate) value_type: Option<String>,
}
