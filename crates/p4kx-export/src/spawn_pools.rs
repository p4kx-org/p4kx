// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::CancelToken;

use crate::catalog_projection::CatalogProjection;
use crate::entity_spawn_model::EntitySpawnPool;
use crate::guid_text::CatalogGuid;
use crate::npc_spawn_model::{DraftNpcPool, PublishedNpcPool};
use crate::spawn_model::{LocationQuery, ShipSpawnPool};
use crate::spawn_read::Spawn;
use crate::{CatalogError, Kind};

/// The distinct ship, NPC and entity spawn descriptions and location queries, by identifier.
#[derive(Debug, Default)]
pub(crate) struct SpawnPools<'f> {
    pub(crate) ships: BTreeMap<[u8; 36], ShipSpawnPool>,
    pub(crate) npcs: BTreeMap<[u8; 36], DraftNpcPool<'f>>,
    pub(crate) entities: BTreeMap<[u8; 36], EntitySpawnPool>,
    pub(crate) locations: BTreeMap<[u8; 36], LocationQuery>,
}

pub(crate) struct PublishedSpawnPools {
    pub(crate) ships: Vec<ShipSpawnPool>,
    pub(crate) npcs: Vec<PublishedNpcPool>,
    pub(crate) entities: Vec<EntitySpawnPool>,
    pub(crate) locations: Vec<LocationQuery>,
}

/// The entry a registered property names, by its identifier.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Target {
    Ships(CatalogGuid),
    Npcs(CatalogGuid),
    Entities(CatalogGuid),
    Location(CatalogGuid),
}

/// What registering a description or query made of it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Registered {
    pub(crate) target: Target,
    /// A spawn description without a group: it is not kept, since it states nothing, but a
    /// property of one still takes the place of the same variable at a lower level.
    pub(crate) empty: bool,
}

/// Keeps `pool` under its identifier unless it is `empty`; `(guid, empty)` is its identifier and
/// whether it has no group.
fn store<P>(
    pools: &mut BTreeMap<[u8; 36], P>,
    pool: P,
    (guid, empty): (CatalogGuid, bool),
    target: fn(CatalogGuid) -> Target,
) -> Registered {
    if !empty {
        pools.entry(guid.text()).or_insert(pool);
    }
    Registered {
        target: target(guid),
        empty,
    }
}

impl<'f> SpawnPools<'f> {
    /// Adds what `other` holds that `self` does not.
    pub(crate) fn merge(&mut self, other: Self) {
        for (key, pool) in other.ships {
            self.ships.entry(key).or_insert(pool);
        }
        for (key, pool) in other.npcs {
            self.npcs.entry(key).or_insert(pool);
        }
        for (key, pool) in other.entities {
            self.entities.entry(key).or_insert(pool);
        }
        for (key, query) in other.locations {
            self.locations.entry(key).or_insert(query);
        }
    }

    /// Finalizes the selected content rows and registers their protected NPC names.
    pub(crate) fn publish(
        self,
        projection: &CatalogProjection<'_, 'f>,
        cancel: Option<&CancelToken>,
    ) -> Result<PublishedSpawnPools, CatalogError> {
        let npc_projection = projection.for_kind(Kind::ContractNpcSpawns);
        let mut npcs = Vec::with_capacity(self.npcs.len());
        for pool in self.npcs.into_values() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            npcs.push(pool.finish(&npc_projection)?);
        }
        Ok(PublishedSpawnPools {
            ships: self.ships.into_values().collect(),
            npcs,
            entities: self.entities.into_values().collect(),
            locations: self.locations.into_values().collect(),
        })
    }

    /// Keeps `spawn` unless it is a spawn description without a group, and says what names it.
    pub(crate) fn register(&mut self, spawn: Spawn<'f>) -> Registered {
        match spawn {
            Spawn::Ships(pool) => {
                let id = (pool.guid, pool.groups.is_empty());
                store(&mut self.ships, pool, id, Target::Ships)
            }
            Spawn::Npcs(pool) => {
                let id = (pool.guid, pool.groups.is_empty());
                store(&mut self.npcs, pool, id, Target::Npcs)
            }
            Spawn::Entities(pool) => {
                let id = (pool.guid, pool.groups.is_empty());
                store(&mut self.entities, pool, id, Target::Entities)
            }
            Spawn::Location(query) => {
                let id = (query.guid, false);
                store(&mut self.locations, query, id, Target::Location)
            }
        }
    }
}
