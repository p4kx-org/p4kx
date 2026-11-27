// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};
use schemars::JsonSchema;
use serde::Serialize;

pub use super::auto_spawn_class::{AutoSpawnClass, ClassSelector, NpcArchetypeWeight};
pub use super::auto_spawn_data::{
    AssignmentKind, AutoSpawnData, SpawnAssignment, SpawnInventory, SpawnLimits, SpawnTags,
};
pub use super::auto_spawn_definition::{
    AutoSpawnDefinition, AutoSpawnSet, AutoSpawnSetEntry, EncounterIndex, EncounterIndexEntry,
    SetEntryData,
};
pub(crate) use super::auto_spawn_definition::{ENCOUNTER_TYPE, SET_TYPE, SPAWN_TYPE};
pub(crate) use super::npc_spawn_definition::{ARCHETYPE_TYPE, POOL_TYPE};
pub use super::npc_spawn_definition::{
    NpcArchetypeDefinition, NpcClassPoolDefinition, NpcClassPoolEntry, NpcIdentityBranch,
};
use super::prepared_definition::PreparedDefinition;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_row::CatalogRow;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) fn kind_for_type(source_type: &str) -> Option<Kind> {
    match source_type {
        POOL_TYPE => Some(Kind::NpcClassPools),
        ARCHETYPE_TYPE => Some(Kind::NpcArchetypes),
        SPAWN_TYPE => Some(Kind::AutoSpawns),
        SET_TYPE => Some(Kind::AutoSpawnSets),
        ENCOUNTER_TYPE => Some(Kind::EncounterIndices),
        _ => None,
    }
}

/// The canonical rows for one authored NPC spawn definition family.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub enum NpcSpawnDefinitionRows<'source> {
    /// Physical NPC archetype Records.
    Archetypes(Vec<NpcArchetypeDefinition<'source>>),
    /// Physical weighted NPC class pool Records.
    ClassPools(Vec<NpcClassPoolDefinition<'source>>),
    /// Physical automatic spawn Records.
    Spawns(Vec<AutoSpawnDefinition<'source>>),
    /// Physical automatic spawn-set Records.
    Sets(Vec<AutoSpawnSet<'source>>),
    /// Physical encounter index Records.
    Encounters(Vec<EncounterIndex<'source>>),
}

pub(crate) enum NpcSpawnDefinitionsPick<'source> {
    ClassPool(Box<PreparedDefinition<'source, NpcClassPoolDefinition<'source>>>),
    Archetype(Box<PreparedDefinition<'source, NpcArchetypeDefinition<'source>>>),
    Spawn(Box<PreparedDefinition<'source, AutoSpawnDefinition<'source>>>),
    Set(Box<PreparedDefinition<'source, AutoSpawnSet<'source>>>),
    Encounter(Box<PreparedDefinition<'source, EncounterIndex<'source>>>),
}

impl<'source> NpcSpawnDefinitionsPick<'source> {
    pub(crate) fn read(
        kind: Kind,
        tree: &RecordTree<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<FieldError>), CatalogError> {
        if kind_for_type(tree.type_name()) != Some(kind) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        match kind {
            Kind::NpcClassPools => PreparedDefinition::read_original(
                tree,
                input,
                admission,
                cancel,
                NpcClassPoolDefinition::read,
            )
            .map(|(row, notes)| (Self::ClassPool(Box::new(row)), notes)),
            Kind::NpcArchetypes => PreparedDefinition::read_original(
                tree,
                input,
                admission,
                cancel,
                NpcArchetypeDefinition::read,
            )
            .map(|(row, notes)| (Self::Archetype(Box::new(row)), notes)),
            Kind::AutoSpawns => PreparedDefinition::read_original(
                tree,
                input,
                admission,
                cancel,
                AutoSpawnDefinition::read,
            )
            .map(|(row, notes)| (Self::Spawn(Box::new(row)), notes)),
            Kind::AutoSpawnSets => PreparedDefinition::read_original(
                tree,
                input,
                admission,
                cancel,
                AutoSpawnSet::read,
            )
            .map(|(row, notes)| (Self::Set(Box::new(row)), notes)),
            Kind::EncounterIndices => PreparedDefinition::read_original(
                tree,
                input,
                admission,
                cancel,
                EncounterIndex::read,
            )
            .map(|(row, notes)| (Self::Encounter(Box::new(row)), notes)),
            _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }
}

#[derive(Default)]
pub(crate) struct NpcSpawnDefinitions<'source> {
    class_pools: Vec<PreparedDefinition<'source, NpcClassPoolDefinition<'source>>>,
    archetypes: Vec<PreparedDefinition<'source, NpcArchetypeDefinition<'source>>>,
    spawns: Vec<PreparedDefinition<'source, AutoSpawnDefinition<'source>>>,
    sets: Vec<PreparedDefinition<'source, AutoSpawnSet<'source>>>,
    encounters: Vec<PreparedDefinition<'source, EncounterIndex<'source>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct NpcSpawnDefinitionsTables<'source> {
    npc_class_pools: CatalogTable<NpcClassPoolDefinition<'source>>,
    npc_archetypes: CatalogTable<NpcArchetypeDefinition<'source>>,
    auto_spawns: CatalogTable<AutoSpawnDefinition<'source>>,
    auto_spawn_sets: CatalogTable<AutoSpawnSet<'source>>,
    encounter_indices: CatalogTable<EncounterIndex<'source>>,
}

impl<'source> NpcSpawnDefinitions<'source> {
    pub(crate) fn push(&mut self, pick: NpcSpawnDefinitionsPick<'source>) {
        match pick {
            NpcSpawnDefinitionsPick::ClassPool(row) => self.class_pools.push(*row),
            NpcSpawnDefinitionsPick::Archetype(row) => self.archetypes.push(*row),
            NpcSpawnDefinitionsPick::Spawn(row) => self.spawns.push(*row),
            NpcSpawnDefinitionsPick::Set(row) => self.sets.push(*row),
            NpcSpawnDefinitionsPick::Encounter(row) => self.encounters.push(*row),
        }
    }

    pub(crate) fn order(&mut self) {
        PreparedDefinition::order(&mut self.class_pools);
        PreparedDefinition::order(&mut self.archetypes);
        PreparedDefinition::order(&mut self.spawns);
        PreparedDefinition::order(&mut self.sets);
        PreparedDefinition::order(&mut self.encounters);
    }

    pub(crate) const fn count(&self, kind: Kind) -> usize {
        match kind {
            Kind::NpcClassPools => self.class_pools.len(),
            Kind::NpcArchetypes => self.archetypes.len(),
            Kind::AutoSpawns => self.spawns.len(),
            Kind::AutoSpawnSets => self.sets.len(),
            Kind::EncounterIndices => self.encounters.len(),
            _ => 0,
        }
    }

    pub(crate) fn catalog_rows(&self) -> impl Iterator<Item = CatalogRow> + '_ {
        PreparedDefinition::catalog_rows(&self.class_pools, Kind::NpcClassPools)
            .chain(PreparedDefinition::catalog_rows(
                &self.archetypes,
                Kind::NpcArchetypes,
            ))
            .chain(PreparedDefinition::catalog_rows(
                &self.spawns,
                Kind::AutoSpawns,
            ))
            .chain(PreparedDefinition::catalog_rows(
                &self.sets,
                Kind::AutoSpawnSets,
            ))
            .chain(PreparedDefinition::catalog_rows(
                &self.encounters,
                Kind::EncounterIndices,
            ))
    }

    fn sources(&self) -> impl Iterator<Item = (Kind, Guid, &'source str, &[CapturedNumber])> {
        PreparedDefinition::sources(Kind::NpcClassPools, &self.class_pools)
            .chain(PreparedDefinition::sources(
                Kind::NpcArchetypes,
                &self.archetypes,
            ))
            .chain(PreparedDefinition::sources(Kind::AutoSpawns, &self.spawns))
            .chain(PreparedDefinition::sources(Kind::AutoSpawnSets, &self.sets))
            .chain(PreparedDefinition::sources(
                Kind::EncounterIndices,
                &self.encounters,
            ))
    }

    pub(crate) fn number_groups(&self) -> impl Iterator<Item = (Kind, &[CapturedNumber])> {
        self.sources().map(|(kind, _, _, numbers)| (kind, numbers))
    }

    pub(crate) fn into_query_rows(
        self,
        kind: Kind,
    ) -> Result<NpcSpawnDefinitionRows<'source>, CatalogError> {
        fn rows<T>(prepared: Vec<PreparedDefinition<'_, T>>) -> Vec<T> {
            prepared
                .into_iter()
                .map(PreparedDefinition::into_definition)
                .collect()
        }
        Ok(match kind {
            Kind::NpcArchetypes => NpcSpawnDefinitionRows::Archetypes(rows(self.archetypes)),
            Kind::NpcClassPools => NpcSpawnDefinitionRows::ClassPools(rows(self.class_pools)),
            Kind::AutoSpawns => NpcSpawnDefinitionRows::Spawns(rows(self.spawns)),
            Kind::AutoSpawnSets => NpcSpawnDefinitionRows::Sets(rows(self.sets)),
            Kind::EncounterIndices => NpcSpawnDefinitionRows::Encounters(rows(self.encounters)),
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }

    pub(crate) fn into_tables(self, forge: &DataForge<'_>) -> NpcSpawnDefinitionsTables<'source> {
        NpcSpawnDefinitionsTables {
            npc_class_pools: PreparedDefinition::table(forge, POOL_TYPE, self.class_pools),
            npc_archetypes: PreparedDefinition::table(forge, ARCHETYPE_TYPE, self.archetypes),
            auto_spawns: PreparedDefinition::table(forge, SPAWN_TYPE, self.spawns),
            auto_spawn_sets: PreparedDefinition::table(forge, SET_TYPE, self.sets),
            encounter_indices: PreparedDefinition::table(forge, ENCOUNTER_TYPE, self.encounters),
        }
    }
}
