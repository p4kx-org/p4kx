// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_projectile::AuthoredProjectile;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_effect_resource::{AudioResource, ParticleResource};
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

#[path = "ammo_conversion.rs"]
mod conversion;
#[path = "ammo_physics.rs"]
mod physics;

use conversion::AmmoConversion;
use physics::AmmoPhysics;

/// Gameplay and audio fields of one admitted Ammo definition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AuthoredAmmoInputs<'source> {
    spawn_type: SourcedValue<String>,
    size: SourcedValue<u8>,
    ammo_category: SourcedValue<String>,
    ui_icon_type: SourcedValue<&'source str>,
    hit_points: SourcedValue<ResourceFloat32>,
    lifetime: SourcedValue<ResourceFloat32>,
    showtime: SourcedValue<ResourceFloat32>,
    inherit_velocity: SourcedValue<ResourceFloat32>,
    bullet_type: SourcedValue<i32>,
    speed: SourcedValue<ResourceFloat32>,
    impulse_scale: SourcedValue<ResourceFloat32>,
    no_bullet_hits: SourcedValue<bool>,
    quiet_removal: SourcedValue<bool>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    resource_type: SourcedValue<Box<SourceRecordReference<'source>>>,
    conversion_rate: SourcedValue<AmmoConversion>,
    use_in_convergence: SourcedValue<bool>,
    physics_controller_params: SourcedValue<Box<AmmoPhysics<'source>>>,
    projectile_params: SourcedValue<AuthoredProjectile<'source>>,
    #[serde(flatten)]
    audio: AmmoAudio,
}

impl<'source> AuthoredAmmoInputs<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            spawn_type: source.enumeration("spawnType", notes)?,
            size: source.uint8("size", notes)?,
            ammo_category: source.enumeration("ammoCategory", notes)?,
            ui_icon_type: source.borrowed_string("UIIconType", notes)?,
            hit_points: source.single("hitPoints", notes)?,
            lifetime: source.single("lifetime", notes)?,
            showtime: source.single("showtime", notes)?,
            inherit_velocity: source.single("inheritVelocity", notes)?,
            bullet_type: source.int32("bulletType", notes)?,
            speed: source.single("speed", notes)?,
            impulse_scale: source.single("impulseScale", notes)?,
            no_bullet_hits: source.boolean("noBulletHits", notes)?,
            quiet_removal: source.boolean("quietRemoval", notes)?,
            resource_type: source.record_link("resourceType", notes)?,
            conversion_rate: source.owned(
                "conversionRate",
                AmmoConversion::TYPES,
                notes,
                AmmoConversion::read,
            )?,
            use_in_convergence: source.boolean("useInConvergence", notes)?,
            physics_controller_params: source.owned(
                "physicsControllerParams",
                &["SEntityPhysicsControllerParams"],
                notes,
                |source, notes| AmmoPhysics::read(source, notes).map(Box::new),
            )?,
            projectile_params: AuthoredProjectile::read_ammo(source, notes)?,
            audio: AmmoAudio::read(source, notes)?,
        })
    }
}

/// Original audio, stimulus and trail resource fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoAudio {
    whiz_sound: SourcedValue<AudioResource>,
    whiz_sound_distance: SourcedValue<ResourceFloat32>,
    impact_ai_stimulus_distance: SourcedValue<ResourceFloat32>,
    ricochet_sound: SourcedValue<AudioResource>,
    projectile_loop_start: SourcedValue<AudioResource>,
    projectile_loop_stop: SourcedValue<AudioResource>,
    shots_per_audio_loop: SourcedValue<u32>,
    trail_particles: SourcedValue<ParticleResource>,
}

impl AmmoAudio {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            whiz_sound: source.inline_class(
                "whizSound",
                &["GlobalResourceAudio"],
                notes,
                AudioResource::read,
            )?,
            whiz_sound_distance: source.single("whizSoundDistance", notes)?,
            impact_ai_stimulus_distance: source.single("impactAIStimulusDistance", notes)?,
            ricochet_sound: source.inline_class(
                "ricochetSound",
                &["GlobalResourceAudio"],
                notes,
                AudioResource::read,
            )?,
            projectile_loop_start: source.inline_class(
                "projectileLoopStart",
                &["GlobalResourceAudio"],
                notes,
                AudioResource::read,
            )?,
            projectile_loop_stop: source.inline_class(
                "projectileLoopStop",
                &["GlobalResourceAudio"],
                notes,
                AudioResource::read,
            )?,
            shots_per_audio_loop: source.uint32("shotsPerAudioLoop", notes)?,
            trail_particles: source.inline_class(
                "trailParticles",
                &["GlobalResourceParticle"],
                notes,
                ParticleResource::read,
            )?,
        })
    }
}

#[cfg(test)]
#[path = "authored_ammo_inputs_tests.rs"]
mod tests;
