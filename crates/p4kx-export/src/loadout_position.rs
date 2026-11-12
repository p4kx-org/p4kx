// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::ExportAccountingError;
use crate::schema_fields::every_field_required;

/// A position in the published ship Loadout and its nested port arrays.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct LoadoutPosition {
    pub(super) loadout_ordinal: u32,
    pub(super) port_ordinals: Vec<u32>,
}

pub(super) fn ordinal(index: usize) -> Result<u32, ExportAccountingError> {
    u32::try_from(index).map_err(|_| ExportAccountingError::CounterOverflow)
}
