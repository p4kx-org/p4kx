// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_effect_resource::{AudioResource, Rtpc};
use super::authored_explosion::ExplosionParameters;
use super::authored_fields::Observed;
use crate::CatalogError;
use crate::catalog_v3::catalog_item_components::source_getters;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const AUDIO_TYPE: &[&str] = &["GlobalResourceAudio"];

/// The inherited settings of one authored explosive ordnance component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct OrdnanceFields<'source> {
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
    source_getters! {
        /// Borrows the authored audio params.
        audio_params: SourcedValue<OrdnanceAudio>,
        /// Borrows the authored requires launcher.
        requires_launcher: SourcedValue<bool>,
        /// Borrows the authored enable lifetime.
        enable_lifetime: SourcedValue<bool>,
        /// Borrows the authored max lifetime.
        max_lifetime: SourcedValue<ResourceFloat32>,
        /// Borrows the authored arm time.
        arm_time: SourcedValue<ResourceFloat32>,
        /// Borrows the authored max armable override.
        max_armable_override: SourcedValue<i32>,
        /// Borrows the authored ignite time.
        ignite_time: SourcedValue<ResourceFloat32>,
        /// Borrows the authored collision delay time.
        collision_delay_time: SourcedValue<ResourceFloat32>,
        /// Borrows the authored explosion safety distance.
        explosion_safety_distance: SourcedValue<ResourceFloat32>,
        /// Borrows the authored projectile proximity.
        projectile_proximity: SourcedValue<ResourceFloat32>,
        /// Borrows the authored explosion settings and their source evidence.
        explosion_params: SourcedValue<ExplosionParameters<'source>>,
        /// Borrows the authored cluster params.
        cluster_params: SourcedValue<OrdnanceCluster>,
        /// Borrows the authored emissions params.
        emissions_params: SourcedValue<Vec<SourcedValue<OrdnanceEmission>>>,
    }

    pub(super) const fn projected(
        &self,
    ) -> [(&'static str, p4kx_extraction::SourceValueState); 13] {
        [
            ("audioParams", self.audio_params.state()),
            ("requiresLauncher", self.requires_launcher.state()),
            ("enableLifetime", self.enable_lifetime.state()),
            ("maxLifetime", self.max_lifetime.state()),
            ("armTime", self.arm_time.state()),
            ("maxArmableOverride", self.max_armable_override.state()),
            ("igniteTime", self.ignite_time.state()),
            ("collisionDelayTime", self.collision_delay_time.state()),
            (
                "explosionSafetyDistance",
                self.explosion_safety_distance.state(),
            ),
            ("projectileProximity", self.projectile_proximity.state()),
            ("explosionParams", self.explosion_params.state()),
            ("clusterParams", self.cluster_params.state()),
            ("emissionsParams", self.emissions_params.state()),
        ]
    }

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
pub struct BombFields<'source> {
    #[serde(flatten)]
    ordnance: OrdnanceFields<'source>,
    drag_area_radius: SourcedValue<ResourceFloat32>,
    centre_of_pressure_offset_y: SourcedValue<ResourceFloat32>,
    maximum_drop_angle_from_flat_flight: SourcedValue<ResourceFloat32>,
    altitude_rtpc: SourcedValue<Rtpc>,
    max_altitude_for_audio_rtpc: SourcedValue<ResourceFloat32>,
}

impl<'source> BombFields<'source> {
    source_getters! {
        /// Borrows the inherited ordnance settings.
        ordnance: OrdnanceFields<'source>,
        /// Borrows the authored drag area radius.
        drag_area_radius: SourcedValue<ResourceFloat32>,
        /// Borrows the authored centre of pressure offset Y.
        centre_of_pressure_offset_y: SourcedValue<ResourceFloat32>,
        /// Borrows the authored maximum drop angle from flat flight.
        maximum_drop_angle_from_flat_flight: SourcedValue<ResourceFloat32>,
        /// Borrows the authored altitude RTPC.
        altitude_rtpc: SourcedValue<Rtpc>,
        /// Borrows the authored max altitude for audio RTPC.
        max_altitude_for_audio_rtpc: SourcedValue<ResourceFloat32>,
    }

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

/// Original ordnance audio triggers.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct OrdnanceAudio {
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
    source_getters! {
        /// Borrows the authored release.
        release: SourcedValue<AudioResource>,
        /// Borrows the authored engine ignite.
        engine_ignite: SourcedValue<AudioResource>,
        /// Borrows the authored trail.
        trail: SourcedValue<AudioResource>,
        /// Borrows the authored trail stop.
        trail_stop: SourcedValue<AudioResource>,
    }

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

/// Authored cluster munition separation settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct OrdnanceCluster {
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
    source_getters! {
        /// Borrows the authored launch detach time.
        launch_detach_time: SourcedValue<ResourceFloat32>,
        /// Borrows the authored impact detach time.
        impact_detach_time: SourcedValue<ResourceFloat32>,
        /// Borrows the authored detach angle relative to gravity.
        detach_angle_relative_to_gravity: SourcedValue<bool>,
        /// Borrows the authored detach angle initial.
        detach_angle_initial: SourcedValue<ResourceFloat32>,
        /// Borrows the authored detach angle increment.
        detach_angle_increment: SourcedValue<ResourceFloat32>,
        /// Borrows the authored detach angle reset count.
        detach_angle_reset_count: SourcedValue<i32>,
        /// Borrows the authored detach delay.
        detach_delay: SourcedValue<ResourceFloat32>,
        /// Borrows the authored detach speed.
        detach_speed: SourcedValue<ResourceFloat32>,
    }

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

/// One authored ordnance emission envelope.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct OrdnanceEmission {
    active: SourcedValue<bool>,
    min_value: SourcedValue<ResourceFloat32>,
    max_value: SourcedValue<ResourceFloat32>,
    rise_rate: SourcedValue<ResourceFloat32>,
    decay_rate: SourcedValue<ResourceFloat32>,
}

impl OrdnanceEmission {
    source_getters! {
        /// Borrows the authored active.
        active: SourcedValue<bool>,
        /// Borrows the authored min value.
        min_value: SourcedValue<ResourceFloat32>,
        /// Borrows the authored max value.
        max_value: SourcedValue<ResourceFloat32>,
        /// Borrows the authored rise rate.
        rise_rate: SourcedValue<ResourceFloat32>,
        /// Borrows the authored decay rate.
        decay_rate: SourcedValue<ResourceFloat32>,
    }

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
