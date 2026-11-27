// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::spawn_model::TagPaths;

/// One alternative of an entity slot (`SpawnDescription_Entity`): an entity the game picks by
/// tags.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct EntitySpawnOption {
    /// The `amount` of entities that spawn; `null` when absent.
    pub amount: Option<i64>,
    /// The `weight` of the alternative among its slot; `null` when absent.
    pub weight: Option<f32>,
    /// The `tags` the game searches entities by.
    pub tags: TagPaths,
    /// The `negativeTags` a match must not carry.
    pub negative_tags: TagPaths,
    /// The `markupTags`.
    pub markup_tags: TagPaths,
    /// The `entityTags`.
    pub entity_tags: TagPaths,
}

/// The alternatives for one entity (`SpawnDescription_EntityOptions`): the game spawns one of them.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct EntitySpawnSlot {
    /// The alternatives in stored order.
    pub options: Vec<EntitySpawnOption>,
}

/// A group of entities that spawn together (`SpawnDescription_EntityGroup`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct EntitySpawnGroup {
    /// The `Name` of the group, `null` when empty.
    pub name: Option<String>,
    /// The entities of the group, one slot each.
    pub slots: Vec<EntitySpawnSlot>,
}

/// An entity spawn description of a contract: what a
/// `MissionPropertyValue_EntitySpawnDescriptions` property states, shared by every contract that
/// states the same.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct EntitySpawnPool {
    /// The identifier of the content (see the catalog documentation); not a GUID of the Archive.
    pub guid: CatalogGuid,
    /// The groups in stored order.
    pub groups: Vec<EntitySpawnGroup>,
}
