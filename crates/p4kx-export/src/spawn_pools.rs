// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::entity_spawn_model::EntitySpawnPool;
use crate::guid_text::CatalogGuid;
use crate::npc_spawn_model::NpcSpawnPool;
use crate::spawn_model::{LocationQuery, ShipSpawnPool};
use crate::spawn_read::Spawn;
use crate::texts::Texts;

/// The distinct ship, NPC and entity spawn descriptions and location queries, by identifier.
#[derive(Debug, Default)]
pub(crate) struct SpawnPools<'f> {
    pub(crate) ships: BTreeMap<[u8; 36], ShipSpawnPool>,
    pub(crate) npcs: BTreeMap<[u8; 36], NpcSpawnPool<'f>>,
    pub(crate) entities: BTreeMap<[u8; 36], EntitySpawnPool>,
    pub(crate) locations: BTreeMap<[u8; 36], LocationQuery>,
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

    /// Resolves the texts the pools hold by their keys.
    pub(crate) fn resolve_texts(&mut self, texts: &Texts<'f>) {
        self.npcs.values_mut().for_each(|pool| pool.resolve(texts));
    }
}
