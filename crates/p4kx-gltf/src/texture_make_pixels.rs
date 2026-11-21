// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::DdsTransfer;

use crate::surface_finish::SurfaceFinish;
use crate::texture_loader::LoadedTexture;

use super::{Made, Outcome, encode, load};
use crate::texture_cache::Decoded;
use crate::texture_origins::{TextureComponents, TexturePaletteUse};
use crate::texture_pixels::{
    MetallicRoughness, Smoothness, base_colour, metallic_roughness, normal_map, opacity_colour,
};
use crate::texture_plan::{Failure, Job, Key};
use crate::texture_report::TextureRole;

pub(super) fn make_base(
    job: &Job,
    keep_alpha: bool,
    decoded: &Decoded,
    outcome: &mut Outcome,
    failures: &mut Vec<Failure>,
) -> Made {
    let diffuse = load(
        decoded,
        TextureRole::Diffuse,
        job.diffuse.as_ref(),
        failures,
    )?;
    let specular = load(
        decoded,
        TextureRole::Specular,
        job.specular.as_ref(),
        failures,
    );
    outcome.tinted = specular.is_some();
    if job.diffuse.as_ref().is_some_and(|source| source.opacity) {
        if job.tints.decal.is_some() {
            outcome.palette_use = TexturePaletteUse::Decal;
        }
        outcome
            .texture_sources
            .push(diffuse.contribution(TextureComponents::Color));
        return Some(
            opacity_colour(&diffuse.image, job.tints.decal)
                .map_err(|error| error.to_string())
                .and_then(|image| encode(&image, DdsTransfer::Srgb)),
        );
    }
    if specular.is_some() {
        outcome.palette_use = TexturePaletteUse::DiffuseAndSpecular;
    }
    let components = if keep_alpha && diffuse.uses_opacity() {
        TextureComponents::ColorAndAlpha
    } else {
        TextureComponents::Color
    };
    outcome
        .texture_sources
        .push(diffuse.colour_contribution(components));
    outcome.texture_sources.extend(
        specular
            .iter()
            .map(|texture| texture.colour_contribution(TextureComponents::Color)),
    );
    Some(
        base_colour(&diffuse, specular.as_deref(), job.tints, keep_alpha)
            .map_err(|error| error.to_string())
            .and_then(|image| encode(&image, DdsTransfer::Srgb)),
    )
}

pub(super) fn make_normal(
    job: &Job,
    decoded: &Decoded,
    outcome: &mut Outcome,
    failures: &mut Vec<Failure>,
) -> Made {
    let normal = load(decoded, TextureRole::Normal, job.normal.as_ref(), failures)?;
    outcome
        .texture_sources
        .push(normal.contribution(TextureComponents::Color));
    Some(
        normal_map(&normal.image)
            .map_err(|error| error.to_string())
            .and_then(|image| encode(&image, DdsTransfer::Linear)),
    )
}

pub(super) fn make_packed(
    job: &Job,
    decoded: &Decoded,
    outcome: &mut Outcome,
    failures: &mut Vec<Failure>,
) -> Made {
    let normal = load(decoded, TextureRole::Normal, job.normal.as_ref(), failures)
        .filter(|normal| normal.has_alpha);
    let explicit = if normal.is_none() {
        job.explicit_gloss
            .iter()
            .find_map(|source| load(decoded, TextureRole::Gloss, Some(source), failures))
    } else {
        None
    };
    let specular = load(
        decoded,
        TextureRole::Specular,
        job.specular.as_ref(),
        failures,
    );
    let finish = match job.key {
        Key::MetallicRoughness { finish, .. } => finish,
        _ => None,
    };
    let inputs = PackedInputs {
        normal: normal.as_deref(),
        gloss: explicit.as_deref(),
        specular: specular.as_deref(),
    };
    match metallic_roughness(
        inputs.smoothness(finish),
        inputs.specular,
        job.tints.specular,
        finish,
    ) {
        Ok(Some(packed)) => {
            inputs.observe(outcome, finish, &packed);
            Some(encode(&packed.image, DdsTransfer::Linear))
        }
        Ok(None) => None,
        Err(error) => Some(Err(error.to_string())),
    }
}

struct PackedInputs<'a> {
    normal: Option<&'a LoadedTexture>,
    gloss: Option<&'a LoadedTexture>,
    specular: Option<&'a LoadedTexture>,
}

impl PackedInputs<'_> {
    fn smoothness(&self, finish: Option<SurfaceFinish>) -> Option<Smoothness<'_>> {
        self.normal
            .map(|normal| Smoothness::NormalAlpha(&normal.image))
            .or_else(|| {
                self.gloss.map(|gloss| match finish {
                    Some(_) => Smoothness::ExplicitRed(&gloss.image),
                    None => Smoothness::ExplicitRgb(&gloss.image),
                })
            })
    }

    fn observe(
        &self,
        outcome: &mut Outcome,
        finish: Option<SurfaceFinish>,
        packed: &MetallicRoughness,
    ) {
        outcome.texture_sources.extend(
            self.normal
                .iter()
                .map(|texture| texture.contribution(TextureComponents::Alpha)),
        );
        outcome.texture_sources.extend(
            self.gloss
                .iter()
                .map(|texture| texture.contribution(TextureComponents::Color)),
        );
        outcome.texture_sources.extend(
            self.specular
                .iter()
                .map(|texture| texture.colour_contribution(TextureComponents::Color)),
        );
        outcome.palette_use = match (
            self.specular.is_some(),
            finish.is_some() && packed.roughness,
        ) {
            (true, true) => TexturePaletteUse::SpecularAndFinish,
            (true, false) => TexturePaletteUse::Specular,
            (false, true) => TexturePaletteUse::Finish,
            (false, false) => TexturePaletteUse::None,
        };
        outcome.roughness = packed.roughness;
        outcome.metallic = packed.metallic;
    }
}

#[cfg(test)]
#[path = "texture_make_pixels/finish_channel_tests.rs"]
mod finish_channel_tests;
