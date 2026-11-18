// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// A generic dismantle process: a `CraftingBlueprintRecord` under `crafting/blueprints/dismantle`
/// whose process is a `GenericCraftingProcess_Dismantle`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DismantleProcess<'a> {
    /// The GUID of the Record.
    pub(crate) guid: CatalogGuid,
    /// The Record name without its type.
    pub(crate) class_name: &'a str,
    /// The record path, without `.xml`.
    pub(crate) record_path: &'a str,
    /// The share of the costs a dismantled item returns, 0 to 1, `null` when the Record holds
    /// none.
    pub(crate) efficiency: Option<f32>,
    /// The dismantle time in whole seconds, `null` when the Record holds none.
    pub(crate) time_seconds: Option<i64>,
}
