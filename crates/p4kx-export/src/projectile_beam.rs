// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_effect_resource::ParticleResource;
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

/// Original ordered Tachyon beam effect inputs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ProjectileBeamEffect<'source> {
    helper: SourcedValue<&'source str>,
    kill_particles_on_deactivate: SourcedValue<bool>,
    particle_effect: SourcedValue<ParticleResource>,
    third_person_particle_effect: SourcedValue<ParticleResource>,
    start_point_strength: SourcedValue<ResourceFloat32>,
    end_point_strength: SourcedValue<ResourceFloat32>,
    use_multi_spline: SourcedValue<bool>,
    multi_spline_offset: SourcedValue<ResourceFloat32>,
    modifier_source: SourcedValue<String>,
    tint_color: SourcedValue<ProjectileTint>,
    particle_audio_params: SourcedValue<Vec<SourcedValue<ProjectileParticleAudio<'source>>>>,
}

impl<'source> ProjectileBeamEffect<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            helper: source.borrowed_string("helper", notes)?,
            kill_particles_on_deactivate: source.boolean("killParticlesOnDeactivate", notes)?,
            particle_effect: source.inline_class(
                "particleEffect",
                &["GlobalResourceParticle"],
                notes,
                ParticleResource::read,
            )?,
            third_person_particle_effect: source.inline_class(
                "thirdPersonParticleEffect",
                &["GlobalResourceParticle"],
                notes,
                ParticleResource::read,
            )?,
            start_point_strength: source.single("startPointStrength", notes)?,
            end_point_strength: source.single("endPointStrength", notes)?,
            use_multi_spline: source.boolean("useMultiSpline", notes)?,
            multi_spline_offset: source.single("multiSplineOffset", notes)?,
            modifier_source: source.enumeration("modifierSource", notes)?,
            tint_color: source.owned("tintColor", &["RGB"], notes, ProjectileTint::read)?,
            particle_audio_params: source.class_array(
                "particleAudioParams",
                &["SBeamParticleAudioParams"],
                notes,
                ProjectileParticleAudio::read,
            )?,
        })
    }
}

/// The original three color channels.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ProjectileTint {
    r: SourcedValue<ResourceFloat32>,
    g: SourcedValue<ResourceFloat32>,
    b: SourcedValue<ResourceFloat32>,
}

impl ProjectileTint {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            r: source.single("r", notes)?,
            g: source.single("g", notes)?,
            b: source.single("b", notes)?,
        })
    }
}

/// An authored subparticle path and audio Record link.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ProjectileParticleAudio<'source> {
    sub_particle_path: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    gpu_particle_audio: SourcedValue<Box<SourceRecordReference<'source>>>,
}

impl<'source> ProjectileParticleAudio<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            sub_particle_path: source.borrowed_string("subParticlePath", notes)?,
            gpu_particle_audio: source.record_link("gpuParticleAudio", notes)?,
        })
    }
}
