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

#[path = "projectile_bullet_behavior.rs"]
mod behavior;
#[path = "projectile_bullet_spawns.rs"]
mod spawns;

use super::penetration::AmmoPenetration;
use super::visual::ProjectileVisual;
use behavior::{BulletDamageDrop, BulletElectron, BulletImpulseFalloff, BulletPierceability};
use spawns::BulletSpawns;

/// Authored Bullet damage and gameplay behavior.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BulletProjectile<'source> {
    damage: SourcedValue<AuthoredDamage<'source>>,
    impact_radius: SourcedValue<ResourceFloat32>,
    min_impact_radius: SourcedValue<ResourceFloat32>,
    ignition_chance_override: SourcedValue<ResourceFloat32>,
    keep_alive_on_zero_damage: SourcedValue<bool>,
    hit_type: SourcedValue<&'source str>,
    damage_drop_params: SourcedValue<BulletDamageDrop<'source>>,
    impulse_falloff_params: SourcedValue<BulletImpulseFalloff>,
    pierceability_params: SourcedValue<BulletPierceability>,
    penetration_params: SourcedValue<AmmoPenetration>,
    electron_params: SourcedValue<BulletElectron>,
    visual_params: SourcedValue<ProjectileVisual<'source>>,
    alternate_visual_params: SourcedValue<ProjectileVisual<'source>>,
    #[serde(flatten)]
    spawns: BulletSpawns<'source>,
}

impl<'source> BulletProjectile<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            damage: source.owned("damage", DAMAGE_TYPES, notes, AuthoredDamage::read)?,
            impact_radius: source.single("impactRadius", notes)?,
            min_impact_radius: source.single("minImpactRadius", notes)?,
            ignition_chance_override: source.single("ignitionChanceOverride", notes)?,
            keep_alive_on_zero_damage: source.boolean("keepAliveOnZeroDamage", notes)?,
            hit_type: source.borrowed_string("hitType", notes)?,
            damage_drop_params: source.owned(
                "damageDropParams",
                &["BulletDamageDropParams"],
                notes,
                BulletDamageDrop::read,
            )?,
            impulse_falloff_params: source.owned(
                "impulseFalloffParams",
                &["BulletImpulseFalloffParams"],
                notes,
                BulletImpulseFalloff::read,
            )?,
            pierceability_params: source.inline_class(
                "pierceabilityParams",
                &["BulletPierceabilityParams"],
                notes,
                BulletPierceability::read,
            )?,
            penetration_params: source.inline_class(
                "penetrationParams",
                &["AmmoPenetrationParams"],
                notes,
                AmmoPenetration::read,
            )?,
            electron_params: source.owned(
                "electronParams",
                &["BulletElectronParams"],
                notes,
                BulletElectron::read,
            )?,
            visual_params: source.inline_class(
                "visualParams",
                &["BulletVisualParams"],
                notes,
                ProjectileVisual::read,
            )?,
            alternate_visual_params: source.owned(
                "alternateVisualParams",
                &["BulletVisualParams"],
                notes,
                ProjectileVisual::read,
            )?,
            spawns: BulletSpawns::read(source, notes)?,
        })
    }
}
