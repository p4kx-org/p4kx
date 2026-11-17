// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::catalog_projection::CatalogProjection;
use crate::contract_missions::{ContractFaction, ReputationGain};
use crate::contract_offer::{GiverFaction, GiverSource};
use crate::guid_text::CatalogGuid;
use crate::mission_index::MissionIndex;
use crate::record_read::ReadError;

/// A faction as a Reference states it: its GUID and the Record name from the path.
pub(crate) type FactionLink<'f> = (CatalogGuid, &'f str);

/// Projects a retained faction link with the available original display-name cell.
pub(crate) fn faction<'f>(
    (guid, name): FactionLink<'f>,
    index: &MissionIndex<'f>,
    projection: &CatalogProjection<'_, 'f>,
) -> Result<ContractFaction<'f>, ReadError> {
    let display_name = index
        .lookup_faction_name(guid.0)?
        .map(|key| projection.finish_protected_key(key.clone()))
        .transpose()?;
    Ok(ContractFaction {
        guid,
        name,
        display_name,
    })
}

/// Returns the faction of a contract and where it was found: the `handler` faction, else the
/// first of `gains` with a positive amount, else `prerequisite`, the faction of the first
/// reputation prerequisite that is no exclusion. A negative amount names a rival that loses
/// standing, not the giver.
pub(crate) fn giver<'f>(
    handler: Option<&ContractFaction<'f>>,
    gains: &[ReputationGain<'f>],
    prerequisite: Option<FactionLink<'f>>,
    index: &MissionIndex<'f>,
    projection: &CatalogProjection<'_, 'f>,
) -> Result<Option<GiverFaction<'f>>, ReadError> {
    if let Some(faction) = handler {
        return Ok(Some(GiverFaction {
            source: GiverSource::Handler,
            faction: faction.clone(),
        }));
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
    let selected = from_result
        .map(|link| (GiverSource::ReputationResult, link))
        .or_else(|| prerequisite.map(|link| (GiverSource::ReputationPrerequisite, link)));
    selected
        .map(|(source, link)| {
            Ok(GiverFaction {
                source,
                faction: faction(link, index, projection)?,
            })
        })
        .transpose()
}
