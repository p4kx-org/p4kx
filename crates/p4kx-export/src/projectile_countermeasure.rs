// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::ExportAccountingError;
use crate::catalog_v3::authored_effect_resource::{AudioResource, ParticleResource};
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

use super::range::ProjectileRange;

const BASE: &str = "CounterMeasureBaseParams";
const CHAFF: &str = "CounterMeasureChaffParams";
const FLARE: &str = "CounterMeasureFlareParams";

/// Original Countermeasure type and audio trigger inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CountermeasureProjectile {
    type_params: SourcedValue<CountermeasureKind>,
    trail_start_audio_trigger: SourcedValue<AudioResource>,
    trail_stop_audio_trigger: SourcedValue<AudioResource>,
    explosion_start_audio_trigger: SourcedValue<AudioResource>,
    explosion_stop_audio_trigger: SourcedValue<AudioResource>,
}

impl CountermeasureProjectile {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            type_params: source.owned(
                "typeParams",
                &[BASE, CHAFF, FLARE],
                notes,
                CountermeasureKind::read,
            )?,
            trail_start_audio_trigger: source.inline_class(
                "trailStartAudioTrigger",
                &["GlobalResourceAudio"],
                notes,
                AudioResource::read,
            )?,
            trail_stop_audio_trigger: source.inline_class(
                "trailStopAudioTrigger",
                &["GlobalResourceAudio"],
                notes,
                AudioResource::read,
            )?,
            explosion_start_audio_trigger: source.inline_class(
                "explosionStartAudioTrigger",
                &["GlobalResourceAudio"],
                notes,
                AudioResource::read,
            )?,
            explosion_stop_audio_trigger: source.inline_class(
                "explosionStopAudioTrigger",
                &["GlobalResourceAudio"],
                notes,
                AudioResource::read,
            )?,
        })
    }
}

/// The actual countermeasure alternative.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum CountermeasureKind {
    Base,
    Chaff { value: Box<CountermeasureChaff> },
    Flare { value: Box<CountermeasureSignals> },
}

impl CountermeasureKind {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            BASE => Self::Base,
            CHAFF => Self::Chaff {
                value: Box::new(CountermeasureChaff::read(source, notes)?),
            },
            FLARE => Self::Flare {
                value: Box::new(CountermeasureSignals::read(source, notes)?),
            },
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        })
    }
}

/// Authored chaff timing, geometry and particle resources.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CountermeasureChaff {
    volume_spawn_delay: SourcedValue<ResourceFloat32>,
    volume_lifetime: SourcedValue<ResourceFloat32>,
    explosion_particles: SourcedValue<ParticleResource>,
    child_explosion_particles: SourcedValue<ParticleResource>,
    volume_particles: SourcedValue<ParticleResource>,
    radius_range: SourcedValue<ProjectileRange>,
    #[serde(flatten)]
    signals: CountermeasureSignals,
}

impl CountermeasureChaff {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            volume_spawn_delay: source.single("volumeSpawnDelay", notes)?,
            volume_lifetime: source.single("volumeLifetime", notes)?,
            explosion_particles: source.inline_class(
                "explosionParticles",
                &["GlobalResourceParticle"],
                notes,
                ParticleResource::read,
            )?,
            child_explosion_particles: source.inline_class(
                "childExplosionParticles",
                &["GlobalResourceParticle"],
                notes,
                ParticleResource::read,
            )?,
            volume_particles: source.inline_class(
                "volumeParticles",
                &["GlobalResourceParticle"],
                notes,
                ParticleResource::read,
            )?,
            radius_range: source.inline_class(
                "radiusRange",
                &["Range"],
                notes,
                ProjectileRange::read,
            )?,
            signals: CountermeasureSignals::read(source, notes)?,
        })
    }
}

/// Original start and end signature values.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CountermeasureSignals {
    start_infrared: SourcedValue<ResourceFloat32>,
    end_infrared: SourcedValue<ResourceFloat32>,
    start_electromagnetic: SourcedValue<ResourceFloat32>,
    end_electromagnetic: SourcedValue<ResourceFloat32>,
    start_cross_section: SourcedValue<ResourceFloat32>,
    end_cross_section: SourcedValue<ResourceFloat32>,
    start_decibel: SourcedValue<ResourceFloat32>,
    end_decibel: SourcedValue<ResourceFloat32>,
}

impl CountermeasureSignals {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            start_infrared: source.single("StartInfrared", notes)?,
            end_infrared: source.single("EndInfrared", notes)?,
            start_electromagnetic: source.single("StartElectromagnetic", notes)?,
            end_electromagnetic: source.single("EndElectromagnetic", notes)?,
            start_cross_section: source.single("StartCrossSection", notes)?,
            end_cross_section: source.single("EndCrossSection", notes)?,
            start_decibel: source.single("StartDecibel", notes)?,
            end_decibel: source.single("EndDecibel", notes)?,
        })
    }
}
