// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::record_fields::{boolean, integer, linked, text};

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

/// Reads a reward definition struct (the `missionReward` of an entry, the `rewardDef` of a
/// property value).
pub(crate) fn mission_reward(reward: &StructValue<'_>) -> MissionReward {
    let reward = Some(reward);
    MissionReward {
        currency: text(reward, "currencyType").map(str::to_owned),
        reward: integer(reward, "reward"),
        max: integer(reward, "max"),
        plus_bonuses: boolean(reward, "plusBonuses"),
        reputation_bonus_guid: linked(reward, "reputationBonus"),
    }
}
