// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::broker_identity::{Names, Scripting};
use crate::broker_links::{Links, Tags};
use crate::broker_requirements::{ReputationResult, StandingTerm};
use crate::broker_schedule::ScheduleWindow;
use crate::broker_terms::{Availability, Cooldowns, Finish, InstanceLimits, MissionReward};
use crate::contract::every_field_required;
use crate::contract_hauling::HaulingOrder;
use crate::contract_template::{ActiveSettings, TemplateProperty};
use crate::guid_text::CatalogGuid;
use crate::spawn_model::{LocationQueryLink, ShipSpawnLink};
use crate::template_extras::{PartialPayout, TemplateOwner};
use crate::template_flow::FlowTrigger;
use crate::template_modifier::TemplateModifier;
use crate::template_objective::TemplateObjective;

/// A mission broker entry: a `MissionBrokerEntry` Record, an offer of the mission board. A
/// contract names it in `brokerEntryGuid`; one without a template takes its title, type and
/// location from it. The grouped fields (`names`, `links`, `module` and `tags`) are written flat.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BrokerEntry<'a> {
    /// The GUID of the Record; the `brokerEntryGuid` of the contracts that name it.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The `notForRelease` flag as stored, `null` when absent.
    pub(crate) not_for_release: Option<bool>,
    /// The names and game texts.
    #[serde(flatten)]
    pub(crate) names: Names<'a>,
    /// The links to other Records.
    #[serde(flatten)]
    pub(crate) links: Links,
    /// The mission scripting module.
    #[serde(flatten)]
    pub(crate) module: Scripting,
    /// The tags the entry grants and asks for.
    #[serde(flatten)]
    pub(crate) tags: Tags,
    /// The `owner` of the Record, `null` when it names none.
    pub(crate) owner: Option<TemplateOwner>,
    /// The `missionDifficulty`.
    pub(crate) difficulty: Option<i64>,
    /// The `preShowObjectives` flag.
    pub(crate) pre_show_objectives: Option<bool>,
    /// The `showLifeTimeInMobiGlas` flag.
    pub(crate) show_life_time_in_mobi_glas: Option<bool>,
    /// The active contract settings: `canBeShared`, `hasCompleteButton`, `handlesAbandonRequest`,
    /// `displayAlliedMarkers` and `onlyOwnerCanComplete`.
    pub(crate) settings: ActiveSettings,
    /// When and to whom the entry is offered.
    pub(crate) availability: Availability,
    /// The windows the entry is offered in, in stored order.
    pub(crate) schedule: Vec<ScheduleWindow>,
    /// The cooldowns and the respawn time.
    pub(crate) cooldowns: Cooldowns,
    /// How many instances run and for how long.
    pub(crate) instances: InstanceLimits,
    /// The `missionReward`, `null` when absent.
    pub(crate) reward: Option<MissionReward>,
    /// The `missionBuyInAmount`.
    pub(crate) buy_in_amount: Option<i64>,
    /// The `refundBuyInOnWithdraw` flag.
    pub(crate) refund_buy_in_on_withdraw: Option<bool>,
    /// The failure conditions and the deadline.
    pub(crate) finish: Finish<'a>,
    /// The `reputationRequirements` expression in stored order.
    pub(crate) standing_requirements: Vec<StandingTerm>,
    /// The `missionResultReputationRewards`, one element per stored position.
    pub(crate) reputation_results: Vec<ReputationResult>,
    /// The mission properties of `properties` in stored order: the variables the objectives and
    /// modifiers refer to.
    pub(crate) mission_properties: Vec<TemplateProperty>,
    /// The ship spawn descriptions the entry states, one per variable, ordered by variable; `level`
    /// is always `broker`.
    pub(crate) ship_spawns: Vec<ShipSpawnLink>,
    /// The location queries the entry states, resolved like `shipSpawns`.
    pub(crate) location_queries: Vec<LocationQueryLink>,
    /// The orders of the hauling objectives of the entry (`source` `brokerObjective`).
    pub(crate) hauling_orders: Vec<HaulingOrder<'a>>,
    /// The objective tokens in depth-first order, each phase after its parent.
    pub(crate) objectives: Vec<TemplateObjective<'a>>,
    /// The `partialRewardPayout`, `null` when the entry has none.
    pub(crate) partial_payout: Option<PartialPayout>,
    /// The `modifiers` in stored order.
    pub(crate) modifiers: Vec<TemplateModifier>,
    /// The `missionFlow` triggers in stored order.
    pub(crate) mission_flow: Vec<FlowTrigger>,
}
