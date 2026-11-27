// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_v3::authored_damage::DamageChannels;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;

/// Nominal combat base values keyed to a physical weapon component and fire mode.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct WeaponCombatBase<'source> {
    #[serde(flatten)]
    pub(super) owner: ComponentOwner,
    #[schemars(range(min = 1, max = 1))]
    pub(super) formula_version: u8,
    pub(super) units: CombatUnits,
    pub(super) mode_index: SourcedValue<u32>,
    pub(super) action_source_value_ordinal: Option<SourcedValue<DecimalU64>>,
    pub(super) action_source_type: Option<SourcedValue<&'source str>>,
    pub(super) ammunition_record_index: Option<SourcedValue<u32>>,
    pub(super) model: SourcedValue<CombatModel>,
}

impl<'source> WeaponCombatBase<'source> {
    /// Borrows the exact original owner and component position.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Returns the canonical combat formula version.
    #[must_use]
    pub const fn formula_version(&self) -> u8 {
        self.formula_version
    }

    /// Borrows the units for this mode's combat values.
    #[must_use]
    pub const fn units(&self) -> &CombatUnits {
        &self.units
    }

    /// Borrows the original position in the fire-action array.
    #[must_use]
    pub const fn mode_index(&self) -> &SourcedValue<u32> {
        &self.mode_index
    }

    /// Borrows the original action value position when the slot was readable.
    #[must_use]
    pub const fn action_source_value_ordinal(&self) -> Option<&SourcedValue<DecimalU64>> {
        self.action_source_value_ordinal.as_ref()
    }

    /// Borrows the original action type when the slot was readable.
    #[must_use]
    pub const fn action_source_type(&self) -> Option<&SourcedValue<&'source str>> {
        self.action_source_type.as_ref()
    }

    /// Borrows the selected physical Ammo index when the route was proved.
    #[must_use]
    pub const fn ammunition_record_index(&self) -> Option<&SourcedValue<u32>> {
        self.ammunition_record_index.as_ref()
    }

    /// Borrows the model and the original evidence used to determine it.
    #[must_use]
    pub const fn model(&self) -> &SourcedValue<CombatModel> {
        &self.model
    }
}

/// Units of the canonical combat metrics.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CombatUnits {
    fire_rate: FireRateUnit,
    alpha: AlphaUnit,
    nominal_dps: DpsUnit,
    nominal_flight_distance: DistanceUnit,
}

impl CombatUnits {
    pub(super) const NOMINAL: Self = Self {
        fire_rate: FireRateUnit::DischargesPerMinute,
        alpha: AlphaUnit::DamagePoints,
        nominal_dps: DpsUnit::DamagePointsPerSecond,
        nominal_flight_distance: DistanceUnit::Metres,
    };

    /// Returns the fire-rate unit.
    #[must_use]
    pub const fn fire_rate(&self) -> FireRateUnit {
        self.fire_rate
    }

    /// Returns the alpha unit.
    #[must_use]
    pub const fn alpha(&self) -> AlphaUnit {
        self.alpha
    }

    /// Returns the nominal DPS unit.
    #[must_use]
    pub const fn nominal_dps(&self) -> DpsUnit {
        self.nominal_dps
    }

    /// Returns the nominal distance unit.
    #[must_use]
    pub const fn nominal_flight_distance(&self) -> DistanceUnit {
        self.nominal_flight_distance
    }
}

/// Unit of fire-rate unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum FireRateUnit {
    /// Discharges per minute.
    DischargesPerMinute,
}

/// Unit of alpha unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum AlphaUnit {
    /// Damage points per discharge.
    DamagePoints,
}

/// Unit of nominal DPS unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum DpsUnit {
    /// Damage points per second.
    DamagePointsPerSecond,
}

/// Unit of nominal distance unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum DistanceUnit {
    /// Metres of nominal projectile travel.
    Metres,
}

/// The outcome of classifying and projecting an authored fire mode.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum CombatModel {
    /// The original action and its proved projectile can supply nominal bullet metrics.
    NominalBullet {
        /// Values and their original sources for this mode.
        values: Box<NominalBullet>,
    },
    /// The action has no proved projectile ammunition selection.
    AmmoSelectionUnavailable,
    /// The proved ammunition target has no readable definition.
    AmmoDefinitionUnavailable,
    /// The original action slot has no readable action.
    ActionUnavailable,
    /// The authored action is outside the nominal projectile formula.
    UnsupportedAction,
    /// The authored action fires in bursts.
    Burst,
    /// The authored action fires a beam.
    Beam,
    /// The authored action charges before firing.
    Charge,
    /// The authored action selects behavior by a condition.
    Conditional,
    /// The selected ammunition has no readable projectile body.
    ProjectileUnavailable,
    /// The selected projectile is outside the nominal bullet formula.
    UnsupportedProjectile,
}

/// Authored inputs and computed nominal values one discharge at a time.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct NominalBullet {
    pub(super) fire_rate_rpm: SourcedValue<i32>,
    pub(super) projectiles_per_discharge: SourcedValue<i32>,
    pub(super) launcher_damage_multiplier: SourcedValue<ResourceFloat32>,
    pub(super) nominal_flight_distance: SourcedValue<f64>,
    pub(super) damage: SourcedValue<NominalBulletDamage>,
}

impl NominalBullet {
    /// Borrows the authored discharges per minute.
    #[must_use]
    pub const fn fire_rate_rpm(&self) -> &SourcedValue<i32> {
        &self.fire_rate_rpm
    }

    /// Borrows the authored projectile count per discharge.
    #[must_use]
    pub const fn projectiles_per_discharge(&self) -> &SourcedValue<i32> {
        &self.projectiles_per_discharge
    }

    /// Borrows the authored launcher damage multiplier.
    #[must_use]
    pub const fn launcher_damage_multiplier(&self) -> &SourcedValue<ResourceFloat32> {
        &self.launcher_damage_multiplier
    }

    /// Borrows the computed nominal flight distance in metres.
    #[must_use]
    pub const fn nominal_flight_distance(&self) -> &SourcedValue<f64> {
        &self.nominal_flight_distance
    }

    /// Borrows the computed damage channels.
    #[must_use]
    pub const fn damage(&self) -> &SourcedValue<NominalBulletDamage> {
        &self.damage
    }
}

/// Independent damage channels for one discharge and one second.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct NominalBulletDamage {
    pub(super) alpha: DamageChannels<SourcedValue<f64>>,
    pub(super) nominal_dps: DamageChannels<SourcedValue<f64>>,
}

impl NominalBulletDamage {
    /// Borrows the damage per discharge by channel.
    #[must_use]
    pub const fn alpha(&self) -> &DamageChannels<SourcedValue<f64>> {
        &self.alpha
    }

    /// Borrows the nominal damage per second by channel.
    #[must_use]
    pub const fn nominal_dps(&self) -> &DamageChannels<SourcedValue<f64>> {
        &self.nominal_dps
    }
}
