// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};
use schemars::JsonSchema;
use serde::Serialize;

use super::auto_spawn_definition::{AutoSpawnDefinition, AutoSpawnSet, EncounterIndex};
pub(crate) use super::auto_spawn_definition::{ENCOUNTER_TYPE, SET_TYPE, SPAWN_TYPE};
pub(crate) use super::npc_spawn_definition::{ARCHETYPE_TYPE, POOL_TYPE};
use super::npc_spawn_definition::{NpcArchetypeDefinition, NpcClassPoolDefinition};
use super::prepared_definition::PreparedDefinition;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
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
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<FieldError>), CatalogError> {
        if kind_for_type(tree.type_name()) != Some(kind) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        match kind {
            Kind::NpcClassPools => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                |source, fields, _, notes| NpcClassPoolDefinition::read(source, fields, notes),
            )
            .map(|(row, notes)| (Self::ClassPool(Box::new(row)), notes)),
            Kind::NpcArchetypes => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                |source, fields, _, notes| NpcArchetypeDefinition::read(source, fields, notes),
            )
            .map(|(row, notes)| (Self::Archetype(Box::new(row)), notes)),
            Kind::AutoSpawns => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                |source, fields, _, notes| AutoSpawnDefinition::read(source, fields, notes),
            )
            .map(|(row, notes)| (Self::Spawn(Box::new(row)), notes)),
            Kind::AutoSpawnSets => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                |source, fields, _, notes| AutoSpawnSet::read(source, fields, notes),
            )
            .map(|(row, notes)| (Self::Set(Box::new(row)), notes)),
            Kind::EncounterIndices => PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                |source, fields, _, notes| EncounterIndex::read(source, fields, notes),
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

    pub(crate) fn sources(
        &self,
    ) -> impl Iterator<Item = (Kind, Guid, &'source str, &[CapturedNumber])> {
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
