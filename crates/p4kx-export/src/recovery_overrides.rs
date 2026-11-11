// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourcedValue};

use super::economy;
use super::projection::RecoveryReader;
use crate::CatalogError;
use crate::catalog_read::CatalogFields;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::contract_recovery::RecoveryOverride;
use crate::source_fields::FieldError;

const OVERRIDE_TYPE: &str = "ItemRecoveryOverrideGroupDef";

pub(crate) fn read<'source>(
    reader: &RecoveryReader<'_, 'source, '_>,
    economy: &CatalogFields<'_, 'source, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<Vec<SourcedValue<RecoveryOverride>>>, CatalogError> {
    reader
        .empty_class_target(economy, "overrides", OVERRIDE_TYPE, notes)?
        .try_map(|groups| {
            groups
                .iter()
                .enumerate()
                .map(|(ordinal, value)| {
                    if reader.cancel.is_some_and(CancelToken::is_cancelled) {
                        return Err(CatalogError::Cancelled);
                    }
                    let name = format!("overrides[{ordinal}]");
                    let path = economy.location(&name);
                    let value = economy.structure_element(&name, value, notes)?;
                    reader.typed(value, &path, OVERRIDE_TYPE, notes, group)
                })
                .collect()
        })
}

fn group(
    fields: &CatalogFields<'_, '_, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<RecoveryOverride, CatalogError> {
    fields.check_observed(
        &[
            "classes",
            "claimCooldownMultiplier",
            "claimCostMultiplier",
            "deliveryTimeMultiplier",
            "maxExpediteCostMultiplier",
            "expeditedDeliveryTimeMultiplier",
            "flatMinimumDeliveryTimerSecondsMultiplier",
            "escalationTimerMultiplier",
            "escalatingClaimCostMultiplier",
            "escalatingDeliveryTimerMultiplier",
        ],
        notes,
    )?;
    Ok(RecoveryOverride {
        classes: references(fields, "classes", notes)?,
        claim_cooldown_multiplier: economy::number(fields, "claimCooldownMultiplier", notes)?,
        claim_cost_multiplier: economy::number(fields, "claimCostMultiplier", notes)?,
        delivery_time_multiplier: economy::number(fields, "deliveryTimeMultiplier", notes)?,
        max_expedite_cost_multiplier: economy::number(fields, "maxExpediteCostMultiplier", notes)?,
        expedited_delivery_time_multiplier: economy::number(
            fields,
            "expeditedDeliveryTimeMultiplier",
            notes,
        )?,
        flat_minimum_delivery_timer_seconds_multiplier: economy::number(
            fields,
            "flatMinimumDeliveryTimerSecondsMultiplier",
            notes,
        )?,
        escalation_timer_multiplier: economy::number(fields, "escalationTimerMultiplier", notes)?,
        escalating_claim_cost_multiplier: economy::number(
            fields,
            "escalatingClaimCostMultiplier",
            notes,
        )?,
        escalating_delivery_timer_multiplier: economy::number(
            fields,
            "escalatingDeliveryTimerMultiplier",
            notes,
        )?,
    })
}

pub(crate) fn references(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>, CatalogError> {
    fields.array(name, notes)?.try_map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(ordinal, value)| {
                fields.reference_value(&format!("{name}[{ordinal}]"), Some(value), notes)
            })
            .collect()
    })
}
