// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use crate::guid_text::CatalogGuid;

/// A mining laser, raw facts of `SEntityComponentMiningLaserParams` (ADR-0262).
///
/// The beams are fire modes of `weapon.fireActions`: their `hitType`, `fullDamageRange`,
/// `zeroDamageRange` and `damagePerSecond`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MiningLaserStats {
    /// `miningLaserModifiers.isOptimalRateGood`.
    pub(crate) is_optimal_rate_good: Option<bool>,
    /// `throttleLerpSpeed`.
    pub(crate) throttle_lerp_speed: Option<f32>,
    /// `throttleMinimum`, the lowest throttle as a fraction of 1.
    pub(crate) throttle_minimum: Option<f32>,
    /// `usesPowerThrottle`.
    pub(crate) uses_power_throttle: Option<bool>,
    /// The `MiningLaserGlobalParams` Record `globalParams` links to.
    pub(crate) global_params_guid: Option<CatalogGuid>,
    /// `miningLaserModifiers`; `null` without them.
    pub(crate) modifiers: Option<MiningModifierValues>,
    /// `filterParams.filterModifier.value`.
    pub(crate) filter_modifier: Option<f32>,
}

/// The modifiers of a `MiningLaserModifiers` struct, each the `value` of a
/// `FloatModifierMultiplicative` as stored, in percent; `null` for a modifier the struct leaves
/// unset.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MiningModifierValues {
    /// `laserInstability`.
    pub(crate) instability: Option<f32>,
    /// `optimalChargeWindowSizeModifier`.
    pub(crate) optimal_charge_window_size: Option<f32>,
    /// `resistanceModifier`.
    pub(crate) resistance: Option<f32>,
    /// `shatterdamageModifier`.
    pub(crate) shatter_damage: Option<f32>,
    /// `clusterFactorModifier`.
    pub(crate) cluster_factor: Option<f32>,
    /// `optimalChargeWindowRateModifier`.
    pub(crate) optimal_charge_window_rate: Option<f32>,
    /// `catastrophicChargeWindowRateModifier`.
    pub(crate) catastrophic_charge_window_rate: Option<f32>,
}

/// A mining module or gadget, raw facts of the `EntityComponentAttachableModifierParams` that
/// holds a mining modifier (ADR-0262).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MiningModifierStats<'a> {
    /// `activationMethod`, e.g. `ActivateOnAttach` for a passive module and `ActivateOnDemand`
    /// for an active module or a gadget. A closed set the game may extend; the schema keeps a
    /// string.
    pub(crate) activation_method: Option<&'a str>,
    /// `charges`; -1 for a gadget that is not used up.
    pub(crate) charges: Option<i64>,
    /// `canInterrupt`.
    pub(crate) can_interrupt: Option<bool>,
    /// `isInterruptible`.
    pub(crate) is_interruptible: Option<bool>,
    /// The first `ItemMiningModifierParams`: the change to the laser; `null` without one.
    pub(crate) laser: Option<MiningEffect>,
    /// The first `ItemMineableRockModifierParams`: the change to the rock; `null` without one.
    pub(crate) rock: Option<MiningEffect>,
    /// The first `MiningFilterItemModifierParams`; `null` without one.
    pub(crate) filter: Option<FilterEffect>,
    /// The `ItemWeaponModifiersParams` in stored order: the multiplier per fire action.
    pub(crate) fire_action_modifiers: Vec<FireActionModifier>,
}

/// The mining modifiers one `ItemMiningModifierParams` or `ItemMineableRockModifierParams`
/// applies.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MiningEffect {
    /// `modifierLifetime.lifetime`, in seconds; `null` for a modifier that lasts.
    pub(crate) lifetime: Option<f32>,
    /// `MiningLaserModifier.isOptimalRateGood`.
    pub(crate) is_optimal_rate_good: Option<bool>,
    /// `MiningLaserModifier`; `null` without it.
    pub(crate) modifiers: Option<MiningModifierValues>,
}

/// The filter change of a `MiningFilterItemModifierParams`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilterEffect {
    /// `modifierLifetime.lifetime`, in seconds; `null` for a modifier that lasts.
    pub(crate) lifetime: Option<f32>,
    /// `filterParams.filterModifier.value`.
    pub(crate) filter_modifier: Option<f32>,
}

/// The damage multiplier an `ItemWeaponModifiersParams` applies to one fire action of the laser.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FireActionModifier {
    /// `fireActionIndex`, the index into the laser's `fireActions` as stored.
    pub(crate) fire_action_index: Option<i64>,
    /// `modifierLifetime.lifetime`, in seconds; `null` for a modifier that lasts.
    pub(crate) lifetime: Option<f32>,
    /// `weaponModifier.weaponStats.damageMultiplier`.
    pub(crate) damage_multiplier: Option<f32>,
}
