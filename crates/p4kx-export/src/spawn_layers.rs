// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeMap;

use p4kx_extraction::{StructValue, TagTree};

use crate::blueprint::as_struct;
use crate::mission_layers::{items, one};
use crate::record_fields::text;
use crate::spawn_model::{LocationPrerequisite, LocationQueryLink, SpawnLevel, SpawnPoolLink};
use crate::spawn_pools::{Registered, SpawnPools, Target};
use crate::spawn_read::Reader;

const LOCATION_PROPERTY: &str = "ContractPrerequisite_LocationProperty";

#[derive(Debug)]
struct Stated {
    variable: String,
    token: Option<String>,
    registered: Registered,
}

/// The properties and location prerequisites one level of a contract states.
#[derive(Debug)]
pub(crate) struct Level {
    at: SpawnLevel,
    stated: Vec<Stated>,
    prerequisites: Vec<LocationPrerequisite>,
}

impl Level {
    /// Returns what this level alone states.
    pub(crate) fn resolved(&self) -> Resolved {
        resolve(&[self])
    }

    /// Reads the properties in the array `properties` of `params` and the location prerequisites
    /// in the array `prerequisites` of `owner`, registering what they state in `pools`.
    ///
    /// # Errors
    ///
    /// Returns the error of the serialisation that makes the identifier of a description.
    pub(crate) fn read<'f>(
        level: SpawnLevel,
        reader: &Reader<'_, 'f>,
        (params, properties): (Option<&StructValue<'f>>, &str),
        (owner, prerequisites): (Option<&StructValue<'_>>, &str),
        pools: &mut SpawnPools<'f>,
    ) -> Result<Self, serde_json::Error> {
        let mut found = Vec::new();
        for property in items(params, properties).iter().filter_map(as_struct) {
            found.extend(stated(reader, property, pools)?);
        }
        Ok(Self {
            at: level,
            stated: found,
            prerequisites: location_prerequisites(level, owner, prerequisites),
        })
    }
}

fn stated<'f>(
    reader: &Reader<'_, 'f>,
    property: &StructValue<'f>,
    pools: &mut SpawnPools<'f>,
) -> Result<Option<Stated>, serde_json::Error> {
    let Some(variable) = text(Some(property), "missionVariableName") else {
        return Ok(None);
    };
    let token = text(Some(property), "extendedTextToken").map(str::to_owned);
    let Some(value) = one(Some(property), "value") else {
        return Ok(None);
    };
    Ok(reader.spawn(value)?.map(|spawn| Stated {
        variable: variable.to_owned(),
        token,
        registered: pools.register(spawn),
    }))
}

fn location_prerequisites(
    level: SpawnLevel,
    owner: Option<&StructValue<'_>>,
    name: &str,
) -> Vec<LocationPrerequisite> {
    items(owner, name)
        .iter()
        .filter_map(as_struct)
        .filter(|prerequisite| prerequisite.type_name() == LOCATION_PROPERTY)
        .filter_map(|prerequisite| {
            Some(LocationPrerequisite {
                level,
                variable: text(Some(prerequisite), "propertyVariableName")?.to_owned(),
                token: text(Some(prerequisite), "propertyExtendedTextToken").map(str::to_owned),
                location_level_type: text(Some(prerequisite), "locationLevelType")
                    .map(str::to_owned),
            })
        })
        .collect()
}

/// The spawn descriptions and location queries of the levels of one Record, read through its tags
/// and weak pointers; what they state is collected in [`pools`](Self::into_pools).
pub(crate) struct RecordSpawns<'x, 'f> {
    reader: Reader<'x, 'f>,
    pools: RefCell<SpawnPools<'f>>,
}

impl<'x, 'f> RecordSpawns<'x, 'f> {
    /// Creates the reader of the Record whose root struct is `root`.
    pub(crate) fn new(tags: &'x TagTree, root: &'x StructValue<'f>) -> Self {
        Self {
            reader: Reader::new(tags, root),
            pools: RefCell::new(SpawnPools::default()),
        }
    }

    /// Returns the distinct descriptions and queries the levels stated.
    pub(crate) fn into_pools(self) -> SpawnPools<'f> {
        self.pools.into_inner()
    }

    fn level(
        &self,
        level: SpawnLevel,
        params: (Option<&StructValue<'f>>, &str),
        owner: (Option<&StructValue<'f>>, &str),
    ) -> Result<Level, serde_json::Error> {
        Level::read(
            level,
            &self.reader,
            params,
            owner,
            &mut self.pools.borrow_mut(),
        )
    }

    /// Reads the level of `handler`.
    ///
    /// # Errors
    ///
    /// Returns the error of the serialisation that makes the identifier of a description.
    pub(crate) fn handler(
        &self,
        handler: &StructValue<'f>,
    ) -> Result<HandlerSpawns<'_, 'x, 'f>, serde_json::Error> {
        let level = self.level(
            SpawnLevel::Handler,
            (one(Some(handler), "contractParams"), "propertyOverrides"),
            (one(Some(handler), "defaultAvailability"), "prerequisites"),
        )?;
        Ok(HandlerSpawns {
            record: self,
            level,
        })
    }
}

/// The level of a handler, the base of the contracts it holds.
pub(crate) struct HandlerSpawns<'r, 'x, 'f> {
    record: &'r RecordSpawns<'x, 'f>,
    level: Level,
}

impl<'r, 'x, 'f> HandlerSpawns<'r, 'x, 'f> {
    /// Reads the level of `contract`; `template` is the level of its template.
    ///
    /// # Errors
    ///
    /// Returns the error of the serialisation that makes the identifier of a description.
    pub(crate) fn contract<'h>(
        &'h self,
        contract: &StructValue<'f>,
        template: Option<&'h Level>,
    ) -> Result<ContractSpawns<'h, 'r, 'x, 'f>, serde_json::Error> {
        let level = self.record.level(
            SpawnLevel::Contract,
            (one(Some(contract), "paramOverrides"), "propertyOverrides"),
            (Some(contract), "additionalPrerequisites"),
        )?;
        Ok(ContractSpawns {
            handler: self,
            contract: level,
            template,
        })
    }
}

/// The levels of one contract that its entries resolve their ships and locations from.
pub(crate) struct ContractSpawns<'h, 'r, 'x, 'f> {
    handler: &'h HandlerSpawns<'r, 'x, 'f>,
    contract: Level,
    template: Option<&'h Level>,
}

impl<'f> ContractSpawns<'_, '_, '_, 'f> {
    /// Returns what the contract, or its sub contract `sub`, states.
    ///
    /// # Errors
    ///
    /// Returns the error of the serialisation that makes the identifier of a description.
    pub(crate) fn resolve(
        &self,
        sub: Option<&StructValue<'f>>,
    ) -> Result<Resolved, serde_json::Error> {
        let sub_level = sub
            .map(|sub| {
                self.handler.record.level(
                    SpawnLevel::SubContract,
                    (
                        one(Some(sub), "paramOverrides").or(Some(sub)),
                        "propertyOverrides",
                    ),
                    (Some(sub), "additionalPrerequisites"),
                )
            })
            .transpose()?;
        let levels: Vec<&Level> = sub_level
            .iter()
            .chain([&self.contract, &self.handler.level])
            .chain(self.template)
            .collect();
        Ok(resolve(&levels))
    }
}

/// What an entry states about spawns and locations.
#[derive(Debug, Default)]
pub(crate) struct Resolved {
    pub(crate) ship_spawns: Vec<SpawnPoolLink>,
    pub(crate) npc_spawns: Vec<SpawnPoolLink>,
    pub(crate) entity_spawns: Vec<SpawnPoolLink>,
    pub(crate) location_queries: Vec<LocationQueryLink>,
    pub(crate) location_prerequisites: Vec<LocationPrerequisite>,
}

/// Resolves the properties of `levels`, nearest first (sub contract, contract, handler,
/// template): the first level that states a variable wins. The links are ordered by variable.
/// The prerequisites are those of the handler, the contract and the sub contract, in that order.
fn resolve(levels: &[&Level]) -> Resolved {
    let mut won: BTreeMap<&str, (&Level, &Stated)> = BTreeMap::new();
    for level in levels {
        for stated in &level.stated {
            won.entry(stated.variable.as_str())
                .or_insert((level, stated));
        }
    }
    let mut resolved = Resolved::default();
    for (variable, (level, stated)) in won {
        let Registered { target, empty } = stated.registered;
        if empty {
            continue;
        }
        let (variable, token, at) = (variable.to_owned(), stated.token.clone(), level.at);
        let (links, pool_guid) = match target {
            Target::Ships(guid) => (&mut resolved.ship_spawns, guid),
            Target::Npcs(guid) => (&mut resolved.npc_spawns, guid),
            Target::Entities(guid) => (&mut resolved.entity_spawns, guid),
            Target::Location(query_guid) => {
                resolved.location_queries.push(LocationQueryLink {
                    variable,
                    token,
                    level: at,
                    query_guid,
                });
                continue;
            }
        };
        links.push(SpawnPoolLink {
            variable,
            token,
            level: at,
            pool_guid,
        });
    }
    resolved.location_prerequisites = levels
        .iter()
        .rev()
        .flat_map(|level| level.prerequisites.iter().cloned())
        .collect();
    resolved
}
