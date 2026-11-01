// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The terms of a mission broker entry: when it is offered, how often, how long it runs and what
//! it pays. Numbers are as stored; the Archive does not state the unit of a time.

use p4kx_extraction::StructValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::{LocalisedText, every_field_required};
use crate::guid_text::CatalogGuid;
use crate::mission_layers::{key, one};
use crate::record_fields::{boolean, integer, linked, single, text};
use crate::template_issues::Unknowns;
use crate::template_objective::ObjectiveState;
use crate::texts::Texts;

/// When and to whom an entry is offered.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Availability {
    /// The `initiallyActive` flag.
    pub(crate) initially_active: Option<bool>,
    /// The `showAsOffer` flag.
    pub(crate) show_as_offer: Option<bool>,
    /// The `requestOnly` flag.
    pub(crate) request_only: Option<bool>,
    /// The `onceOnly` flag.
    pub(crate) once_only: Option<bool>,
    /// The `notifyOnAvailable` flag.
    pub(crate) notify_on_available: Option<bool>,
    /// The `availableInPrison` flag.
    pub(crate) available_in_prison: Option<bool>,
    /// The `lawfulMission` flag.
    pub(crate) lawful_mission: Option<bool>,
    /// The `tutorial` flag.
    pub(crate) tutorial: Option<bool>,
    /// The `canReacceptAfterAbandoning` flag.
    pub(crate) can_reaccept_after_abandoning: Option<bool>,
    /// The `canReacceptAfterFailing` flag.
    pub(crate) can_reaccept_after_failing: Option<bool>,
    /// The `applyCompletionTagsOnAbandoned` flag.
    pub(crate) apply_completion_tags_on_abandoned: Option<bool>,
    /// The `applyCompletionTagsOnFailed` flag.
    pub(crate) apply_completion_tags_on_failed: Option<bool>,
    /// The lowest wanted level (`reputationPrerequisites.wantedLevel.minValue`) the player may
    /// have.
    pub(crate) wanted_level_min: Option<f32>,
    /// The highest wanted level (`reputationPrerequisites.wantedLevel.maxValue`) the player may
    /// have.
    pub(crate) wanted_level_max: Option<f32>,
}

/// The cooldowns and the respawn time of an entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Cooldowns {
    /// The `hasPersonalCooldown` flag.
    pub(crate) has_personal_cooldown: Option<bool>,
    /// The `personalCooldownTime`.
    pub(crate) personal_cooldown_time: Option<f32>,
    /// The `personalCooldownTimeVariation`.
    pub(crate) personal_cooldown_time_variation: Option<f32>,
    /// The `abandonedCooldownTime`, the cooldown after the player abandons the mission.
    pub(crate) abandoned_cooldown_time: Option<f32>,
    /// The `abandonedCooldownTimeVariation`.
    pub(crate) abandoned_cooldown_time_variation: Option<f32>,
    /// The `respawnTime` of the offer.
    pub(crate) respawn_time: Option<f32>,
    /// The `respawnTimeVariation`.
    pub(crate) respawn_time_variation: Option<f32>,
}

/// How many instances of an entry run and for how long.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct InstanceLimits {
    /// The `maxInstances`.
    pub(crate) max_instances: Option<i64>,
    /// The `maxInstancesPerPlayer`.
    pub(crate) max_instances_per_player: Option<i64>,
    /// The `maxPlayersPerInstance`.
    pub(crate) max_players_per_instance: Option<i64>,
    /// The `instanceHasLifeTime` flag.
    pub(crate) has_life_time: Option<bool>,
    /// The `instanceLifeTime`.
    pub(crate) life_time: Option<f32>,
    /// The `instanceLifeTimeVariation`.
    pub(crate) life_time_variation: Option<f32>,
}

/// The `missionReward` of an entry, as stored.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionReward {
    /// The `currencyType` as stored, e.g. `UEC`.
    pub(crate) currency: Option<String>,
    /// The `reward`.
    pub(crate) reward: Option<i64>,
    /// The `max`.
    pub(crate) max: Option<i64>,
    /// The `plusBonuses` flag.
    pub(crate) plus_bonuses: Option<bool>,
    /// The `reputationBonus` Record.
    pub(crate) reputation_bonus_guid: Option<CatalogGuid>,
}

/// The failure conditions and the deadline of an entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Finish<'a> {
    /// The `failIfSentToPrison` flag.
    pub(crate) fail_if_sent_to_prison: Option<bool>,
    /// The `failIfBecameCriminal` flag.
    pub(crate) fail_if_became_criminal: Option<bool>,
    /// The `failIfLeavePrison` flag.
    pub(crate) fail_if_leave_prison: Option<bool>,
    /// The `missionAutoEnd` flag of the deadline.
    pub(crate) auto_end: Option<bool>,
    /// The `missionCompletionTime` of the deadline.
    pub(crate) completion_time: Option<f32>,
    /// The `missionEndReason` of the deadline.
    #[schemars(extend("x-gameText" = true))]
    pub(crate) end_reason: LocalisedText<'a>,
    /// The `missionResultAfterTimerEnd` of the deadline: the state the mission ends in.
    pub(crate) result_after_timer_end: Option<ObjectiveState>,
    /// The `remainingTimeToShowTimer` of the deadline.
    pub(crate) remaining_time_to_show_timer: Option<f32>,
}

/// Reads the availability flags and the wanted level range of `root`.
pub(crate) fn availability(root: Option<&StructValue<'_>>) -> Availability {
    let flag = |name: &str| boolean(root, name);
    let wanted = one(one(root, "reputationPrerequisites"), "wantedLevel");
    Availability {
        initially_active: flag("initiallyActive"),
        show_as_offer: flag("showAsOffer"),
        request_only: flag("requestOnly"),
        once_only: flag("onceOnly"),
        notify_on_available: flag("notifyOnAvailable"),
        available_in_prison: flag("availableInPrison"),
        lawful_mission: flag("lawfulMission"),
        tutorial: flag("tutorial"),
        can_reaccept_after_abandoning: flag("canReacceptAfterAbandoning"),
        can_reaccept_after_failing: flag("canReacceptAfterFailing"),
        apply_completion_tags_on_abandoned: flag("applyCompletionTagsOnAbandoned"),
        apply_completion_tags_on_failed: flag("applyCompletionTagsOnFailed"),
        wanted_level_min: single(wanted, "minValue"),
        wanted_level_max: single(wanted, "maxValue"),
    }
}

/// Reads the cooldowns and the respawn time of `root`.
pub(crate) fn cooldowns(root: Option<&StructValue<'_>>) -> Cooldowns {
    Cooldowns {
        has_personal_cooldown: boolean(root, "hasPersonalCooldown"),
        personal_cooldown_time: single(root, "personalCooldownTime"),
        personal_cooldown_time_variation: single(root, "personalCooldownTimeVariation"),
        abandoned_cooldown_time: single(root, "abandonedCooldownTime"),
        abandoned_cooldown_time_variation: single(root, "abandonedCooldownTimeVariation"),
        respawn_time: single(root, "respawnTime"),
        respawn_time_variation: single(root, "respawnTimeVariation"),
    }
}

/// Reads the instance limits and life time of `root`.
pub(crate) fn instances(root: Option<&StructValue<'_>>) -> InstanceLimits {
    InstanceLimits {
        max_instances: integer(root, "maxInstances"),
        max_instances_per_player: integer(root, "maxInstancesPerPlayer"),
        max_players_per_instance: integer(root, "maxPlayersPerInstance"),
        has_life_time: boolean(root, "instanceHasLifeTime"),
        life_time: single(root, "instanceLifeTime"),
        life_time_variation: single(root, "instanceLifeTimeVariation"),
    }
}

/// Reads the `missionReward` of `root`, `None` when it has none.
pub(crate) fn reward(root: Option<&StructValue<'_>>) -> Option<MissionReward> {
    let reward = one(root, "missionReward")?;
    let reward = Some(reward);
    Some(MissionReward {
        currency: text(reward, "currencyType").map(str::to_owned),
        reward: integer(reward, "reward"),
        max: integer(reward, "max"),
        plus_bonuses: boolean(reward, "plusBonuses"),
        reputation_bonus_guid: linked(reward, "reputationBonus"),
    })
}

/// Reads the failure conditions and the deadline of `root`.
pub(crate) fn finish<'f>(
    texts: &Texts<'f>,
    unknowns: &Unknowns,
    root: Option<&StructValue<'f>>,
) -> Finish<'f> {
    let deadline = one(root, "missionDeadline");
    Finish {
        fail_if_sent_to_prison: boolean(root, "failIfSentToPrison"),
        fail_if_became_criminal: boolean(root, "failIfBecameCriminal"),
        fail_if_leave_prison: boolean(root, "failIfLeavePrison"),
        auto_end: boolean(deadline, "missionAutoEnd"),
        completion_time: single(deadline, "missionCompletionTime"),
        end_reason: texts.localised(key(deadline, "missionEndReason")),
        result_after_timer_end: unknowns.parse(
            "timer end result",
            text(deadline, "missionResultAfterTimerEnd"),
            ObjectiveState::parse,
        ),
        remaining_time_to_show_timer: single(deadline, "remainingTimeToShowTimer"),
    }
}
