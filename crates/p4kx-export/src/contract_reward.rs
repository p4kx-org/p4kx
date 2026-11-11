// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::resource_integer::ResourceInt64;

#[path = "contract_reward_read.rs"]
mod read;

pub(crate) use self::read::fixed_rewards;

/// A stored payout selected by the contract result's first mission outcome.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct FixedReward<'source> {
    /// The original currency type, when stated.
    pub(crate) currency: SourcedValue<&'source str>,
    /// The original reward amount.
    pub(crate) amount: SourcedValue<ResourceInt64>,
    /// The original maximum amount, with zero for a fixed payout.
    pub(crate) max: SourcedValue<ResourceInt64>,
    /// Whether the reward specifies bonuses on top.
    pub(crate) plus_bonuses: SourcedValue<bool>,
}
