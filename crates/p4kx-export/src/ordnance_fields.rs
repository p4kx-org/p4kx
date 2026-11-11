// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_effect_resource::{AudioResource, Rtpc};
use super::authored_explosion::ExplosionParameters;
use super::authored_fields::Observed;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const AUDIO_TYPE: &[&str] = &["GlobalResourceAudio"];

/// The inherited settings of one authored explosive ordnance component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct OrdnanceFields<'source> {
    audio_params: SourcedValue<OrdnanceAudio>,
    requires_launcher: SourcedValue<bool>,
    enable_lifetime: SourcedValue<bool>,
    max_lifetime: SourcedValue<ResourceFloat32>,
    arm_time: SourcedValue<ResourceFloat32>,
    max_armable_override: SourcedValue<i32>,
    ignite_time: SourcedValue<ResourceFloat32>,
    collision_delay_time: SourcedValue<ResourceFloat32>,
    explosion_safety_distance: SourcedValue<ResourceFloat32>,
    projectile_proximity: SourcedValue<ResourceFloat32>,
    explosion_params: SourcedValue<ExplosionParameters<'source>>,
    cluster_params: SourcedValue<OrdnanceCluster>,
    emissions_params: SourcedValue<Vec<SourcedValue<OrdnanceEmission>>>,
}

impl<'source> OrdnanceFields<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            audio_params: source.inline_class(
                "audioParams",
                &["SOrdnanceAudioParams"],
                notes,
                OrdnanceAudio::read,
            )?,
            requires_launcher: source.boolean("requiresLauncher", notes)?,
            enable_lifetime: source.boolean("enableLifetime", notes)?,
            max_lifetime: source.single("maxLifetime", notes)?,
            arm_time: source.single("armTime", notes)?,
            max_armable_override: source.int32("maxArmableOverride", notes)?,
            ignite_time: source.single("igniteTime", notes)?,
            collision_delay_time: source.single("collisionDelayTime", notes)?,
            explosion_safety_distance: source.single("explosionSafetyDistance", notes)?,
            projectile_proximity: source.single("projectileProximity", notes)?,
            explosion_params: source.inline_class(
                "explosionParams",
                &["ExplosionParams"],
                notes,
                ExplosionParameters::read,
            )?,
            cluster_params: source.strong_pointer(
                "clusterParams",
                &["SOrdnanceClusterParams"],
                notes,
                OrdnanceCluster::read,
            )?,
            emissions_params: source.class_array(
                "emissionsParams",
                &["SOrdnanceEmissionsParams"],
                notes,
                OrdnanceEmission::read,
            )?,
        })
    }
}

/// The five bomb-specific fields beside the shared ordnance settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BombFields<'source> {
    #[serde(flatten)]
    ordnance: OrdnanceFields<'source>,
    drag_area_radius: SourcedValue<ResourceFloat32>,
    centre_of_pressure_offset_y: SourcedValue<ResourceFloat32>,
    maximum_drop_angle_from_flat_flight: SourcedValue<ResourceFloat32>,
    altitude_rtpc: SourcedValue<Rtpc>,
    max_altitude_for_audio_rtpc: SourcedValue<ResourceFloat32>,
}

impl<'source> BombFields<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            ordnance: OrdnanceFields::read(source, notes)?,
            drag_area_radius: source.single("dragAreaRadius", notes)?,
            centre_of_pressure_offset_y: source.single("centreOfPressureOffsetY", notes)?,
            maximum_drop_angle_from_flat_flight: source
                .single("maximumDropAngleFromFlatFlight", notes)?,
            altitude_rtpc: source.inline_class(
                "altitudeRtpc",
                &["AudioRtpc"],
                notes,
                Rtpc::read,
            )?,
            max_altitude_for_audio_rtpc: source.single("maxAltitudeForAudioRtpc", notes)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct OrdnanceAudio {
    #[serde(rename = "releaseAudioTrigger")]
    release: SourcedValue<AudioResource>,
    #[serde(rename = "engineIgniteAudioTrigger")]
    engine_ignite: SourcedValue<AudioResource>,
    #[serde(rename = "trailAudioTrigger")]
    trail: SourcedValue<AudioResource>,
    #[serde(rename = "trailStopAudioTrigger")]
    trail_stop: SourcedValue<AudioResource>,
}

impl OrdnanceAudio {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            release: source.inline_class(
                "releaseAudioTrigger",
                AUDIO_TYPE,
                notes,
                AudioResource::read,
            )?,
            engine_ignite: source.inline_class(
                "engineIgniteAudioTrigger",
                AUDIO_TYPE,
                notes,
                AudioResource::read,
            )?,
            trail: source.inline_class(
                "trailAudioTrigger",
                AUDIO_TYPE,
                notes,
                AudioResource::read,
            )?,
            trail_stop: source.inline_class(
                "trailStopAudioTrigger",
                AUDIO_TYPE,
                notes,
                AudioResource::read,
            )?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct OrdnanceCluster {
    launch_detach_time: SourcedValue<ResourceFloat32>,
    impact_detach_time: SourcedValue<ResourceFloat32>,
    detach_angle_relative_to_gravity: SourcedValue<bool>,
    detach_angle_initial: SourcedValue<ResourceFloat32>,
    detach_angle_increment: SourcedValue<ResourceFloat32>,
    detach_angle_reset_count: SourcedValue<i32>,
    detach_delay: SourcedValue<ResourceFloat32>,
    detach_speed: SourcedValue<ResourceFloat32>,
}

impl OrdnanceCluster {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            launch_detach_time: source.single("launchDetachTime", notes)?,
            impact_detach_time: source.single("impactDetachTime", notes)?,
            detach_angle_relative_to_gravity: source
                .boolean("detachAngleRelativeToGravity", notes)?,
            detach_angle_initial: source.single("detachAngleInitial", notes)?,
            detach_angle_increment: source.single("detachAngleIncrement", notes)?,
            detach_angle_reset_count: source.int32("detachAngleResetCount", notes)?,
            detach_delay: source.single("detachDelay", notes)?,
            detach_speed: source.single("detachSpeed", notes)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct OrdnanceEmission {
    active: SourcedValue<bool>,
    min_value: SourcedValue<ResourceFloat32>,
    max_value: SourcedValue<ResourceFloat32>,
    rise_rate: SourcedValue<ResourceFloat32>,
    decay_rate: SourcedValue<ResourceFloat32>,
}

impl OrdnanceEmission {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            active: source.boolean("active", notes)?,
            min_value: source.single("minValue", notes)?,
            max_value: source.single("maxValue", notes)?,
            rise_rate: source.single("riseRate", notes)?,
            decay_rate: source.single("decayRate", notes)?,
        })
    }
}
