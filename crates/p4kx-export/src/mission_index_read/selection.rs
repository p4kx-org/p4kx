// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeError, Record};

use crate::Kind;

const TEMPLATES: &str = "libs/foundry/records/contracts/contracttemplates/";
const TYPES: &str = "libs/foundry/records/missiontype/";
const ORGANIZATIONS: &str = "libs/foundry/records/missiondata/pu_organizations/";
const REWARDS: &str = "libs/foundry/records/reputation/rewards/missionrewards_reputation/";
const LOCALITIES: &str = "libs/foundry/records/missiondata/pu_missionlocality/";
const BROKERS: &str = "libs/foundry/records/missionbroker/";
const FACTIONS: &str = "libs/foundry/records/factions/factionreputation/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    Template,
    Type,
    Organization,
    Reward,
    Locality,
    Broker,
    Faction,
}

impl Action {
    pub(super) const fn kinds(self) -> &'static [Kind] {
        match self {
            Self::Template => &[
                Kind::Contracts,
                Kind::ContractShipSpawns,
                Kind::ContractLocationQueries,
                Kind::ContractNpcSpawns,
                Kind::ContractEntitySpawns,
            ],
            _ => &[Kind::Contracts],
        }
    }
}

pub(super) fn choose(record: &Record<'_>) -> Result<Option<Action>, DataForgeError> {
    choose_at_path(record, record.record_path()?)
}

pub(super) fn choose_at_path(
    record: &Record<'_>,
    path: &str,
) -> Result<Option<Action>, DataForgeError> {
    for (prefix, action) in [
        (TEMPLATES, Action::Template),
        (TYPES, Action::Type),
        (ORGANIZATIONS, Action::Organization),
        (REWARDS, Action::Reward),
        (LOCALITIES, Action::Locality),
        (BROKERS, Action::Broker),
        (FACTIONS, Action::Faction),
    ] {
        if path
            .get(..prefix.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
        {
            return Ok(Some(action));
        }
    }
    Ok((record.type_name()? == "ContractTemplate").then_some(Action::Template))
}
