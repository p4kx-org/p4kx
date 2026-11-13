// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};
use schemars::JsonSchema;
use serde::Serialize;

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
        _ => None,
    }
}

pub(crate) enum NpcSpawnDefinitionsPick<'source> {
    ClassPool(Box<PreparedDefinition<'source, NpcClassPoolDefinition<'source>>>),
    Archetype(Box<PreparedDefinition<'source, NpcArchetypeDefinition<'source>>>),
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
            _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }
}

#[derive(Default)]
pub(crate) struct NpcSpawnDefinitions<'source> {
    class_pools: Vec<PreparedDefinition<'source, NpcClassPoolDefinition<'source>>>,
    archetypes: Vec<PreparedDefinition<'source, NpcArchetypeDefinition<'source>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct NpcSpawnDefinitionsTables<'source> {
    npc_class_pools: CatalogTable<NpcClassPoolDefinition<'source>>,
    npc_archetypes: CatalogTable<NpcArchetypeDefinition<'source>>,
}

impl<'source> NpcSpawnDefinitions<'source> {
    pub(crate) fn push(&mut self, pick: NpcSpawnDefinitionsPick<'source>) {
        match pick {
            NpcSpawnDefinitionsPick::ClassPool(row) => self.class_pools.push(*row),
            NpcSpawnDefinitionsPick::Archetype(row) => self.archetypes.push(*row),
        }
    }

    pub(crate) fn order(&mut self) {
        PreparedDefinition::order(&mut self.class_pools);
        PreparedDefinition::order(&mut self.archetypes);
    }

    pub(crate) const fn count(&self, kind: Kind) -> usize {
        match kind {
            Kind::NpcClassPools => self.class_pools.len(),
            Kind::NpcArchetypes => self.archetypes.len(),
            _ => 0,
        }
    }

    pub(crate) fn sources(
        &self,
    ) -> impl Iterator<Item = (Kind, Guid, &'source str, &[CapturedNumber])> {
        PreparedDefinition::sources(Kind::NpcClassPools, &self.class_pools).chain(
            PreparedDefinition::sources(Kind::NpcArchetypes, &self.archetypes),
        )
    }

    pub(crate) fn number_groups(&self) -> impl Iterator<Item = (Kind, &[CapturedNumber])> {
        self.sources().map(|(kind, _, _, numbers)| (kind, numbers))
    }

    pub(crate) fn into_tables(self, forge: &DataForge<'_>) -> NpcSpawnDefinitionsTables<'source> {
        NpcSpawnDefinitionsTables {
            npc_class_pools: PreparedDefinition::table(forge, POOL_TYPE, self.class_pools),
            npc_archetypes: PreparedDefinition::table(forge, ARCHETYPE_TYPE, self.archetypes),
        }
    }
}
