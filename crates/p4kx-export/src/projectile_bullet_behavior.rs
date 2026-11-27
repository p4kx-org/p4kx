// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_damage::{AuthoredDamage, DAMAGE_TYPES};
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Three independently authored damage alternatives for falloff.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[expect(
    clippy::struct_field_names,
    reason = "matches the authored falloff field names"
)]
pub(crate) struct BulletDamageDrop<'source> {
    damage_drop_min_distance: SourcedValue<AuthoredDamage<'source>>,
    damage_drop_per_meter: SourcedValue<AuthoredDamage<'source>>,
    damage_drop_min_damage: SourcedValue<AuthoredDamage<'source>>,
}

impl<'source> BulletDamageDrop<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            damage_drop_min_distance: source.owned(
                "damageDropMinDistance",
                DAMAGE_TYPES,
                notes,
                AuthoredDamage::read,
            )?,
            damage_drop_per_meter: source.owned(
                "damageDropPerMeter",
                DAMAGE_TYPES,
                notes,
                AuthoredDamage::read,
            )?,
            damage_drop_min_damage: source.owned(
                "damageDropMinDamage",
                DAMAGE_TYPES,
                notes,
                AuthoredDamage::read,
            )?,
        })
    }
}

/// Original impulse falloff inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BulletImpulseFalloff {
    min_distance: SourcedValue<ResourceFloat32>,
    drop_falloff: SourcedValue<ResourceFloat32>,
    max_falloff: SourcedValue<ResourceFloat32>,
}

impl BulletImpulseFalloff {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            min_distance: source.single("minDistance", notes)?,
            drop_falloff: source.single("dropFalloff", notes)?,
            max_falloff: source.single("maxFalloff", notes)?,
        })
    }
}

/// Original pierceability falloff levels and thickness.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BulletPierceability {
    damage_falloff_level1: SourcedValue<ResourceFloat32>,
    damage_falloff_level2: SourcedValue<ResourceFloat32>,
    damage_falloff_level3: SourcedValue<ResourceFloat32>,
    max_penetration_thickness: SourcedValue<ResourceFloat32>,
}

impl BulletPierceability {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            damage_falloff_level1: source.single("damageFalloffLevel1", notes)?,
            damage_falloff_level2: source.single("damageFalloffLevel2", notes)?,
            damage_falloff_level3: source.single("damageFalloffLevel3", notes)?,
            max_penetration_thickness: source.single("maxPenetrationThickness", notes)?,
        })
    }
}

/// Original electron jump inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BulletElectron {
    residual_charge_multiplier: SourcedValue<ResourceFloat32>,
    maximum_jumps: SourcedValue<i32>,
    jump_range: SourcedValue<ResourceFloat32>,
}

impl BulletElectron {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            residual_charge_multiplier: source.single("residualChargeMultiplier", notes)?,
            maximum_jumps: source.int32("maximumJumps", notes)?,
            jump_range: source.single("jumpRange", notes)?,
        })
    }
}
