// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(crate) const POOL_TYPE: &str = "NPCClassPool";
pub(crate) const ARCHETYPE_TYPE: &str = "NPCArchetype";

/// A physical NPC class pool and its ordered weighted entries.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct NpcClassPoolDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    npc_classes: SourcedValue<Vec<SourcedValue<NpcClassPoolEntry<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> NpcClassPoolDefinition<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let npc_classes = fields.class_array_to(
            "npcClasses",
            "NPCClassPoolEntry",
            notes,
            NpcClassPoolEntry::read,
        )?;
        let authored_fields =
            fields.authored_members_except([("npcClasses", npc_classes.state())])?;
        Ok(Self {
            source,
            npc_classes,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct NpcClassPoolEntry<'source> {
    weight: SourcedValue<ResourceFloat32>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    npc_class: SourcedValue<Box<SourceRecordReference<'source>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> NpcClassPoolEntry<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let weight = fields.single("weight", notes)?;
        let npc_class = fields.record_link_to("npcClass", "EntityClassDefinition", notes)?;
        let authored_fields = fields.authored_members_except([
            ("weight", weight.state()),
            ("npcClass", npc_class.state()),
        ])?;
        Ok(Self {
            weight,
            npc_class,
            authored_fields,
        })
    }
}

/// A physical NPC archetype and its authored identity-specific branches.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct NpcArchetypeDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    identity: SourcedValue<Vec<SourcedValue<NpcIdentityBranch<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> NpcArchetypeDefinition<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let identity = fields.class_array_to(
            "identity",
            "NPCIdentityToArchetype",
            notes,
            NpcIdentityBranch::read,
        )?;
        let authored_fields = fields.authored_members_except([("identity", identity.state())])?;
        Ok(Self {
            source,
            identity,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct NpcIdentityBranch<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    identity: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    npc_pools: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> NpcIdentityBranch<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let identity = fields.record_link_to("identity", "FactionIdentity", notes)?;
        let npc_pools = fields.record_link_array_to("npcPools", POOL_TYPE, notes)?;
        let authored_fields = fields.authored_members_except([
            ("identity", identity.state()),
            ("npcPools", npc_pools.state()),
        ])?;
        Ok(Self {
            identity,
            npc_pools,
            authored_fields,
        })
    }
}
