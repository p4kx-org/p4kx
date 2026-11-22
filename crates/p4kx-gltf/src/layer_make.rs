// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_texture::{DdsTransfer, RgbaImage, TextureError};

use crate::layer_bake::{Maps, base_colour, normal_colour};
use crate::layer_plan::{LayerMap, Layered};
use crate::texture_cache::Decoded;
use crate::texture_loader::LoadedTexture;
use crate::texture_make::{Made, Outcome, encode, load as load_texture};
use crate::texture_origins::{TextureComponents, TexturePaletteUse, TextureReadUse};
use crate::texture_plan::{Failure, Job};
use crate::texture_report::TextureRole;
use crate::texture_roles::Source;

const fn side(image: &RgbaImage) -> (u32, u32) {
    (image.width(), image.height())
}

/// Loads the texture of `source`, which counts as not loaded when it has no pixels.
pub(super) fn load(
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

pub(super) fn loaded_maps(
    layered: &Layered,
    decoded: &Decoded,
    kind: LayerMap,
    failures: &mut Vec<Failure>,
) -> Vec<Option<Arc<LoadedTexture>>> {
    if kind == LayerMap::Specular && !layered.has_specular_map() {
        return Vec::new();
    }
    layered
        .layers
        .iter()
        .map(|layer| {
            load(
                decoded,
                kind.role(),
                kind.source(layer).map(|(source, _)| source),
                failures,
            )
        })
        .collect()
}

pub(crate) struct LayerImage {
    pub(crate) image: RgbaImage,
    sources: Vec<TextureReadUse>,
    material_layers: Vec<usize>,
}

fn uses<'image>(
    sized_by: &'image LoadedTexture,
    mask: Option<&'image LoadedTexture>,
    own: Option<&'image LoadedTexture>,
    layers: &'image [Option<Arc<LoadedTexture>>],
    sampled_maps: usize,
    extra_colour: impl Iterator<Item = &'image LoadedTexture>,
    colour: bool,
) -> Vec<TextureReadUse> {
    let sampled = sampled_textures(layers, sampled_maps);
    let sources = mask.into_iter().chain(own).map(|image| (image, false));
    let sources = sources
        .chain(sampled.map(|image| (image, colour)))
        .chain(extra_colour.map(|image| (image, true)));
    let mut size_used = false;
    let mut sources: Vec<_> = sources
        .map(|(image, colour)| {
            size_used |= std::ptr::eq(image, sized_by);
            if colour {
                image.colour_contribution(TextureComponents::Color)
            } else {
                image.contribution(TextureComponents::Color)
            }
        })
        .collect();
    if !size_used {
        sources.push(sized_by.contribution(TextureComponents::Dimensions));
    }
    sources
}

pub(super) fn sampled_textures(
    layers: &[Option<Arc<LoadedTexture>>],
    sampled: usize,
) -> impl Iterator<Item = &LoadedTexture> {
    layers.iter().enumerate().filter_map(move |(index, image)| {
        if sampled & (1 << index) == 0 {
            return None;
        }
        image.as_deref()
    })
}

pub(super) fn used_positions(layered: &Layered, used_layers: usize) -> Vec<usize> {
    layered
        .layers
        .iter()
        .enumerate()
        .filter_map(|(index, layer)| (used_layers & (1 << index) != 0).then_some(layer.at))
        .collect()
}

/// Blends the base colour from the layers' selected maps, recording failed reads.
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
    let loaded = loaded_maps(layered, decoded, LayerMap::Diffuse, failures);
    let specular = loaded_maps(layered, decoded, LayerMap::Specular, failures);
    let sized_by = mask
        .as_deref()
        .or_else(|| loaded.iter().chain(&specular).find_map(Option::as_deref))?;
    Some(bake_base_maps(
        layered,
        &loaded,
        &specular,
        mask.as_deref(),
        sized_by,
    ))
}

fn bake_base_maps(
    layered: &Layered,
    diffuse: &[Option<Arc<LoadedTexture>>],
    specular: &[Option<Arc<LoadedTexture>>],
    mask: Option<&LoadedTexture>,
    sized_by: &LoadedTexture,
) -> Result<LayerImage, TextureError> {
    let size = side(&sized_by.image);
    let tile_size = (size.0 as usize, size.1 as usize);
    let diffuse_maps = Maps::new(
        layered,
        diffuse.iter().map(Option::as_deref),
        tile_size,
        LayerMap::Diffuse,
    )?;
    let specular_maps = Maps::new(
        layered,
        specular.iter().map(Option::as_deref),
        tile_size,
        LayerMap::Specular,
    )?;
    let pixels = base_colour(
        layered,
        &diffuse_maps,
        &specular_maps,
        mask.map(|mask| &mask.image),
        size,
    )?;
    let sampled_specular = sampled_textures(specular, pixels.sampled_specular);
    Ok(LayerImage {
        image: pixels.image,
        material_layers: used_positions(layered, pixels.used_layers),
        sources: uses(
            sized_by,
            mask,
            None,
            diffuse,
            pixels.sampled_maps,
            sampled_specular,
            true,
        ),
    })
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
        outcome.material_layers = baked.material_layers;
        outcome.palette_use = TexturePaletteUse::Layers;
        encode(&baked.image, DdsTransfer::Srgb)
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
    let loaded = loaded_maps(layered, decoded, LayerMap::Normal, failures);
    let sized_by = mask
        .as_deref()
        .or_else(|| loaded.iter().flatten().next().map(AsRef::as_ref))
        .or(own.as_deref())?;
    let size = side(&sized_by.image);
    Some(
        Maps::new(
            layered,
            loaded.iter().map(Option::as_deref),
            (size.0 as usize, size.1 as usize),
            LayerMap::Normal,
        )
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
                std::iter::empty(),
                false,
            );
            outcome.material_layers = used_positions(layered, pixels.used_layers);
            encode(&pixels.image, DdsTransfer::Linear)
        }),
    )
}
