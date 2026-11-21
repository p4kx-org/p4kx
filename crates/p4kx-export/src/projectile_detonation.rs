// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_explosion::ExplosionParameters;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

/// Authored activation and explosion fields from the projectile base.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ProjectileDetonation<'source> {
    arm_time: SourcedValue<ResourceFloat32>,
    safe_distance: SourcedValue<ResourceFloat32>,
    destruct_delay: SourcedValue<ResourceFloat32>,
    explode_on_impact: SourcedValue<bool>,
    explode_on_final_impact: SourcedValue<bool>,
    explode_on_expire: SourcedValue<bool>,
    explode_on_target_range: SourcedValue<bool>,
    explosion_params: SourcedValue<Box<ExplosionParameters<'source>>>,
    spawned_entity_params: SourcedValue<ProjectileSpawnedEntity<'source>>,
}

impl<'source> ProjectileDetonation<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            arm_time: source.single("armTime", notes)?,
            safe_distance: source.single("safeDistance", notes)?,
            destruct_delay: source.single("destructDelay", notes)?,
            explode_on_impact: source.boolean("explodeOnImpact", notes)?,
            explode_on_final_impact: source.boolean("explodeOnFinalImpact", notes)?,
            explode_on_expire: source.boolean("explodeOnExpire", notes)?,
            explode_on_target_range: source.boolean("explodeOnTargetRange", notes)?,
            explosion_params: source.inline_class(
                "explosionParams",
                &["ExplosionParams"],
                notes,
                |source, notes| ExplosionParameters::read(source, notes).map(Box::new),
            )?,
            spawned_entity_params: source.inline_class(
                "spawnedEntityParams",
                &["ProjectileSpawnedEntityParams"],
                notes,
                ProjectileSpawnedEntity::read,
            )?,
        })
    }
}

/// Original collision-spawn Record link and one-shot flag.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ProjectileSpawnedEntity<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    entity_to_spawn_on_collision: SourcedValue<Box<SourceRecordReference<'source>>>,
    one_shot: SourcedValue<bool>,
}

impl<'source> ProjectileSpawnedEntity<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            entity_to_spawn_on_collision: source.record_link("entityToSpawnOnCollision", notes)?,
            one_shot: source.boolean("oneShot", notes)?,
        })
    }
}

/// Original proximity trigger fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ProjectileProximity {
    arm_time: SourcedValue<ResourceFloat32>,
    safe_distance: SourcedValue<ResourceFloat32>,
    radius: SourcedValue<ResourceFloat32>,
}

impl ProjectileProximity {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            arm_time: source.single("armTime", notes)?,
            safe_distance: source.single("safeDistance", notes)?,
            radius: source.single("radius", notes)?,
        })
    }
}
