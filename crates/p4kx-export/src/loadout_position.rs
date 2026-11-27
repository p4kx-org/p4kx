// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::ExportAccountingError;
use crate::schema_fields::every_field_required;

/// A physical ship Loadout and a route through its nested port arrays.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LoadoutPosition {
    loadout_record_index: u32,
    pub(super) port_ordinals: Vec<u32>,
}

impl LoadoutPosition {
    /// Returns the original owning Loadout Record index.
    #[must_use]
    pub const fn loadout_record_index(&self) -> u32 {
        self.loadout_record_index
    }

    /// Borrows the nested port ordinals in traversal order.
    #[must_use]
    pub fn port_ordinals(&self) -> &[u32] {
        &self.port_ordinals
    }

    pub(crate) fn from_port_ordinals(
        loadout_record_index: u32,
        indices: &[usize],
    ) -> Result<Self, ExportAccountingError> {
        let port_ordinals = indices
            .iter()
            .copied()
            .map(ordinal)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            loadout_record_index,
            port_ordinals,
        })
    }

    pub(super) const fn root(loadout_record_index: u32) -> Self {
        Self {
            loadout_record_index,
            port_ordinals: Vec::new(),
        }
    }
}

pub(super) fn ordinal(index: usize) -> Result<u32, ExportAccountingError> {
    u32::try_from(index).map_err(|_| ExportAccountingError::CounterOverflow)
}
