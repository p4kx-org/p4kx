// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::DdsTransfer;

use super::{Made, Outcome, encode, load};
use crate::texture_cache::Decoded;
use crate::texture_origins::{TextureComponents, TexturePaletteUse};
use crate::texture_pixels::{base_colour, metallic_roughness, normal_map, opacity_colour};
use crate::texture_plan::{Failure, Job};
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
    let components = if keep_alpha && diffuse.has_alpha {
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
    let gloss = load(decoded, TextureRole::Normal, job.normal.as_ref(), failures)
        .filter(|gloss| gloss.has_alpha);
    let specular = load(
        decoded,
        TextureRole::Specular,
        job.specular.as_ref(),
        failures,
    );
    match metallic_roughness(
        gloss.as_ref().map(|gloss| &gloss.image),
        specular.as_deref(),
        job.tints.specular,
    ) {
        Ok(Some(packed)) => {
            if specular.is_some() {
                outcome.palette_use = TexturePaletteUse::Specular;
            }
            outcome.texture_sources.extend(
                gloss
                    .iter()
                    .map(|texture| texture.contribution(TextureComponents::Alpha)),
            );
            outcome.texture_sources.extend(
                specular
                    .iter()
                    .map(|texture| texture.colour_contribution(TextureComponents::Color)),
            );
            outcome.roughness = packed.roughness;
            outcome.metallic = packed.metallic;
            Some(encode(&packed.image, DdsTransfer::Linear))
        }
        Ok(None) => None,
        Err(error) => Some(Err(error.to_string())),
    }
}
