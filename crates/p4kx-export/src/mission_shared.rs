// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, StructValue};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalFlowFields;
use crate::contract_facts::ContractFacts;
use crate::contract_missions::{ContractFaction, PickupPlace, ReputationGain};
use crate::contract_offer::{BlueprintReward, GiverFaction, OfferedAt};
use crate::contract_reward::{FixedReward, fixed_rewards};
use crate::guid_text::CatalogGuid;
use crate::mission_blueprints::{Overrides, blueprint_rewards};
use crate::mission_cooldown::ContractCooldown;
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
    pub(crate) cooldown: ContractCooldown,
    pub(crate) spawns: Resolved,
    pub(crate) property_set_guids: Vec<CatalogGuid>,
}

/// The standing, facts and fixed rewards shared by a contract and its sub contracts.
pub(crate) struct Shared<'f> {
    pub(crate) standing: Standing,
    pub(crate) facts: SharedFacts,
    pub(crate) fixed_rewards: SourcedValue<Vec<FixedReward<'f>>>,
}

impl<'f> Shared<'f> {
    /// Reads what the entries of `contract` of `handler` share.
    pub(crate) fn read(
        fields: &OriginalFlowFields<'_, '_, 'f>,
        handler: &StructValue<'f>,
        contract: &StructValue<'f>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            standing: Standing::read(handler, contract),
            facts: SharedFacts::read(fields, handler, contract)?,
            fixed_rewards: fixed_rewards(fields, contract)?,
        })
    }
}

/// The giver faction and blueprint pools resolved from the entry's results and overrides.
pub(crate) struct Rewards<'f> {
    pub(crate) gains: SourcedValue<Vec<ReputationGain<'f>>>,
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
        (fields, input): (
            &OriginalFlowFields<'_, '_, 'f>,
            &OriginalCatalogInput<'_, 'f>,
        ),
    ) -> Result<Self, CatalogError> {
        let block = entry_results(contract, sub);
        let gains = gains(fields, input, index, contract, sub)?;
        let giver_faction = giver(
            handler_faction,
            gains.value().map_or(&[], Vec::as_slice),
            required_faction(handler, contract, sub),
            (index, texts),
        );
        let overrides = Overrides {
            sub: sub.and_then(|sub| one(Some(sub), "paramOverrides").or(Some(sub))),
            contract: one(Some(contract), "paramOverrides"),
            handler: one(Some(handler), "contractParams"),
        };
        Ok(Self {
            gains,
            giver_faction,
            blueprints: blueprint_rewards(block, &overrides),
        })
    }
}
