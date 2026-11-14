// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::ExportAccountingError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_damage::{AuthoredDamage, DAMAGE_TYPES};
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

use super::super::range::ProjectileRange;

const BASE: &str = "HitBehaviorBase";
const DAMAGE_OVER_TIME: &str = "HitBehaviorDamageOverTime";

/// Authored links to additional projectiles and ordered on-hit behavior.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BulletSpawns<'source> {
    additional_projectiles_params: SourcedValue<AdditionalProjectiles<'source>>,
    hit_behaviors: SourcedValue<Vec<SourcedValue<ProjectileHitBehavior<'source>>>>,
}

impl<'source> BulletSpawns<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            additional_projectiles_params: source.owned(
                "additionalProjectilesParams",
                &["AdditionalProjectilesParams"],
                notes,
                AdditionalProjectiles::read,
            )?,
            hit_behaviors: source.array(
                "hitBehaviors",
                &[BASE, DAMAGE_OVER_TIME],
                notes,
                ProjectileHitBehavior::read,
            )?,
        })
    }
}

/// The actual on-hit behavior type and its raw inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum ProjectileHitBehavior<'source> {
    Base,
    DamageOverTime { value: Box<DamageOverTime<'source>> },
}

impl<'source> ProjectileHitBehavior<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            BASE => Self::Base,
            DAMAGE_OVER_TIME => Self::DamageOverTime {
                value: Box::new(DamageOverTime::read(source, notes)?),
            },
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

/// An original additional-projectile definition link and launch inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AdditionalProjectiles<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    ammo_record: SourcedValue<Box<SourceRecordReference<'source>>>,
    amount: SourcedValue<i32>,
    spread_degrees: SourcedValue<ResourceFloat32>,
    lifetime_range: SourcedValue<ProjectileRange>,
}

impl<'source> AdditionalProjectiles<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            ammo_record: source.record_link("ammoRecord", notes)?,
            amount: source.int32("amount", notes)?,
            spread_degrees: source.single("spreadDegrees", notes)?,
            lifetime_range: source.owned(
                "lifetimeRange",
                &["Range"],
                notes,
                ProjectileRange::read,
            )?,
        })
    }
}

/// Original damage-over-time behavior inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DamageOverTime<'source> {
    damage_per_second: SourcedValue<AuthoredDamage<'source>>,
    duration: SourcedValue<ResourceFloat32>,
    damage_period: SourcedValue<ResourceFloat32>,
    ignore_shields: SourcedValue<bool>,
    apply_to_health_pool: SourcedValue<bool>,
}

impl<'source> DamageOverTime<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            damage_per_second: source.owned(
                "damagePerSecond",
                DAMAGE_TYPES,
                notes,
                AuthoredDamage::read,
            )?,
            duration: source.single("duration", notes)?,
            damage_period: source.single("damagePeriod", notes)?,
            ignore_shields: source.boolean("ignoreShields", notes)?,
            apply_to_health_pool: source.boolean("applyToHealthPool", notes)?,
        })
    }
}
