// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use crate::Kind;
use crate::catalog_v3::loot_filter_catalog::LootFilterDefinition;
use crate::contract_loot::{LootArchetype, LootTable};
use crate::contract_loot_legacy::{LegacyLootArchetype, LegacyLootTable};
use crate::guid_text::sort_by_guid;
use crate::loot_legacy::{legacy_archetype, legacy_table};
use crate::loot_records::{loot_archetype, loot_table};

/// An entity of one of the loot kinds.
pub(crate) enum LootPick<'f> {
    /// A loot table.
    Table(LootTable<'f>),
    /// A loot archetype.
    Archetype(LootArchetype<'f>),
    /// A pool filter.
    Filter(Box<LootFilterDefinition<'f>>),
    /// An older loot table.
    LegacyTable(LegacyLootTable<'f>),
    /// An older loot archetype.
    LegacyArchetype(LegacyLootArchetype<'f>),
}

impl<'f> LootPick<'f> {
    /// Returns the entity of `kind` the Record tree `tree` describes, `None` for a kind that is
    /// not a loot kind.
    pub(crate) fn entity(kind: Kind, tree: &RecordTree<'f>) -> Option<Self> {
        Some(match kind {
            Kind::LootTables => Self::Table(loot_table(tree)),
            Kind::LootArchetypes => Self::Archetype(loot_archetype(tree)),
            Kind::LegacyLootTables => Self::LegacyTable(legacy_table(tree)),
            Kind::LegacyLootArchetypes => Self::LegacyArchetype(legacy_archetype(tree)),
            _ => return None,
        })
    }
}

/// The entities of the loot kinds, each ordered by the text of the GUID.
#[derive(Default)]
pub(crate) struct LootKinds<'f> {
    pub(crate) tables: Vec<LootTable<'f>>,
    pub(crate) archetypes: Vec<LootArchetype<'f>>,
    pub(crate) filters: Vec<LootFilterDefinition<'f>>,
    pub(crate) legacy_tables: Vec<LegacyLootTable<'f>>,
    pub(crate) legacy_archetypes: Vec<LegacyLootArchetype<'f>>,
}

impl<'f> LootKinds<'f> {
    /// Stores `pick` under its kind.
    pub(crate) fn push(&mut self, pick: LootPick<'f>) {
        match pick {
            LootPick::Table(entity) => self.tables.push(entity),
            LootPick::Archetype(entity) => self.archetypes.push(entity),
            LootPick::Filter(entity) => self.filters.push(*entity),
            LootPick::LegacyTable(entity) => self.legacy_tables.push(entity),
            LootPick::LegacyArchetype(entity) => self.legacy_archetypes.push(entity),
        }
    }

    /// Orders every kind by the text of the GUID.
    pub(crate) fn order(&mut self) {
        sort_by_guid(&mut self.tables, |entity| entity.guid);
        sort_by_guid(&mut self.archetypes, |entity| entity.guid);
        sort_by_guid(&mut self.filters, |entity| entity.physical_guid);
        sort_by_guid(&mut self.legacy_tables, |entity| entity.guid);
        sort_by_guid(&mut self.legacy_archetypes, |entity| entity.guid);
    }

    /// Returns how many entities `kind` holds, 0 for a kind that is not a loot kind.
    pub(crate) const fn count(&self, kind: Kind) -> usize {
        match kind {
            Kind::LootTables => self.tables.len(),
            Kind::LootArchetypes => self.archetypes.len(),
            Kind::LootPoolFilters => self.filters.len(),
            Kind::LegacyLootTables => self.legacy_tables.len(),
            Kind::LegacyLootArchetypes => self.legacy_archetypes.len(),
            _ => 0,
        }
    }
}
