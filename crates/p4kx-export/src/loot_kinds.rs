// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Record, RecordTree};

use crate::catalog_projection::CatalogProjection;
use crate::catalog_row::CatalogRow;
use crate::catalog_v3::loot_definition_catalog::{
    self, LootDefinition, LootDefinitionKind, LootDefinitions,
};
use crate::contract_loot::{LootArchetype, LootTable};
use crate::contract_loot_legacy::{LegacyLootArchetype, LegacyLootTable};
use crate::guid_text::sort_by_guid;
use crate::loot_legacy::{legacy_archetype, legacy_table};
use crate::{CatalogError, Kind};

/// An entity of one of the loot kinds.
pub(crate) enum LootPick<'f> {
    /// A loot table.
    Table(LootTable<'f>, Box<LootDefinition<'f>>),
    /// A loot archetype.
    Archetype(LootArchetype<'f>, Box<LootDefinition<'f>>),
    /// A physical Loot Record with an owner-local source value arena.
    Definition(LootDefinitionKind, Box<LootDefinition<'f>>),
    /// An older loot table.
    LegacyTable(Box<LegacyLootTable<'f>>),
    /// An older loot archetype.
    LegacyArchetype(Box<LegacyLootArchetype<'f>>),
}

impl<'f> LootPick<'f> {
    pub(crate) fn read_weighted(
        kind: LootDefinitionKind,
        record: &Record<'f>,
        tree: &RecordTree<'f>,
        projection: &CatalogProjection<'_, 'f>,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Self>, CatalogError> {
        Ok(Some(match kind {
            LootDefinitionKind::Table => {
                let (definition, row) = loot_definition_catalog::read_table(
                    record,
                    tree,
                    projection.original(),
                    cancel,
                )?;
                Self::Table(row, Box::new(definition))
            }
            LootDefinitionKind::Archetype => {
                let (definition, row) = loot_definition_catalog::read_archetype(
                    record,
                    tree,
                    projection.original(),
                    cancel,
                )?;
                Self::Archetype(row, Box::new(definition))
            }
            _ => return Ok(None),
        }))
    }

    pub(crate) fn read_legacy(
        kind: Kind,
        record: &Record<'f>,
        tree: &RecordTree<'f>,
        projection: &CatalogProjection<'_, 'f>,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Self>, CatalogError> {
        let input = projection.original();
        Ok(Some(match kind {
            Kind::LegacyLootTables => {
                Self::LegacyTable(Box::new(legacy_table(record, tree, input, cancel)?))
            }
            Kind::LegacyLootArchetypes => {
                Self::LegacyArchetype(Box::new(legacy_archetype(record, tree, input, cancel)?))
            }
            _ => return Ok(None),
        }))
    }
}

/// The entities of the loot kinds, each ordered by the text of the GUID.
#[derive(Default)]
pub(crate) struct LootKinds<'f> {
    pub(crate) tables: Vec<LootTable<'f>>,
    pub(crate) archetypes: Vec<LootArchetype<'f>>,
    pub(crate) definitions: LootDefinitions<'f>,
    pub(crate) legacy_tables: Vec<LegacyLootTable<'f>>,
    pub(crate) legacy_archetypes: Vec<LegacyLootArchetype<'f>>,
}

fn legacy_order(row: &LootDefinition<'_>) -> (crate::guid_text::GuidText, Option<u32>) {
    (
        row.physical_guid.text(),
        row.source.record_index.value().copied(),
    )
}

impl<'f> LootKinds<'f> {
    /// Stores `pick` under its kind.
    pub(crate) fn push(&mut self, pick: LootPick<'f>) {
        match pick {
            LootPick::Table(entity, definition) => {
                self.tables.push(entity);
                self.definitions
                    .push(LootDefinitionKind::Table, *definition);
            }
            LootPick::Archetype(entity, definition) => {
                self.archetypes.push(entity);
                self.definitions
                    .push(LootDefinitionKind::Archetype, *definition);
            }
            LootPick::Definition(kind, entity) => self.definitions.push(kind, *entity),
            LootPick::LegacyTable(entity) => self.legacy_tables.push(*entity),
            LootPick::LegacyArchetype(entity) => self.legacy_archetypes.push(*entity),
        }
    }

    /// Orders every kind by the text of the GUID.
    pub(crate) fn order(&mut self) {
        sort_by_guid(&mut self.tables, |entity| entity.guid);
        sort_by_guid(&mut self.archetypes, |entity| entity.guid);
        self.definitions.order();
        self.legacy_tables
            .sort_by_cached_key(|entity| legacy_order(&entity.original));
        self.legacy_archetypes
            .sort_by_cached_key(|entity| legacy_order(&entity.original));
    }

    pub(crate) fn legacy_sources(&self) -> impl Iterator<Item = (Kind, &LootDefinition<'f>)> {
        let tables = self
            .legacy_tables
            .iter()
            .map(|row| (Kind::LegacyLootTables, &row.original));
        let archetypes = self
            .legacy_archetypes
            .iter()
            .map(|row| (Kind::LegacyLootArchetypes, &row.original));
        tables.chain(archetypes)
    }

    pub(crate) fn legacy_emitted(&self) -> impl Iterator<Item = CatalogRow> {
        self.legacy_sources().map(|(kind, row)| CatalogRow {
            kind,
            guid: row.physical_guid.0,
            record_path: row.physical_path.to_owned(),
            record_index: row.source.record_index.value().copied(),
        })
    }

    /// Returns how many entities `kind` holds, 0 for a kind that is not a loot kind.
    pub(crate) const fn count(&self, kind: Kind) -> usize {
        match kind {
            Kind::LegacyLootTables => self.legacy_tables.len(),
            Kind::LegacyLootArchetypes => self.legacy_archetypes.len(),
            _ => match LootDefinitionKind::of(kind) {
                Some(kind) => self.definitions.count(kind),
                None => 0,
            },
        }
    }
}
