// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::guid_text::CatalogGuid;
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;

/// The selected physical item-recovery policy and its authored economy.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ItemRecoveryEconomy<'source> {
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) economy_params: SourcedValue<RecoveryEconomy>,
}

/// Distinct source layouts and independent global economy settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RecoveryEconomy {
    pub(crate) legacy: RecoveryLegacy,
    pub(crate) cooldown_params: SourcedValue<RecoveryCooldown>,
    pub(crate) delivery_params: SourcedValue<RecoveryDelivery>,
    pub(crate) repair_params: SourcedValue<RecoveryRepair>,
    pub(crate) escalating_claims_params: SourcedValue<RecoveryEscalating>,
}

/// The selected physical policy's original override array, in source order.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ItemRecoveryOverrides {
    pub(crate) owner_guid: SourcedValue<CatalogGuid>,
    pub(crate) owner_record_index: SourcedValue<u32>,
    pub(crate) overrides: SourcedValue<Vec<SourcedValue<RecoveryOverride>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RecoveryLegacy {
    pub(crate) global_brick_timer: SourcedValue<ResourceNumber>,
    pub(crate) base_reclaim_time: SourcedValue<ResourceNumber>,
    pub(crate) base_delivery_time: SourcedValue<ResourceNumber>,
    pub(crate) scaling_price_floor: SourcedValue<ResourceNumber>,
    #[serde(rename = "aUECperSecond")]
    pub(crate) a_uec_per_second: SourcedValue<ResourceNumber>,
    pub(crate) default_loadout_cooldown_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) default_loadout_delivery_time_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) claim_cost_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) vehicle_chassis_repair_cost_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) vehicle_component_repair_cost_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) vehicle_repair_missing_item_cost_multiplier: SourcedValue<ResourceNumber>,
    #[serde(rename = "deliverySecondsPerAUEC")]
    pub(crate) delivery_seconds_per_auec: SourcedValue<ResourceNumber>,
    #[serde(rename = "expeditedDeliverySecondsPerAUEC")]
    pub(crate) expedited_delivery_seconds_per_auec: SourcedValue<ResourceNumber>,
    pub(crate) default_loadout_expedited_delivery_time_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) flat_minimum_delivery_timer_seconds: SourcedValue<ResourceNumber>,
    pub(crate) max_expedited_claim_cost_multiplier: SourcedValue<ResourceNumber>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RecoveryCooldown {
    pub(crate) base_time: SourcedValue<ResourceFloat32>,
    pub(crate) scaling_price_floor: SourcedValue<ResourceFloat32>,
    #[serde(rename = "aUECperSecond")]
    pub(crate) a_uec_per_second: SourcedValue<ResourceFloat32>,
    pub(crate) default_loadout_multiplier: SourcedValue<ResourceFloat32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RecoveryDelivery {
    pub(crate) base_time: SourcedValue<ResourceFloat32>,
    pub(crate) scaling_price_floor: SourcedValue<ResourceFloat32>,
    #[serde(rename = "aUECperSecond")]
    pub(crate) a_uec_per_second: SourcedValue<ResourceFloat32>,
    pub(crate) default_loadout_multiplier: SourcedValue<ResourceFloat32>,
    pub(crate) expedited_time_multiplier: SourcedValue<ResourceFloat32>,
    pub(crate) expedited_default_time_multiplier: SourcedValue<ResourceFloat32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RecoveryRepair {
    #[serde(rename = "vehicleChassisRepairCostMultiplier")]
    pub(crate) chassis: SourcedValue<ResourceFloat32>,
    #[serde(rename = "vehicleComponentRepairCostMultiplier")]
    pub(crate) component: SourcedValue<ResourceFloat32>,
    #[serde(rename = "vehicleRepairMissingItemCostMultiplier")]
    pub(crate) missing_item: SourcedValue<ResourceFloat32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RecoveryEscalating {
    pub(crate) escalating_claim_chain_limit: SourcedValue<ResourceNumber>,
    pub(crate) escalation_timer_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) escalating_claim_cost_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) escalating_delivery_timer_multiplier: SourcedValue<ResourceNumber>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RecoveryOverride {
    pub(crate) classes: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
    pub(crate) claim_cooldown_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) claim_cost_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) delivery_time_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) max_expedite_cost_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) expedited_delivery_time_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) flat_minimum_delivery_timer_seconds_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) escalation_timer_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) escalating_claim_cost_multiplier: SourcedValue<ResourceNumber>,
    pub(crate) escalating_delivery_timer_multiplier: SourcedValue<ResourceNumber>,
}
