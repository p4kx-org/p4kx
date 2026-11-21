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
#[non_exhaustive]
pub struct ItemRecoveryEconomy<'source> {
    /// The original physical owner and its Record header fields.
    pub source: CanonicalRecordHeader<'source>,
    /// The authored economy settings.
    pub economy_params: SourcedValue<RecoveryEconomy>,
}

/// Distinct source layouts and independent global economy settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct RecoveryEconomy {
    /// Authored settings stored directly on the economy block.
    pub legacy: RecoveryLegacy,
    /// The authored claim cooldown settings.
    pub cooldown_params: SourcedValue<RecoveryCooldown>,
    /// The authored delivery-time settings.
    pub delivery_params: SourcedValue<RecoveryDelivery>,
    /// The authored repair settings.
    pub repair_params: SourcedValue<RecoveryRepair>,
    /// The authored escalating-claim settings.
    pub escalating_claims_params: SourcedValue<RecoveryEscalating>,
}

/// The selected physical policy's original override array, in source order.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ItemRecoveryOverrides {
    /// The original owner's GUID.
    pub owner_guid: SourcedValue<CatalogGuid>,
    /// The original owner's physical Record index.
    pub owner_record_index: SourcedValue<u32>,
    /// Authored classes and modifiers in original array order.
    pub overrides: SourcedValue<Vec<SourcedValue<RecoveryOverride>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Legacy item recovery economy values stored directly on the source block.
#[non_exhaustive]
pub struct RecoveryLegacy {
    /// The authored global brick timer.
    pub global_brick_timer: SourcedValue<ResourceNumber>,
    /// The authored base reclaim time.
    pub base_reclaim_time: SourcedValue<ResourceNumber>,
    /// The authored base delivery time.
    pub base_delivery_time: SourcedValue<ResourceNumber>,
    /// The authored scaling price floor.
    pub scaling_price_floor: SourcedValue<ResourceNumber>,
    #[serde(rename = "aUECperSecond")]
    /// The authored currency rate in aUEC per second.
    pub a_uec_per_second: SourcedValue<ResourceNumber>,
    /// The authored default loadout cooldown multiplier.
    pub default_loadout_cooldown_multiplier: SourcedValue<ResourceNumber>,
    /// The authored default loadout delivery time multiplier.
    pub default_loadout_delivery_time_multiplier: SourcedValue<ResourceNumber>,
    /// The authored claim cost multiplier.
    pub claim_cost_multiplier: SourcedValue<ResourceNumber>,
    /// The authored vehicle chassis repair cost multiplier.
    pub vehicle_chassis_repair_cost_multiplier: SourcedValue<ResourceNumber>,
    /// The authored vehicle component repair cost multiplier.
    pub vehicle_component_repair_cost_multiplier: SourcedValue<ResourceNumber>,
    /// The authored vehicle repair missing item cost multiplier.
    pub vehicle_repair_missing_item_cost_multiplier: SourcedValue<ResourceNumber>,
    #[serde(rename = "deliverySecondsPerAUEC")]
    /// The authored delivery time in seconds per aUEC.
    pub delivery_seconds_per_auec: SourcedValue<ResourceNumber>,
    #[serde(rename = "expeditedDeliverySecondsPerAUEC")]
    /// The authored expedited delivery time in seconds per aUEC.
    pub expedited_delivery_seconds_per_auec: SourcedValue<ResourceNumber>,
    /// The authored default loadout expedited delivery time multiplier.
    pub default_loadout_expedited_delivery_time_multiplier: SourcedValue<ResourceNumber>,
    /// The authored flat minimum delivery timer seconds.
    pub flat_minimum_delivery_timer_seconds: SourcedValue<ResourceNumber>,
    /// The authored max expedited claim cost multiplier.
    pub max_expedited_claim_cost_multiplier: SourcedValue<ResourceNumber>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Authored claim cooldown settings.
#[non_exhaustive]
pub struct RecoveryCooldown {
    /// The authored base time.
    pub base_time: SourcedValue<ResourceFloat32>,
    /// The authored scaling price floor.
    pub scaling_price_floor: SourcedValue<ResourceFloat32>,
    #[serde(rename = "aUECperSecond")]
    /// The authored currency rate in aUEC per second.
    pub a_uec_per_second: SourcedValue<ResourceFloat32>,
    /// The authored default loadout multiplier.
    pub default_loadout_multiplier: SourcedValue<ResourceFloat32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Authored standard and expedited delivery settings.
#[non_exhaustive]
pub struct RecoveryDelivery {
    /// The authored base time.
    pub base_time: SourcedValue<ResourceFloat32>,
    /// The authored scaling price floor.
    pub scaling_price_floor: SourcedValue<ResourceFloat32>,
    #[serde(rename = "aUECperSecond")]
    /// The authored currency rate in aUEC per second.
    pub a_uec_per_second: SourcedValue<ResourceFloat32>,
    /// The authored default loadout multiplier.
    pub default_loadout_multiplier: SourcedValue<ResourceFloat32>,
    /// The authored expedited time multiplier.
    pub expedited_time_multiplier: SourcedValue<ResourceFloat32>,
    /// The authored expedited default time multiplier.
    pub expedited_default_time_multiplier: SourcedValue<ResourceFloat32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Authored repair cost multipliers.
#[non_exhaustive]
pub struct RecoveryRepair {
    #[serde(rename = "vehicleChassisRepairCostMultiplier")]
    /// The authored chassis repair cost multiplier.
    pub chassis: SourcedValue<ResourceFloat32>,
    #[serde(rename = "vehicleComponentRepairCostMultiplier")]
    /// The authored component repair cost multiplier.
    pub component: SourcedValue<ResourceFloat32>,
    #[serde(rename = "vehicleRepairMissingItemCostMultiplier")]
    /// The authored missing-item repair cost multiplier.
    pub missing_item: SourcedValue<ResourceFloat32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Authored limit and multipliers for escalating claims.
#[non_exhaustive]
pub struct RecoveryEscalating {
    /// The authored escalating claim chain limit.
    pub escalating_claim_chain_limit: SourcedValue<ResourceNumber>,
    /// The authored escalation timer multiplier.
    pub escalation_timer_multiplier: SourcedValue<ResourceNumber>,
    /// The authored escalating claim cost multiplier.
    pub escalating_claim_cost_multiplier: SourcedValue<ResourceNumber>,
    /// The authored escalating delivery timer multiplier.
    pub escalating_delivery_timer_multiplier: SourcedValue<ResourceNumber>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// The original class filter and cost or time multipliers of an override group.
#[non_exhaustive]
pub struct RecoveryOverride {
    /// The original class references in source order.
    pub classes: SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>,
    /// The authored claim cooldown multiplier.
    pub claim_cooldown_multiplier: SourcedValue<ResourceNumber>,
    /// The authored claim cost multiplier.
    pub claim_cost_multiplier: SourcedValue<ResourceNumber>,
    /// The authored delivery time multiplier.
    pub delivery_time_multiplier: SourcedValue<ResourceNumber>,
    /// The authored max expedite cost multiplier.
    pub max_expedite_cost_multiplier: SourcedValue<ResourceNumber>,
    /// The authored expedited delivery time multiplier.
    pub expedited_delivery_time_multiplier: SourcedValue<ResourceNumber>,
    /// The authored flat minimum delivery timer seconds multiplier.
    pub flat_minimum_delivery_timer_seconds_multiplier: SourcedValue<ResourceNumber>,
    /// The authored escalation timer multiplier.
    pub escalation_timer_multiplier: SourcedValue<ResourceNumber>,
    /// The authored escalating claim cost multiplier.
    pub escalating_claim_cost_multiplier: SourcedValue<ResourceNumber>,
    /// The authored escalating delivery timer multiplier.
    pub escalating_delivery_timer_multiplier: SourcedValue<ResourceNumber>,
}
