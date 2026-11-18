// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, SourcedValue};

use super::inputs::{SelectedItem, unavailable_value};
use crate::catalog_projection::CatalogProjection;
use crate::playability_rules::{self, ItemRule};
use crate::{CatalogError, ExportAccountingError, Playability};

pub(super) fn read(
    inputs: &SelectedItem<'_, '_>,
    projection: &CatalogProjection<'_, '_>,
) -> Result<SourcedValue<Playability>, CatalogError> {
    let display = inputs.display_name();
    let uses = &projection.origins().uses;
    if let Some(value) = unavailable_value(display.state)? {
        return display.bind(value, uses);
    }
    let name = inputs.name();
    let path = inputs.record_path();
    let path_value = path
        .value()
        .ok_or(ExportAccountingError::MissingFieldInspection)?;
    let decision = playability_rules::item_inputs(display.key(), path_value, name.key());
    let state = decision
        .value
        .map_or(SourceValue::Absent, SourceValue::Present);
    let state = if decision.rule == ItemRule::Name {
        unavailable_value(name.state)?.unwrap_or(state)
    } else {
        state
    };
    let mut result = display.bind(state, uses)?;
    if decision.rule != ItemRule::Purchasable {
        result = uses.add_value_origins(result, &[path.origin()])?;
    }
    if decision.rule == ItemRule::Name {
        result = name.add_parents(result, uses)?;
    }
    Ok(result)
}
