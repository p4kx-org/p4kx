// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_damage::DamageVector;
use crate::catalog_v3::authored_effect_resource::{AudioResource, ParticleResource};
use crate::catalog_v3::authored_fields::Observed;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// The original audiovisual and material-effect declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ExplosionEffects {
    max_blur_dist: SourcedValue<ResourceFloat32>,
    effect_scale: SourcedValue<ResourceFloat32>,
    use_random_scale: SourcedValue<bool>,
    effect_scale_min: SourcedValue<ResourceFloat32>,
    effect_scale_max: SourcedValue<ResourceFloat32>,
    effect: SourcedValue<ParticleResource>,
    failed_effect: SourcedValue<ParticleResource>,
    custom_material_effect: SourcedValue<String>,
    sound: SourcedValue<AudioResource>,
    failed_sound: SourcedValue<AudioResource>,
    material_effect_entry: SourcedValue<MaterialEffect>,
    particle_strength: SourcedValue<ResourceFloat32>,
}

impl ExplosionEffects {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            max_blur_dist: source.single("maxblurdist", notes)?,
            effect_scale: source.single("effectScale", notes)?,
            use_random_scale: source.boolean("useRandomScale", notes)?,
            effect_scale_min: source.single("effectScaleMin", notes)?,
            effect_scale_max: source.single("effectScaleMax", notes)?,
            effect: source.owned(
                "effect",
                &["GlobalResourceParticle"],
                notes,
                ParticleResource::read,
            )?,
            failed_effect: source.owned(
                "failedEffect",
                &["GlobalResourceParticle"],
                notes,
                ParticleResource::read,
            )?,
            custom_material_effect: source.string("customMaterialEffect", notes)?,
            sound: source.owned(
                "sound",
                &["GlobalResourceAudio"],
                notes,
                AudioResource::read,
            )?,
            failed_sound: source.owned(
                "failedSound",
                &["GlobalResourceAudio"],
                notes,
                AudioResource::read,
            )?,
            material_effect_entry: source.owned(
                "materialEffectEntry",
                &["MaterialEffectEntry"],
                notes,
                MaterialEffect::read,
            )?,
            particle_strength: source.single("particleStrength", notes)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct MaterialEffect {
    library_name: SourcedValue<String>,
    effect_name: SourcedValue<String>,
}

impl MaterialEffect {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            library_name: source.string("libraryName", notes)?,
            effect_name: source.string("effectName", notes)?,
        })
    }
}

/// The two authored flashbang effect settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Flashbang {
    blind_amount: SourcedValue<ResourceFloat32>,
    flashbang_base_time: SourcedValue<ResourceFloat32>,
}

impl Flashbang {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            blind_amount: source.single("blindAmount", notes)?,
            flashbang_base_time: source.single("flashbangBaseTime", notes)?,
        })
    }
}

/// Original shockwave shape and material fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Shockwave {
    primitive_type: SourcedValue<String>,
    axis: SourcedValue<DamageVector>,
    half_height: SourcedValue<ResourceFloat32>,
    detonation_speed: SourcedValue<ResourceFloat32>,
    peak_pressure: SourcedValue<ResourceFloat32>,
    duration: SourcedValue<ResourceFloat32>,
    duration_scale: SourcedValue<ResourceFloat32>,
    density: SourcedValue<ResourceFloat32>,
    resistance: SourcedValue<ResourceFloat32>,
}

impl Shockwave {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            primitive_type: source.enumeration("primitiveType", notes)?,
            axis: source.owned("axis", &["Vec3"], notes, DamageVector::read)?,
            half_height: source.single("halfHeight", notes)?,
            detonation_speed: source.single("detonationSpeed", notes)?,
            peak_pressure: source.single("peakPressure", notes)?,
            duration: source.single("duration", notes)?,
            duration_scale: source.single("durationScale", notes)?,
            density: source.single("density", notes)?,
            resistance: source.single("resistance", notes)?,
        })
    }
}

/// Stored fragment shape, count and optional surface.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Fragments {
    mass: SourcedValue<ResourceFloat32>,
    radius: SourcedValue<ResourceFloat32>,
    drag: SourcedValue<ResourceFloat32>,
    amount: SourcedValue<i32>,
    surface_type_override: SourcedValue<String>,
}

impl Fragments {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            mass: source.single("mass", notes)?,
            radius: source.single("radius", notes)?,
            drag: source.single("drag", notes)?,
            amount: source.int32("amount", notes)?,
            surface_type_override: source.string("surfaceTypeOverride", notes)?,
        })
    }
}
