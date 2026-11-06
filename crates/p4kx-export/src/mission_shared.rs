// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::contract_facts::ContractFacts;
use crate::contract_missions::{ContractFaction, PickupPlace, ReputationGain};
use crate::contract_offer::{BlueprintReward, GiverFaction, OfferedAt};
use crate::contract_reward::{FixedReward, fixed_rewards};
use crate::guid_text::CatalogGuid;
use crate::mission_blueprints::{Overrides, blueprint_rewards};
use crate::mission_cooldown::CooldownDefaults;
use crate::mission_facts::SharedFacts;
use crate::mission_giver::giver;
use crate::mission_index::MissionIndex;
use crate::mission_layers::one;
use crate::mission_results::{entry_results, gains};
use crate::mission_standing::{Standing, required_faction};
use crate::spawn_layers::Resolved;
use crate::texts::Texts;

/// What an entry is resolved to besides its layers: its places, facts and spawn descriptions.
pub(crate) struct Found<'f> {
    pub(crate) pickup: Vec<PickupPlace<'f>>,
    pub(crate) offered_at: OfferedAt<'f>,
    pub(crate) facts: ContractFacts,
    pub(crate) spawns: Resolved,
    pub(crate) property_set_guids: Vec<CatalogGuid>,
}

/// What the entries of one contract share: the standing it requires, the cooldown defaults of its
/// handler, its facts and its fixed rewards.
pub(crate) struct Shared<'f> {
    pub(crate) standing: Standing,
    pub(crate) cooldown: CooldownDefaults,
    pub(crate) facts: SharedFacts,
    pub(crate) fixed_rewards: Vec<FixedReward<'f>>,
}

impl<'f> Shared<'f> {
    /// Reads what the entries of `contract` of `handler` share.
    pub(crate) fn read(handler: &StructValue<'f>, contract: &StructValue<'f>) -> Self {
        Self {
            standing: Standing::read(handler, contract),
            cooldown: CooldownDefaults::read(handler),
            facts: SharedFacts::read(handler, contract),
            fixed_rewards: fixed_rewards(contract),
        }
    }
}

/// What an entry's own resolved results and overrides grant: reputation, the giver faction that
/// follows from it and the blueprint pools.
pub(crate) struct Rewards<'f> {
    pub(crate) gains: Vec<ReputationGain<'f>>,
    pub(crate) giver_faction: Option<GiverFaction<'f>>,
    pub(crate) blueprints: Vec<BlueprintReward>,
}

impl<'f> Rewards<'f> {
    /// Reads the rewards of the entry for `sub` (or the contract itself) of `contract`, whose
    /// handler states `handler_faction`.
    pub(crate) fn read(
        (handler, contract, sub): (&StructValue<'f>, &StructValue<'f>, Option<&StructValue<'f>>),
        handler_faction: Option<&ContractFaction<'f>>,
        (index, texts): (&MissionIndex<'f>, &Texts<'f>),
    ) -> Self {
        let block = entry_results(contract, sub);
        let gains = gains(block, index);
        let giver_faction = giver(
            handler_faction,
            &gains,
            required_faction(handler, contract, sub),
            (index, texts),
        );
        let overrides = Overrides {
            sub: sub.and_then(|sub| one(Some(sub), "paramOverrides").or(Some(sub))),
            contract: one(Some(contract), "paramOverrides"),
            handler: one(Some(handler), "contractParams"),
        };
        Self {
            gains,
            giver_faction,
            blueprints: blueprint_rewards(block, &overrides),
        }
    }
}
