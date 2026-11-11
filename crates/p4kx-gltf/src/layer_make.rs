// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_texture::{RgbaImage, TextureError};

use crate::layer_bake::{Maps, base_colour, normal_colour};
use crate::layer_plan::Layered;
use crate::texture_cache::Decoded;
use crate::texture_loader::LoadedTexture;
use crate::texture_make::{Made, Outcome, encode, load as load_texture};
use crate::texture_origins::{TextureComponents, TextureReadUse};
use crate::texture_plan::{Failure, Job};
use crate::texture_report::TextureRole;
use crate::texture_roles::Source;

const fn side(image: &RgbaImage) -> (u32, u32) {
    (image.width(), image.height())
}

/// Loads the texture of `source`, which counts as not loaded when it has no pixels.
fn load(
    decoded: &Decoded,
    role: TextureRole,
    source: Option<&Source>,
    failures: &mut Vec<Failure>,
) -> Option<Arc<LoadedTexture>> {
    let texture = load_texture(decoded, role, source, failures)?;
    if texture.image.width() == 0 || texture.image.height() == 0 {
        failures.push(Failure {
            role,
            file: source?.file.clone(),
            reason: "the texture has no pixels".to_owned(),
        });
        return None;
    }
    Some(texture)
}

fn loaded_maps(
    layered: &Layered,
    decoded: &Decoded,
    normal: bool,
    failures: &mut Vec<Failure>,
) -> Vec<Option<Arc<LoadedTexture>>> {
    let role = if normal {
        TextureRole::Normal
    } else {
        TextureRole::Diffuse
    };
    layered
        .layers
        .iter()
        .map(|layer| {
            let map = if normal {
                &layer.normal
            } else {
                &layer.diffuse
            };
            load(
                decoded,
                role,
                map.as_ref().map(|(source, _)| source),
                failures,
            )
        })
        .collect()
}

pub(crate) struct LayerImage {
    pub(crate) image: RgbaImage,
    sources: Vec<TextureReadUse>,
}

fn uses(
    sized_by: &LoadedTexture,
    mask: Option<&LoadedTexture>,
    own: Option<&LoadedTexture>,
    layers: &[Option<Arc<LoadedTexture>>],
    sampled_maps: usize,
) -> Vec<TextureReadUse> {
    let sampled = layers.iter().enumerate().filter_map(|(index, image)| {
        if sampled_maps & (1 << index) == 0 {
            return None;
        }
        image.as_deref()
    });
    let colors = mask.into_iter().chain(own).chain(sampled);
    let mut size_used = false;
    let mut sources: Vec<_> = colors
        .map(|image| {
            size_used |= std::ptr::eq(image, sized_by);
            image.contribution(TextureComponents::Color)
        })
        .collect();
    if !size_used {
        sources.push(sized_by.contribution(TextureComponents::Dimensions));
    }
    sources
}

/// Blends the base colour image of a layered material from the diffuse maps of its layers; `None`
/// when no map could be loaded, which `failures` records.
pub(crate) fn bake_base(
    job: &Job,
    decoded: &Decoded,
    failures: &mut Vec<Failure>,
) -> Option<Result<LayerImage, TextureError>> {
    let layered = job.layered.as_ref()?;
    let mask = load(
        decoded,
        TextureRole::Diffuse,
        layered.mask.as_ref(),
        failures,
    );
    let loaded = loaded_maps(layered, decoded, false, failures);
    let sized_by = mask
        .as_deref()
        .or_else(|| loaded.iter().flatten().next().map(AsRef::as_ref))?;
    let size = side(&sized_by.image);
    let images: Vec<Option<&RgbaImage>> = loaded
        .iter()
        .map(|map| map.as_ref().map(|map| &map.image))
        .collect();
    Some(
        Maps::new(layered, &images, (size.0 as usize, size.1 as usize), false)
            .and_then(|maps| {
                base_colour(layered, &maps, mask.as_ref().map(|mask| &mask.image), size)
            })
            .map(|pixels| LayerImage {
                image: pixels.image,
                sources: uses(
                    sized_by,
                    mask.as_deref(),
                    None,
                    &loaded,
                    pixels.sampled_maps,
                ),
            }),
    )
}

/// Makes the base colour image of a layered material from the diffuse maps of its layers.
pub(crate) fn make_base(
    job: &Job,
    decoded: &Decoded,
    outcome: &mut Outcome,
    failures: &mut Vec<Failure>,
) -> Made {
    let made = bake_base(job, decoded, failures)?;
    outcome.tinted = true;
    outcome.layered = true;
    Some(made.map_err(|error| error.to_string()).and_then(|baked| {
        outcome.texture_sources = baked.sources;
        encode(&baked.image)
    }))
}

/// Makes the normal image of a layered material from the normal maps of its layers, with the
/// material's own normal map added.
pub(crate) fn make_normal(
    job: &Job,
    decoded: &Decoded,
    outcome: &mut Outcome,
    failures: &mut Vec<Failure>,
) -> Made {
    let layered = job.layered.as_ref()?;
    let mask = load(
        decoded,
        TextureRole::Diffuse,
        layered.mask.as_ref(),
        failures,
    );
    let own = load(decoded, TextureRole::Normal, job.normal.as_ref(), failures);
    let loaded = loaded_maps(layered, decoded, true, failures);
    let sized_by = mask
        .as_deref()
        .or_else(|| loaded.iter().flatten().next().map(AsRef::as_ref))
        .or(own.as_deref())?;
    let size = side(&sized_by.image);
    let images: Vec<Option<&RgbaImage>> = loaded
        .iter()
        .map(|map| map.as_ref().map(|map| &map.image))
        .collect();
    Some(
        Maps::new(layered, &images, (size.0 as usize, size.1 as usize), true)
            .and_then(|maps| {
                normal_colour(
                    layered,
                    &maps,
                    own.as_ref().map(|own| &own.image),
                    mask.as_ref().map(|mask| &mask.image),
                    size,
                )
            })
            .map_err(|error| error.to_string())
            .and_then(|pixels| {
                outcome.texture_sources = uses(
                    sized_by,
                    mask.as_deref(),
                    own.as_deref(),
                    &loaded,
                    pixels.sampled_maps,
                );
                encode(&pixels.image)
            }),
    )
}
