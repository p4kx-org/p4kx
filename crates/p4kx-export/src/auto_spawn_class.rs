// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::npc_spawn_definitions::{ARCHETYPE_TYPE, POOL_TYPE};
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

const BASE_TYPE: &str = "AutoSpawnClass";
const ENTITY_TYPE: &str = "EntityClassDefinition";

type RecordLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

/// An authored selector for a direct class, an archetype, or a class pool.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct AutoSpawnClass<'source> {
    /// The declared class selector type.
    pub source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    /// The typed interpretation of the original selector.
    pub selector: ClassSelector<'source>,
    /// Additional original fields.
    pub authored_fields: SourceArenaOutput<'source>,
}

/// The authored kind of an automatic spawn class selector.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum ClassSelector<'source> {
    /// The selector declares an entity class directly.
    TargetClass {
        /// The original entity class reference.
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        npc_class: RecordLink<'source>,
    },
    /// The selector declares weighted archetypes.
    Archetype {
        /// The candidate archetypes in authored order.
        archetype: SourcedValue<Vec<SourcedValue<NpcArchetypeWeight<'source>>>>,
    },
    /// The selector declares class pools.
    ClassPool {
        /// The original pool references in authored order.
        #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
        class_pool: SourcedValue<Vec<RecordLink<'source>>>,
    },
    /// An additional original selector type retained in authored fields.
    Authored,
}

impl<'source> AutoSpawnClass<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        fields.strong_pointer_family_to("class", BASE_TYPE, notes, Self::read_concrete)
    }

    fn read_concrete(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let selector = match fields.source_type()? {
            "AutoSpawnTargetClass" => ClassSelector::TargetClass {
                npc_class: fields.record_link_to("npcClass", ENTITY_TYPE, notes)?,
            },
            "AutoSpawnArchetype" => ClassSelector::Archetype {
                archetype: fields.class_array_to(
                    "archetype",
                    "NPCArchetypeWeight",
                    notes,
                    NpcArchetypeWeight::read,
                )?,
            },
            "AutoSpawnClassPool" => ClassSelector::ClassPool {
                class_pool: fields.record_link_array_to("classPool", POOL_TYPE, notes)?,
            },
            _ => {
                fields.note_retained_variant(notes)?;
                ClassSelector::Authored
            }
        };
        Ok(Self {
            source_type: fields.source_type_value()?,
            authored_fields: fields.authored_members_except(selector.projected())?,
            selector,
        })
    }
}

impl ClassSelector<'_> {
    const fn projected(&self) -> Option<(&'static str, SourceValueState)> {
        match self {
            Self::TargetClass { npc_class } => Some(("npcClass", npc_class.state())),
            Self::Archetype { archetype } => Some(("archetype", archetype.state())),
            Self::ClassPool { class_pool } => Some(("classPool", class_pool.state())),
            Self::Authored => None,
        }
    }
}

/// An authored weighted archetype candidate.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct NpcArchetypeWeight<'source> {
    /// The authored weight.
    pub weight: SourcedValue<ResourceFloat32>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// The original archetype reference.
    pub record: RecordLink<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> NpcArchetypeWeight<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let weight = fields.single("weight", notes)?;
        let record = fields.record_link_to("record", ARCHETYPE_TYPE, notes)?;
        let authored_fields = fields
            .authored_members_except([("weight", weight.state()), ("record", record.state())])?;
        Ok(Self {
            weight,
            record,
            authored_fields,
        })
    }
}
