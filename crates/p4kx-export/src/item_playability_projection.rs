// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUses, SourceValue, SourcedValue};

use super::inputs::{SelectedItem, unavailable_value};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::authored_fields::locale_input::AuthoredLocaleInput;
use crate::playability_rules::{self, ItemRule, PurchaseRank};
use crate::{CatalogError, ExportAccountingError, Playability};

pub(super) fn read(
    inputs: &SelectedItem<'_, '_>,
    projection: &CatalogProjection<'_, '_>,
) -> Result<SourcedValue<Playability>, CatalogError> {
    let uses = &projection.origins().uses;
    let rank = read_purchase_rank(inputs.purchasable_ordinal(), inputs.display_name(), uses)?;
    let Some(&purchase) = rank.value() else {
        return Ok(rank.map(|_| Playability::Player));
    };
    let name = inputs.name();
    let path = inputs.record_path();
    let path_value = path
        .value()
        .ok_or(ExportAccountingError::MissingFieldInspection)?;
    let decision = playability_rules::item_inputs(purchase, path_value, name.key());
    if decision.rule == ItemRule::Purchasable {
        return Ok(rank.map(|_| Playability::Player));
    }
    let state = decision
        .value
        .map_or(SourceValue::Absent, SourceValue::Present);
    let state = if decision.rule == ItemRule::Name {
        unavailable_value(name.state)?.unwrap_or(state)
    } else {
        state
    };
    let mut result = uses.value_with_origins(state, &[rank.origin(), path.origin()])?;
    if decision.rule == ItemRule::Name {
        result = name.add_parents(result, uses)?;
    }
    Ok(result)
}

/// Projects the observed purchase rank from the selected slot and its original Locale.
pub(crate) fn read_purchase_rank(
    selected: &SourcedValue<u32>,
    name: Option<&AuthoredLocaleInput<'_>>,
    uses: &SourceUses,
) -> Result<SourcedValue<PurchaseRank>, CatalogError> {
    let key = name.map(AuthoredLocaleInput::key);
    let state = key.map_or_else(|| selected.state(), SourcedValue::state);
    let rank = unavailable_value(state)?.unwrap_or_else(|| {
        let token = key.and_then(|key| key.value().copied());
        SourceValue::Present(playability_rules::purchase_rank(token))
    });
    let source = key.map_or_else(|| selected.origin(), SourcedValue::origin);
    Ok(uses.value_with_origins(rank, &[selected.origin(), source])?)
}
