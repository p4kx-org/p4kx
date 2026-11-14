// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::guid_text::CatalogGuid;

/// A mission type: a `MissionType` Record under `missiontype`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionType<'a> {
    /// The GUID of the Record, the target of `contracts[].missionTypeGuid`.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `missiontype.job`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The localised name of the type (`LocalisedTypeName`).
    #[schemars(extend("x-gameText" = true))]
    pub(crate) type_name: LocalisedText<'a>,
}

/// A mission locality: a `MissionLocality` Record under `missiondata/pu_missionlocality`, a named
/// set of starmap objects. The game does not localise its name.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionLocality<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `pyro`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The `availableLocations`, the `guid`s of entries of `starmapObjects`, in the stored order.
    /// A Reference to no Record is left out; a GUID missing from `starmapObjects` is kept.
    pub(crate) starmap_object_guids: Vec<CatalogGuid>,
}

/// A contract scenario: a `ScenarioProgress` Record under `contracts/contractscenarios`, the
/// rewards a community progression hands out.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ContractScenario<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type, e.g. `ORS_ScenarioProgress`.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The reward tiers per faction (`factionRewardTiers`), in the stored order.
    pub(crate) faction_reward_tiers: Vec<ScenarioFactionTiers<'a>>,
}

/// The reward tiers of one faction in a scenario.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ScenarioFactionTiers<'a> {
    /// The authored `Faction` GUID, including dangling or filtered targets of `definitions.factions`.
    pub(crate) faction_guid: Option<CatalogGuid>,
    /// The `factionReputationRef` of the `Faction` Record, an entry of `factionReputations`;
    /// `null` when the Record is missing, unreadable or names none.
    pub(crate) faction_reputation_guid: Option<CatalogGuid>,
    /// The Record name of the faction, the last part of its record path, e.g.
    /// `faction_lawful_crusaderindustries`; `null` for no Reference.
    pub(crate) faction_name: Option<&'a str>,
    /// Whether the tier points count as percent (`convertTiersPointsToPercent`).
    pub(crate) points_as_percent: Option<bool>,
    /// The progressions (`tierProgressions`), in the stored order.
    pub(crate) progressions: Vec<ScenarioProgression<'a>>,
}

/// One progression of a scenario: a ladder of tiers.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ScenarioProgression<'a> {
    /// The `minMultiplierThreshold`.
    pub(crate) min_multiplier_threshold: Option<f32>,
    /// The struct name of the `completionType`, e.g. `CompletionType_GlobalProgression`.
    pub(crate) completion_type: Option<&'a str>,
    /// The `periodMissionScenario` Record, `null` when it names none.
    pub(crate) period_scenario_guid: Option<CatalogGuid>,
    /// The tiers (`tierRewards`), in the stored order.
    pub(crate) tiers: Vec<ScenarioTier<'a>>,
}

/// A reward tier of a scenario progression.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ScenarioTier<'a> {
    /// The points from which the tier is reached (`minPoints`).
    pub(crate) min_points: Option<i64>,
    /// The identifier of the badge the tier awards (`badgeToAward`), `null` for none; the game
    /// stores the text `None` for a tier without a badge.
    pub(crate) badge_to_award: Option<&'a str>,
    /// The `globalPointsMultiplier`.
    pub(crate) global_points_multiplier: Option<f32>,
    /// The blueprint pools the tier rewards (`blueprintPool`), the `guid`s of entries of
    /// `blueprintPools`, in the stored order.
    pub(crate) blueprint_pool_guids: Vec<CatalogGuid>,
}
