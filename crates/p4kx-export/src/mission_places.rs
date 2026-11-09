// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{Guid, StructValue};

use crate::contract::LocalisedText;
use crate::contract_missions::PickupPlace;
use crate::contract_offer::OfferedAt;
use crate::contract_starmap::StarmapObject;
use crate::guid_text::CatalogGuid;
use crate::mission_index::MissionIndex;
use crate::mission_layers::items;
use crate::record_fields::linked;

const LOCALITY: &str = "ContractPrerequisite_Locality";
const LOCATION: &str = "ContractPrerequisite_Location";
const SYSTEM: &str = "SolarSystem";
const STAR: &str = "Star";

/// The places a level of a contract names in its prerequisites: the `MissionLocality` Records of
/// `ContractPrerequisite_Locality` and the starmap objects of `ContractPrerequisite_Location`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Where {
    pub(crate) localities: Vec<Guid>,
    pub(crate) locations: Vec<Guid>,
}

impl Where {
    /// Reads the prerequisites in the array field `name` of `parent`.
    pub(crate) fn read(parent: Option<&StructValue<'_>>, name: &str) -> Self {
        let mut found = Self::default();
        for prerequisite in items(parent, name)
            .iter()
            .filter_map(crate::blueprint::as_struct)
        {
            match prerequisite.type_name() {
                LOCALITY => found
                    .localities
                    .extend(linked(Some(prerequisite), "localityAvailable").map(|guid| guid.0)),
                LOCATION => found
                    .locations
                    .extend(linked(Some(prerequisite), "locationAvailable").map(|guid| guid.0)),
                _ => {}
            }
        }
        found
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.localities.is_empty() && self.locations.is_empty()
    }
}

/// The starmap objects by GUID, to turn the places of a contract into names.
pub(crate) struct Places<'s, 'f> {
    objects: HashMap<Guid, &'s StarmapObject<'f>>,
}

impl<'s, 'f> Places<'s, 'f> {
    pub(crate) fn new(objects: &'s [StarmapObject<'f>]) -> Self {
        Self {
            objects: objects
                .iter()
                .map(|object| (object.guid.0, object))
                .collect(),
        }
    }

    /// Returns the pickup places of the levels `levels`, nearest first: the starmap objects of
    /// their localities and locations, a star left out, the system first, then planets and moons,
    /// then the rest, each group in stored order and none twice.
    pub(crate) fn pickup(
        &self,
        index: &MissionIndex<'f>,
        levels: &[&Where],
    ) -> Vec<PickupPlace<'f>> {
        let listed = unique(levels.iter().flat_map(|level| {
            level
                .localities
                .iter()
                .flat_map(|locality| index.locality(*locality).iter().copied())
                .chain(level.locations.iter().copied())
        }));
        let mut ranked: Vec<(u8, Guid, &StarmapObject<'f>)> = Vec::new();
        for guid in listed {
            let Some(object) = self.objects.get(&guid).copied() else {
                continue;
            };
            let kind = object.r#type;
            if kind.is_some_and(|kind| kind.eq_ignore_ascii_case(STAR)) {
                continue;
            }
            let rank = match kind {
                Some(kind) if kind.eq_ignore_ascii_case(SYSTEM) => 0,
                Some(kind)
                    if kind.eq_ignore_ascii_case("Planet") || kind.eq_ignore_ascii_case("Moon") =>
                {
                    1
                }
                _ => 2,
            };
            ranked.push((rank, guid, object));
        }
        ranked.sort_by_key(|(rank, _, _)| *rank);
        ranked
            .into_iter()
            .map(|(_, guid, object)| describe(guid, Some(object)))
            .collect()
    }

    /// Returns the pickup places and everywhere the contract is offered. A contract left without
    /// a pickup place takes `broker`, the location of its broker entry; `offeredAt` takes it only
    /// when the prerequisites name no place at all.
    pub(crate) fn locate(
        &self,
        index: &MissionIndex<'f>,
        prerequisites: &Prerequisites<'_>,
        broker: Option<Guid>,
    ) -> (Vec<PickupPlace<'f>>, OfferedAt<'f>) {
        let levels = prerequisites.levels();
        let mut pickup = self.pickup(index, &levels);
        if pickup.is_empty()
            && let Some(location) = broker
        {
            let at_broker = Where {
                localities: Vec::new(),
                locations: vec![location],
            };
            pickup = self.pickup(index, &[&at_broker]);
        }
        (pickup, self.offered(&levels, broker))
    }

    /// Returns every place the levels `levels` name, uncapped: their localities and their
    /// location objects, each in stored order and none twice. When they name no place at all the
    /// location `fallback` is the only one and `broker_fallback` is set.
    fn offered(&self, levels: &[&Where], fallback: Option<Guid>) -> OfferedAt<'f> {
        let localities = unique(
            levels
                .iter()
                .flat_map(|level| level.localities.iter().copied()),
        );
        let mut locations = unique(
            levels
                .iter()
                .flat_map(|level| level.locations.iter().copied()),
        );
        let broker_fallback = localities.is_empty() && locations.is_empty() && fallback.is_some();
        if broker_fallback {
            locations.extend(fallback);
        }
        OfferedAt {
            locality_guids: localities.into_iter().map(CatalogGuid).collect(),
            locations: locations
                .into_iter()
                .map(|guid| describe(guid, self.objects.get(&guid).copied()))
                .collect(),
            broker_fallback,
        }
    }
}

/// The prerequisites a contract's places are read from: those of its handler and contract, or the
/// sub contract's own when it names any.
pub(crate) struct Prerequisites<'w> {
    pub(crate) own: Option<&'w Where>,
    pub(crate) inherited: [&'w Where; 2],
}

impl<'w> Prerequisites<'w> {
    fn levels(&self) -> Vec<&'w Where> {
        self.own
            .filter(|own| !own.is_empty())
            .map_or_else(|| self.inherited.to_vec(), |own| vec![own])
    }
}

/// Returns the GUIDs of `listed` in order, none twice.
fn unique(listed: impl Iterator<Item = Guid>) -> Vec<Guid> {
    let mut seen: HashSet<Guid> = HashSet::new();
    listed.filter(|guid| seen.insert(*guid)).collect()
}

/// Describes the starmap object `guid`; without the object only its GUID is known.
fn describe<'f>(guid: Guid, object: Option<&StarmapObject<'f>>) -> PickupPlace<'f> {
    PickupPlace {
        starmap_object_guid: CatalogGuid(guid),
        object_type: object.and_then(|object| object.r#type),
        place: object.map_or_else(LocalisedText::default, |object| LocalisedText {
            key: object.name.key,
            en: object.name.en.clone(),
            de: object.name.de.clone(),
        }),
    }
}
