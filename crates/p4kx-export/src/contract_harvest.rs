// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// What a harvestable spawns, followed from the `HarvestablePreset` Record to the entity and its
/// mining parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct HarvestableLink<'a> {
    /// The GUID of the `HarvestablePreset` Record the Reference `harvestable` links to; `null`
    /// when the element names an entity only.
    pub(crate) preset_guid: Option<CatalogGuid>,
    /// The Record name of the `HarvestablePreset` without its type, e.g.
    /// `Mining_Common_Aluminum`; `null` for none or an unreadable Record.
    pub(crate) preset_class_name: Option<&'a str>,
    /// The GUID of the entity: `harvestableEntityClass` of the element, else `entityClass` of the
    /// preset.
    pub(crate) entity_guid: Option<CatalogGuid>,
    /// The GUID of the `MineableComposition` Record the entity's `MineableParams` link to; `null`
    /// for an entity that is not mineable.
    pub(crate) mineable_composition_guid: Option<CatalogGuid>,
    /// The GUID of the `MiningGlobalParams` Record the entity's `MineableParams` link to.
    pub(crate) mining_global_params_guid: Option<CatalogGuid>,
}
