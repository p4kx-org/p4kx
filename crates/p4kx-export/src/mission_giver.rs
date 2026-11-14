// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::contract_missions::{ContractFaction, ReputationGain};
use crate::contract_offer::{GiverFaction, GiverSource};
use crate::guid_text::CatalogGuid;
use crate::mission_index::MissionIndex;
use crate::texts::Texts;

/// A faction as a Reference states it: its GUID and the Record name from the path.
pub(crate) type FactionLink<'f> = (CatalogGuid, &'f str);

/// Returns the faction of a contract and where it was found: the `handler` faction, else the
/// first of `gains` with a positive amount, else `prerequisite`, the faction of the first
/// reputation prerequisite that is no exclusion. A negative amount names a rival that loses
/// standing, not the giver.
pub(crate) fn giver<'f>(
    handler: Option<&ContractFaction<'f>>,
    gains: &[ReputationGain<'f>],
    prerequisite: Option<FactionLink<'f>>,
    (index, texts): (&MissionIndex<'f>, &Texts<'f>),
) -> Option<GiverFaction<'f>> {
    let named = |(guid, name): FactionLink<'f>| ContractFaction {
        guid,
        name,
        display_name: texts.localised(index.faction_name(guid.0)),
    };
    if let Some(faction) = handler {
        return Some(GiverFaction {
            source: GiverSource::Handler,
            faction: faction.clone(),
        });
    }
    let from_result = gains
        .iter()
        .filter(|gain| gain.amount.value().is_some_and(|amount| amount.get() > 0))
        .find_map(|gain| {
            Some((
                *gain.faction_reputation_guid.value()?,
                *gain.faction_reputation_name.value()?,
            ))
        });
    if let Some(link) = from_result {
        return Some(GiverFaction {
            source: GiverSource::ReputationResult,
            faction: named(link),
        });
    }
    prerequisite.map(|link| GiverFaction {
        source: GiverSource::ReputationPrerequisite,
        faction: named(link),
    })
}
