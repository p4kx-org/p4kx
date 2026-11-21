// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_rgb::AuthoredRgb;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceText;

/// Original light pool fields of the Ammo definition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoLight<'source> {
    flare_name: SourcedValue<SourceText<'source>>,
    flare_scale: SourcedValue<ResourceFloat32>,
    radius: SourcedValue<ResourceFloat32>,
    diffuse_color: SourcedValue<AuthoredRgb>,
    diffuse_multiplier: SourcedValue<ResourceFloat32>,
    specular_multiplier: SourcedValue<ResourceFloat32>,
    attenuation_bulb_size: SourcedValue<ResourceFloat32>,
    anim_speed: SourcedValue<ResourceFloat32>,
    ramp_time: SourcedValue<ResourceFloat32>,
    fake: SourcedValue<bool>,
    auto_clip: SourcedValue<bool>,
    style: SourcedValue<u8>,
    anim_phase: SourcedValue<u8>,
    flare_lens_optics_frustum_angle: SourcedValue<u8>,
}

impl<'source> AmmoLight<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            flare_name: source
                .borrowed_string("flareName", notes)?
                .map(|text| SourceText::read("PooledLightData", "flareName", text)),
            flare_scale: source.single("flareScale", notes)?,
            radius: source.single("radius", notes)?,
            diffuse_color: source.inline_class(
                "diffuseColor",
                &["RGB"],
                notes,
                AuthoredRgb::read,
            )?,
            diffuse_multiplier: source.single("diffuseMultiplier", notes)?,
            specular_multiplier: source.single("specularMultiplier", notes)?,
            attenuation_bulb_size: source.single("attenuationBulbSize", notes)?,
            anim_speed: source.single("animSpeed", notes)?,
            ramp_time: source.single("rampTime", notes)?,
            fake: source.boolean("fake", notes)?,
            auto_clip: source.boolean("autoClip", notes)?,
            style: source.uint8("style", notes)?,
            anim_phase: source.uint8("animPhase", notes)?,
            flare_lens_optics_frustum_angle: source.uint8("flareLensOpticsFrustumAngle", notes)?,
        })
    }
}
