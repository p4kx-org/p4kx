// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_texture::{DdsTransfer, RgbaImage, TextureError};

use crate::layer_make;
use crate::layer_plan::{LayerPlan, Layered, MAX_BAKED_LAYERS};
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

fn bake(
    layered: &Layered,
    maps: &[Option<SmoothnessMap<'_>>],
    mask: Option<&RgbaImage>,
    size: (u32, u32),
) -> Result<Pixels, TextureError> {
    let sampled = AtomicUsize::new(0);
    let used = AtomicUsize::new(0);
    let image = texture_pixels::fill(size.0, size.1, |y, out| {
        let mut row_sampled = 0;
        let mut row_used = 0;
        let mut shares = [0.0; MAX_BAKED_LAYERS];
        for (x, target) in out.as_chunks_mut::<RGBA>().0.iter_mut().enumerate() {
            let uv = (centre(x, size.0 as usize), centre(y, size.1 as usize));
            layered.shares(mask_at(mask, x, y), &mut shares);
            let mut roughness = 0.0;
            for (index, ((layer, map), share)) in
                layered.layers.iter().zip(maps).zip(shares).enumerate()
            {
                if share <= 0.0 {
                    continue;
                }
                row_used |= 1 << index;
                let value = match (map, layer.finish) {
                    (Some(map), Some(finish)) => {
                        row_sampled |= 1 << index;
                        finish.sampled_roughness(map.sample(uv))
                    }
                    _ => layer.roughness,
                };
                roughness = value.mul_add(share, roughness);
            }
            *target = [
                OPAQUE,
                texture_pixels::encode_share(roughness),
                OPAQUE,
                OPAQUE,
            ];
        }
        sampled.fetch_or(row_sampled, Ordering::Relaxed);
        used.fetch_or(row_used, Ordering::Relaxed);
    })?;
    Ok(Pixels {
        image,
        sampled: sampled.load(Ordering::Relaxed),
        used: used.load(Ordering::Relaxed),
    })
}

fn sources(
    inputs: &[Input],
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
    let sized_by = mask.as_deref().or_else(|| {
        inputs.iter().find_map(|input| {
            input
                .selected
                .as_ref()
                .map(|selected| selected.texture.as_ref())
        })
    })?;
    let size = (sized_by.image.width(), sized_by.image.height());
    Some(
        maps(&inputs, (size.0 as usize, size.1 as usize))
            .and_then(|maps| bake(layered, &maps, mask.as_ref().map(|mask| &mask.image), size))
            .map_err(|error| error.to_string())
            .and_then(|pixels| {
                outcome.texture_sources = sources(&inputs, mask.as_deref(), sized_by, &pixels);
                outcome.material_layers = layered
                    .layers
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| pixels.used & (1 << *index) != 0)
                    .map(|(_, layer)| layer.at)
                    .collect();
                outcome.palette_use = TexturePaletteUse::Layers;
                outcome.roughness = true;
                texture_make::encode(&pixels.image, DdsTransfer::Linear)
            }),
    )
}

#[cfg(test)]
mod tests;
