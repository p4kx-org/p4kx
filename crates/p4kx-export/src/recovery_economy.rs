// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, Value};

use super::projection::RecoveryReader;
use crate::CatalogError;
use crate::catalog_read::CatalogFields;
use crate::contract_recovery::{
    RecoveryCooldown, RecoveryDelivery, RecoveryEconomy, RecoveryEscalating, RecoveryLegacy,
    RecoveryRepair,
};
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;
use crate::resource_raw_policy::ResourceNumberContract;
use crate::source_fields::FieldError;

pub(crate) const ECONOMY: &str = "economyParams";
pub(crate) const ECONOMY_TYPE: &str = "ItemRecoveryEconomyParams";

pub(crate) fn read<'source>(
    reader: &RecoveryReader<'_, 'source, '_>,
    fields: &CatalogFields<'_, 'source, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<RecoveryEconomy, CatalogError> {
    fields.check_observed(
        &[
            "globalBrickTimer",
            "baseReclaimTime",
            "baseDeliveryTime",
            "scalingPriceFloor",
            "aUECperSecond",
            "defaultLoadoutCooldownMultiplier",
            "defaultLoadoutDeliveryTimeMultiplier",
            "claimCostMultiplier",
            "vehicleChassisRepairCostMultiplier",
            "vehicleComponentRepairCostMultiplier",
            "vehicleRepairMissingItemCostMultiplier",
            "deliverySecondsPerAUEC",
            "expeditedDeliverySecondsPerAUEC",
            "defaultLoadoutExpeditedDeliveryTimeMultiplier",
            "flatMinimumDeliveryTimerSeconds",
            "maxExpeditedClaimCostMultiplier",
            "cooldownParams",
            "deliveryParams",
            "repairParams",
            "escalatingClaimsParams",
            "overrides",
        ],
        notes,
    )?;
    Ok(RecoveryEconomy {
        legacy: legacy(fields, notes)?,
        cooldown_params: reader.block(
            fields,
            "cooldownParams",
            "ItemRecoveryCooldownParams",
            notes,
            cooldown,
        )?,
        delivery_params: reader.block(
            fields,
            "deliveryParams",
            "ItemRecoveryDeliveryTimeParams",
            notes,
            delivery,
        )?,
        repair_params: reader.block(
            fields,
            "repairParams",
            "ItemRecoveryRepairParams",
            notes,
            repair,
        )?,
        escalating_claims_params: reader.block(
            fields,
            "escalatingClaimsParams",
            "ItemRecoveryEscalatingClaimsParams",
            notes,
            escalating,
        )?,
    })
}

fn legacy(
    fields: &CatalogFields<'_, '_, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<RecoveryLegacy, CatalogError> {
    Ok(RecoveryLegacy {
        global_brick_timer: number(fields, "globalBrickTimer", notes)?,
        base_reclaim_time: number(fields, "baseReclaimTime", notes)?,
        base_delivery_time: number(fields, "baseDeliveryTime", notes)?,
        scaling_price_floor: number(fields, "scalingPriceFloor", notes)?,
        a_uec_per_second: number(fields, "aUECperSecond", notes)?,
        default_loadout_cooldown_multiplier: number(
            fields,
            "defaultLoadoutCooldownMultiplier",
            notes,
        )?,
        default_loadout_delivery_time_multiplier: number(
            fields,
            "defaultLoadoutDeliveryTimeMultiplier",
            notes,
        )?,
        claim_cost_multiplier: number(fields, "claimCostMultiplier", notes)?,
        vehicle_chassis_repair_cost_multiplier: number(
            fields,
            "vehicleChassisRepairCostMultiplier",
            notes,
        )?,
        vehicle_component_repair_cost_multiplier: number(
            fields,
            "vehicleComponentRepairCostMultiplier",
            notes,
        )?,
        vehicle_repair_missing_item_cost_multiplier: number(
            fields,
            "vehicleRepairMissingItemCostMultiplier",
            notes,
        )?,
        delivery_seconds_per_auec: number(fields, "deliverySecondsPerAUEC", notes)?,
        expedited_delivery_seconds_per_auec: number(
            fields,
            "expeditedDeliverySecondsPerAUEC",
            notes,
        )?,
        default_loadout_expedited_delivery_time_multiplier: number(
            fields,
            "defaultLoadoutExpeditedDeliveryTimeMultiplier",
            notes,
        )?,
        flat_minimum_delivery_timer_seconds: number(
            fields,
            "flatMinimumDeliveryTimerSeconds",
            notes,
        )?,
        max_expedited_claim_cost_multiplier: number(
            fields,
            "maxExpeditedClaimCostMultiplier",
            notes,
        )?,
    })
}

fn cooldown(
    fields: &CatalogFields<'_, '_, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<RecoveryCooldown, CatalogError> {
    fields.check_observed(
        &[
            "baseTime",
            "scalingPriceFloor",
            "aUECperSecond",
            "defaultLoadoutMultiplier",
        ],
        notes,
    )?;
    Ok(RecoveryCooldown {
        base_time: single(fields, "baseTime", notes)?,
        scaling_price_floor: single(fields, "scalingPriceFloor", notes)?,
        a_uec_per_second: single(fields, "aUECperSecond", notes)?,
        default_loadout_multiplier: single(fields, "defaultLoadoutMultiplier", notes)?,
    })
}

fn delivery(
    fields: &CatalogFields<'_, '_, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<RecoveryDelivery, CatalogError> {
    fields.check_observed(
        &[
            "baseTime",
            "scalingPriceFloor",
            "aUECperSecond",
            "defaultLoadoutMultiplier",
            "expeditedTimeMultiplier",
            "expeditedDefaultTimeMultiplier",
        ],
        notes,
    )?;
    Ok(RecoveryDelivery {
        base_time: single(fields, "baseTime", notes)?,
        scaling_price_floor: single(fields, "scalingPriceFloor", notes)?,
        a_uec_per_second: single(fields, "aUECperSecond", notes)?,
        default_loadout_multiplier: single(fields, "defaultLoadoutMultiplier", notes)?,
        expedited_time_multiplier: single(fields, "expeditedTimeMultiplier", notes)?,
        expedited_default_time_multiplier: single(fields, "expeditedDefaultTimeMultiplier", notes)?,
    })
}

fn repair(
    fields: &CatalogFields<'_, '_, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<RecoveryRepair, CatalogError> {
    fields.check_observed(
        &[
            "vehicleChassisRepairCostMultiplier",
            "vehicleComponentRepairCostMultiplier",
            "vehicleRepairMissingItemCostMultiplier",
        ],
        notes,
    )?;
    Ok(RecoveryRepair {
        chassis: single(fields, "vehicleChassisRepairCostMultiplier", notes)?,
        component: single(fields, "vehicleComponentRepairCostMultiplier", notes)?,
        missing_item: single(fields, "vehicleRepairMissingItemCostMultiplier", notes)?,
    })
}

fn escalating(
    fields: &CatalogFields<'_, '_, '_>,
    notes: &mut Vec<FieldError>,
) -> Result<RecoveryEscalating, CatalogError> {
    fields.check_observed(
        &[
            "escalatingClaimChainLimit",
            "escalationTimerMultiplier",
            "escalatingClaimCostMultiplier",
            "escalatingDeliveryTimerMultiplier",
        ],
        notes,
    )?;
    Ok(RecoveryEscalating {
        escalating_claim_chain_limit: integer(fields, "escalatingClaimChainLimit", notes)?,
        escalation_timer_multiplier: number(fields, "escalationTimerMultiplier", notes)?,
        escalating_claim_cost_multiplier: number(fields, "escalatingClaimCostMultiplier", notes)?,
        escalating_delivery_timer_multiplier: number(
            fields,
            "escalatingDeliveryTimerMultiplier",
            notes,
        )?,
    })
}

pub(crate) fn number(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
    fields.read(name, "a nullable number", notes, |original| {
        let value = ResourceNumber::read(original)?;
        fields.context().retain_number(
            || fields.location(name),
            fields.source().type_name(),
            ResourceNumberContract::Number,
            original,
        );
        Some(value)
    })
}

pub(crate) fn single(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<ResourceFloat32>, CatalogError> {
    fields.read(
        name,
        "a nullable Single",
        notes,
        |original| match original {
            Value::Single(value) => {
                fields.context().retain_number(
                    || fields.location(name),
                    fields.source().type_name(),
                    ResourceNumberContract::Single,
                    original,
                );
                Some(ResourceFloat32::new(*value))
            }
            _ => None,
        },
    )
}

fn integer(
    fields: &CatalogFields<'_, '_, '_>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<ResourceNumber>, CatalogError> {
    fields.read(
        name,
        "a nullable integer",
        notes,
        |original| match ResourceNumber::read(original) {
            Some(ResourceNumber::Single(_) | ResourceNumber::Double(_)) | None => None,
            Some(integer) => Some(integer),
        },
    )
}
