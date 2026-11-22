// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_texture::{DdsTransfer, RgbaImage, TextureError};

use crate::layer_bake::Maps;
use crate::layer_make;
use crate::layer_plan::{LayerMap, LayerPlan, Layered, MAX_BAKED_LAYERS};
use crate::layer_smoothness::{SmoothnessChannel, SmoothnessMap};
use crate::texture_cache::Decoded;
use crate::texture_loader::LoadedTexture;
use crate::texture_make::{self, Made, Outcome};
use crate::texture_origins::{TextureComponents, TexturePaletteUse, TextureReadUse};
use crate::texture_pixels::{self, OPAQUE, RGBA};
use crate::texture_plan::{Failure, Job};
use crate::texture_report::TextureRole;

struct Selected {
    texture: Arc<LoadedTexture>,
    repeats: [f32; 2],
    channel: SmoothnessChannel,
}

#[derive(Default)]
struct Input {
    selected: Option<Selected>,
    selectors: Vec<TextureReadUse>,
}

fn select(layer: &LayerPlan, decoded: &Decoded, failures: &mut Vec<Failure>) -> Input {
    let mut input = Input::default();
    if layer.finish.is_none() {
        return input;
    }
    if let Some((source, repeats)) = layer.normal.as_ref().filter(|(source, _)| source.gloss)
        && let Some(texture) =
            layer_make::load(decoded, TextureRole::Normal, Some(source), failures)
    {
        if texture.has_alpha {
            input.selected = Some(Selected {
                texture,
                repeats: *repeats,
                channel: SmoothnessChannel::Alpha,
            });
            return input;
        }
        input
            .selectors
            .push(texture.contribution(TextureComponents::Dimensions));
    }
    for (source, repeats) in &layer.gloss {
        if let Some(texture) = layer_make::load(decoded, TextureRole::Gloss, Some(source), failures)
        {
            input.selected = Some(Selected {
                texture,
                repeats: *repeats,
                channel: SmoothnessChannel::Red,
            });
            break;
        }
    }
    input
}

fn maps(
    inputs: &[Input],
    size: (usize, usize),
) -> Result<Vec<Option<SmoothnessMap<'_>>>, TextureError> {
    inputs
        .iter()
        .map(|input| {
            input
                .selected
                .as_ref()
                .map(|selected| {
                    SmoothnessMap::new(&selected.texture, selected.repeats, size, selected.channel)
                })
                .transpose()
        })
        .collect()
}

struct Pixels {
    image: RgbaImage,
    sampled: usize,
    sampled_specular: usize,
    used: usize,
}

fn mask_at(mask: Option<&RgbaImage>, x: usize, y: usize) -> Option<[f32; 3]> {
    let [red, green, blue, _] = texture_pixels::texel(mask?, x, y);
    Some([red, green, blue].map(texture_pixels::unit))
}

fn centre(position: usize, length: usize) -> f32 {
    #[expect(clippy::cast_precision_loss, reason = "image sides are below 2^24")]
    let value = (position as f32 + 0.5) / length.max(1) as f32;
    value
}

#[derive(Default)]
struct SampledLayers {
    smoothness: usize,
    specular: usize,
    used: usize,
}

fn packed_sample(
    layered: &Layered,
    maps: &[Option<SmoothnessMap<'_>>],
    specular: Option<&Maps<'_>>,
    shares: [f32; MAX_BAKED_LAYERS],
    uv: (f32, f32),
    sampled: &mut SampledLayers,
) -> (f32, f32) {
    let mut roughness = 0.0;
    let mut metallic = 0.0;
    for (index, ((layer, map), share)) in layered.layers.iter().zip(maps).zip(shares).enumerate() {
        if share <= 0.0 {
            continue;
        }
        sampled.used |= 1 << index;
        let value = match (map, layer.finish) {
            (Some(map), Some(finish)) => {
                sampled.smoothness |= 1 << index;
                finish.sampled_roughness(map.sample(uv))
            }
            _ => layer.roughness,
        };
        roughness = value.mul_add(share, roughness);
        if let Some(specular) = specular {
            let reflectance = specular.specular(layer, index, uv);
            if reflectance.is_some() {
                sampled.specular |= 1 << index;
            }
            let share_at_texel = reflectance.map_or(layer.metal, |value| value.metallic);
            metallic = share_at_texel.mul_add(share, metallic);
        }
    }
    (roughness, metallic)
}

fn opaque_metallic_channel(image: RgbaImage) -> Result<RgbaImage, TextureError> {
    let (width, height) = (image.width(), image.height());
    let mut pixels = image.into_pixels();
    for [_, _, blue, _] in pixels.as_chunks_mut::<RGBA>().0 {
        *blue = OPAQUE;
    }
    RgbaImage::new(width, height, pixels)
}

fn bake(
    layered: &Layered,
    maps: &[Option<SmoothnessMap<'_>>],
    specular: &Maps<'_>,
    roughness_enabled: bool,
    mask: Option<&RgbaImage>,
    size: (u32, u32),
) -> Result<Pixels, TextureError> {
    let sampled = AtomicUsize::new(0);
    let sampled_specular = AtomicUsize::new(0);
    let used = AtomicUsize::new(0);
    let specular = specular.has_tiles().then_some(specular);
    let image = texture_pixels::fill(size.0, size.1, |y, out| {
        let mut sampled_row = SampledLayers::default();
        let mut shares = [0.0; MAX_BAKED_LAYERS];
        for (x, target) in out.as_chunks_mut::<RGBA>().0.iter_mut().enumerate() {
            let uv = (centre(x, size.0 as usize), centre(y, size.1 as usize));
            layered.shares(mask_at(mask, x, y), &mut shares);
            let (roughness, metallic) =
                packed_sample(layered, maps, specular, shares, uv, &mut sampled_row);
            let green = if roughness_enabled {
                texture_pixels::encode_share(roughness)
            } else {
                OPAQUE
            };
            let blue = if specular.is_some() {
                texture_pixels::encode_share(metallic)
            } else {
                OPAQUE
            };
            *target = [OPAQUE, green, blue, OPAQUE];
        }
        sampled.fetch_or(sampled_row.smoothness, Ordering::Relaxed);
        sampled_specular.fetch_or(sampled_row.specular, Ordering::Relaxed);
        used.fetch_or(sampled_row.used, Ordering::Relaxed);
    })?;
    let sampled_specular = sampled_specular.into_inner();
    let image = if specular.is_some() && sampled_specular == 0 {
        opaque_metallic_channel(image)?
    } else {
        image
    };
    Ok(Pixels {
        image,
        sampled: sampled.into_inner(),
        sampled_specular,
        used: used.into_inner(),
    })
}

fn sources(
    inputs: &[Input],
    specular: &[Option<Arc<LoadedTexture>>],
    mask: Option<&LoadedTexture>,
    sized_by: &LoadedTexture,
    pixels: &Pixels,
) -> Vec<TextureReadUse> {
    let mut sources: Vec<_> = mask
        .iter()
        .map(|mask| mask.contribution(TextureComponents::Color))
        .collect();
    let mut size_used = mask.is_some_and(|mask| std::ptr::eq(mask, sized_by));
    for (index, input) in inputs.iter().enumerate() {
        if pixels.used & (1 << index) == 0 {
            continue;
        }
        sources.extend(input.selectors.iter().copied());
        if pixels.sampled & (1 << index) == 0 {
            continue;
        }
        if let Some(selected) = &input.selected {
            let components = match selected.channel {
                SmoothnessChannel::Alpha => TextureComponents::Alpha,
                SmoothnessChannel::Red => TextureComponents::Color,
            };
            sources.push(selected.texture.contribution(components));
            size_used |= std::ptr::eq(selected.texture.as_ref(), sized_by);
        }
    }
    for texture in layer_make::sampled_textures(specular, pixels.sampled_specular) {
        sources.push(texture.colour_contribution(TextureComponents::Color));
        size_used |= std::ptr::eq(texture, sized_by);
    }
    if !size_used {
        sources.push(sized_by.contribution(TextureComponents::Dimensions));
    }
    sources
}

pub(crate) fn make(
    job: &Job,
    decoded: &Decoded,
    outcome: &mut Outcome,
    failures: &mut Vec<Failure>,
) -> Made {
    let layered = job.layered.as_ref()?;
    let mask = layer_make::load(
        decoded,
        TextureRole::Diffuse,
        layered.mask.as_ref(),
        failures,
    );
    let inputs: Vec<_> = layered
        .layers
        .iter()
        .map(|layer| select(layer, decoded, failures))
        .collect();
    let specular = layer_make::loaded_maps(layered, decoded, LayerMap::Specular, failures);
    make_loaded(layered, &inputs, &specular, mask.as_deref(), outcome)
}

fn make_loaded(
    layered: &Layered,
    inputs: &[Input],
    specular: &[Option<Arc<LoadedTexture>>],
    mask: Option<&LoadedTexture>,
    outcome: &mut Outcome,
) -> Made {
    let smoothness = inputs.iter().find_map(|input| {
        input
            .selected
            .as_ref()
            .map(|selected| selected.texture.as_ref())
    });
    let prior_size = layered
        .has_sampled_finish()
        .then(|| mask.or(smoothness))
        .flatten();
    let sized_by = prior_size.or_else(|| {
        let metal = specular.iter().find_map(Option::as_deref)?;
        mask.or(Some(metal))
    })?;
    let roughness_enabled = prior_size.is_some();
    let size = (sized_by.image.width(), sized_by.image.height());
    let tile_size = (size.0 as usize, size.1 as usize);
    let pixels = maps(inputs, tile_size).and_then(|smoothness| {
        let reflectance = Maps::new(
            layered,
            specular.iter().map(Option::as_deref),
            tile_size,
            LayerMap::Specular,
        )?;
        bake(
            layered,
            &smoothness,
            &reflectance,
            roughness_enabled,
            mask.map(|mask| &mask.image),
            size,
        )
    });
    match pixels {
        Ok(pixels) if roughness_enabled || pixels.sampled_specular != 0 => {
            outcome.texture_sources = sources(inputs, specular, mask, sized_by, &pixels);
            outcome.material_layers = layer_make::used_positions(layered, pixels.used);
            outcome.palette_use = TexturePaletteUse::Layers;
            outcome.roughness = roughness_enabled;
            outcome.metallic = pixels.sampled_specular != 0;
            Some(texture_make::encode(&pixels.image, DdsTransfer::Linear))
        }
        Ok(_) => None,
        Err(error) => Some(Err(error.to_string())),
    }
}

#[cfg(test)]
mod tests;
