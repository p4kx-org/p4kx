// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// A crafting station: an entity whose `Components` hold a `CrafterComponentParams`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Crafter<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// `AttachDef.Type` of the entity, e.g. `Crafter`; `null` without an `AttachDef`. With
    /// `subType` and `size` it is what a port's `types` and size range are matched against.
    pub(crate) r#type: Option<&'a str>,
    /// `AttachDef.SubType`.
    pub(crate) sub_type: Option<&'a str>,
    /// `AttachDef.Size`, an integer.
    pub(crate) size: Option<i64>,
    /// `maxJobsInHistory`: how many finished jobs the station lists.
    pub(crate) max_jobs_in_history: Option<i64>,
    /// `baseCraftingTime`; `null` when the Record stores no number.
    pub(crate) base_crafting_time: Option<f32>,
    /// The job queues, from `queues` in stored order.
    pub(crate) queues: Vec<CrafterQueue<'a>>,
    /// The crafting modifiers, from `modifiers` in stored order; `[]` without any.
    pub(crate) modifiers: Vec<CrafterModifier<'a>>,
    /// The codes of the problems found in the modifiers and queues, e.g. `unknown_modifier_type`,
    /// sorted and without duplicates; empty when everything was understood.
    pub(crate) issues: Vec<&'static str>,
    /// The GUIDs of the ships whose Default Loadout mounts the crafter in any port, ordered by
    /// GUID; empty for a crafter no ship mounts, e.g. a station.
    pub(crate) host_guids: Vec<CatalogGuid>,
}

/// One job queue of a [`Crafter`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CrafterQueue<'a> {
    /// `debugName`.
    pub(crate) name: Option<&'a str>,
    /// `maxJobsInProgress`: how many jobs the queue crafts at once.
    pub(crate) max_jobs_in_progress: Option<i64>,
    /// `maxJobsWaiting`: how many further jobs the queue holds.
    pub(crate) max_jobs_waiting: Option<i64>,
    /// The blueprint categories the queue accepts: the struct `blueprintCategoryAvailability`
    /// holds. `kind` is `whitelist`, `none` or `unknown`.
    pub(crate) blueprint_category_availability: CategoryRule<'a>,
    /// `processTypeAvailability` as stored: one boolean per process type, in the order of the
    /// game's process types, which the Archive does not name; `[]` when the Record has none.
    pub(crate) process_type_availability: Vec<bool>,
}

/// A crafting modifier of a [`Crafter`]: a `CraftingModifierParams`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CrafterModifier<'a> {
    /// What the modifier changes: `time` (`CraftingModifierType_Time`), `none` (no
    /// `modifierType` stored) or `unknown` (a struct this export does not know, kept with its
    /// `structName` and no values). A closed set the game may extend, so the schema keeps a
    /// string.
    pub(crate) r#type: &'static str,
    /// The name of the struct `modifierType` holds; `null` for none.
    pub(crate) struct_name: Option<&'a str>,
    /// `timeMultiplier` of a `time` modifier: the factor on the crafting time (below 1 is
    /// faster); `null` for any other type or when the Record stores no number.
    pub(crate) time_multiplier: Option<f32>,
    /// When the modifier applies: the struct `modifierCondition` holds. `kind` is
    /// `blueprintCategory`, `none` (no condition stored) or `unknown`.
    pub(crate) condition: CategoryRule<'a>,
}

/// A polymorphic struct that names blueprint categories: the condition of a modifier or the
/// availability of a queue.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CategoryRule<'a> {
    /// Which struct it is, as the parent field documents; `unknown` keeps an unrecognised struct
    /// without values. A closed set the game may extend, so the schema keeps a string.
    pub(crate) kind: &'static str,
    /// The name of the struct; `null` when the field holds none.
    pub(crate) struct_name: Option<&'a str>,
    /// Category Record GUIDs in `definitions.blueprintCategories.rows`, in stored order;
    /// `[]` for `none` and `unknown`.
    pub(crate) blueprint_category_guids: Vec<CatalogGuid>,
}
