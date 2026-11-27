// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_texture::{DdsTransfer, RgbaImage, TextureError};

use crate::layer_plan::{LayerMap, LayerPlan, Layered, MAX_BAKED_LAYERS};
use crate::layer_sampling::{self, Tile, TileChannels, round_byte};
#[cfg(test)]
use crate::layer_sampling::{axis, minify_rgb as minify, tile_len};
use crate::texture_loader::LoadedTexture;
use crate::texture_pixels::{
    OPAQUE, RGBA, colour_decoder, encode, fill, metal_share, sampled, texel,
};

const BYTE_UNIT: f32 = 1.0 / 255.0;
const HALF_BYTE: f32 = 127.5;

/// What a layer is blended from: the map of one layer in the form it is sampled from.
#[derive(Default)]
pub(crate) struct Maps<'a> {
    tiles: Vec<Option<Tile<'a>>>,
}

fn byte(value: u8) -> f32 {
    f32::from(value) * BYTE_UNIT
}

impl<'a> Maps<'a> {
    /// Prepares the maps of the layers for an output of `size` texels: `images` has the loaded map
    /// of each layer in its authored order.
    pub(crate) fn new(
        layered: &Layered,
        images: impl IntoIterator<Item = Option<&'a LoadedTexture>>,
        size: (usize, usize),
        kind: LayerMap,
    ) -> Result<Self, TextureError> {
        let tiles = layered
            .layers
            .iter()
            .zip(images)
            .map(|(layer, image)| match (image, kind.source(layer)) {
                (Some(image), Some((_, repeats))) => {
                    let colour = (kind != LayerMap::Normal).then(|| image.decoded_transfer());
                    let pixels = if colour.is_some() {
                        image.colour_image()?
                    } else {
                        Cow::Borrowed(&image.image)
                    };
                    layer_sampling::tile_of(pixels, *repeats, size, colour, TileChannels::Rgb)
                        .map(Some)
                }
                _ => Ok(None),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { tiles })
    }

    fn tile(&self, index: usize) -> Option<&Tile<'_>> {
        self.tiles.get(index)?.as_ref()
    }

    pub(crate) fn specular(
        &self,
        layer: &LayerPlan,
        index: usize,
        uv: (f32, f32),
    ) -> Option<Reflectance> {
        let tile = self.tile(index)?;
        let (_, repeats) = layer.specular_map.as_ref()?;
        let colour = tinted_tile(tile, *repeats, uv, layer.specular);
        Some(Reflectance {
            colour,
            metallic: metal_share(colour),
        })
    }

    pub(crate) fn has_tiles(&self) -> bool {
        self.tiles.iter().any(Option::is_some)
    }
}

fn mask_at(mask: Option<&RgbaImage>, x: usize, y: usize) -> Option<[f32; 3]> {
    let [red, green, blue, _] = texel(mask?, x, y);
    Some([byte(red), byte(green), byte(blue)])
}

fn centre(position: usize, length: usize) -> f32 {
    #[expect(clippy::cast_precision_loss, reason = "image sides are below 2^24")]
    let at = (position as f32 + 0.5) / length.max(1) as f32;
    at
}

fn tinted_tile(tile: &Tile<'_>, repeats: [f32; 2], uv: (f32, f32), tint: [f32; 3]) -> [f32; 3] {
    let transfer = tile.colour().unwrap_or(DdsTransfer::Unknown);
    let [red, green, blue] =
        layer_sampling::sample(tile.image(), repeats, uv, colour_decoder(transfer));
    let [tint_red, tint_green, tint_blue] = tint;
    [red * tint_red, green * tint_green, blue * tint_blue]
}

pub(crate) struct Reflectance {
    colour: [f32; 3],
    pub(crate) metallic: f32,
}

fn layer_colour(
    layer: &LayerPlan,
    tile: Option<&Tile<'_>>,
    uv: (f32, f32),
    reflected: Option<Reflectance>,
) -> [f32; 3] {
    let mut mixed = match (tile, &layer.diffuse) {
        (Some(tile), Some((_, repeats))) => tinted_tile(tile, *repeats, uv, layer.colour),
        _ => layer.colour,
    };
    let reflectance = reflected.unwrap_or(Reflectance {
        colour: layer.specular,
        metallic: layer.metal,
    });
    for (value, specular) in mixed.iter_mut().zip(reflectance.colour) {
        *value = (specular - *value).mul_add(reflectance.metallic, *value);
    }
    mixed
}

pub(crate) struct LayerPixels {
    pub(crate) image: RgbaImage,
    pub(crate) sampled_maps: usize,
    pub(crate) sampled_specular: usize,
    pub(crate) used_layers: usize,
}

/// Blends tiled diffuse and specular colour at the layers' masked or positional shares.
pub(crate) fn base_colour(
    layered: &Layered,
    maps: &Maps<'_>,
    specular: &Maps<'_>,
    mask: Option<&RgbaImage>,
    (width, height): (u32, u32),
) -> Result<LayerPixels, TextureError> {
    let size = (width as usize, height as usize);
    let sampled_maps = AtomicUsize::new(0);
    let sampled_specular = AtomicUsize::new(0);
    let used_layers = AtomicUsize::new(0);
    let image = fill(width, height, |y, out| {
        let mut sampled_row = 0;
        let mut specular_row = 0;
        let mut layer_row = 0;
        let mut shares = [0.0_f32; MAX_BAKED_LAYERS];
        for (x, target) in out.as_chunks_mut::<RGBA>().0.iter_mut().enumerate() {
            let uv = (centre(x, size.0), centre(y, size.1));
            layered.shares(mask_at(mask, x, y), &mut shares);
            let mut sum = [0.0_f32; 3];
            for (index, ((layer, tile), share)) in layered
                .layers
                .iter()
                .zip(&maps.tiles)
                .zip(shares)
                .enumerate()
            {
                if share <= 0.0 {
                    continue;
                }
                layer_row |= 1 << index;
                if tile.is_some() {
                    sampled_row |= 1 << index;
                }
                let reflected = specular.specular(layer, index, uv);
                if reflected.is_some() {
                    specular_row |= 1 << index;
                }
                let colour = layer_colour(layer, tile.as_ref(), uv, reflected);
                for (total, value) in sum.iter_mut().zip(colour) {
                    *total = value.mul_add(share, *total);
                }
            }
            let [red, green, blue] = sum;
            *target = [encode(red), encode(green), encode(blue), OPAQUE];
        }
        sampled_maps.fetch_or(sampled_row, Ordering::Relaxed);
        sampled_specular.fetch_or(specular_row, Ordering::Relaxed);
        used_layers.fetch_or(layer_row, Ordering::Relaxed);
    })?;
    Ok(LayerPixels {
        image,
        sampled_maps: sampled_maps.into_inner(),
        sampled_specular: sampled_specular.into_inner(),
        used_layers: used_layers.into_inner(),
    })
}

fn slope(texel: [u8; RGBA]) -> [f32; 2] {
    let [red, green, ..] = texel;
    [
        f32::from(red) / HALF_BYTE - 1.0,
        f32::from(green) / HALF_BYTE - 1.0,
    ]
}

const fn slope_byte(value: f32) -> u8 {
    round_byte(value.clamp(-1.0, 1.0).mul_add(HALF_BYTE, HALF_BYTE))
}

fn decode_slope(value: u8) -> f32 {
    f32::from(value) / HALF_BYTE - 1.0
}

/// Returns the normal image of `layered` of `size` texels: the layers' normal maps, tiled and
/// blended by the layers' shares, with the slopes of `own`, the material's own normal map
/// sampled at the same texel, added to them. The result is in glTF's convention, like
/// [`normal_map`](crate::texture_pixels::normal_map).
pub(crate) fn normal_colour(
    layered: &Layered,
    maps: &Maps<'_>,
    own: Option<&RgbaImage>,
    mask: Option<&RgbaImage>,
    (width, height): (u32, u32),
) -> Result<LayerPixels, TextureError> {
    let size = (width as usize, height as usize);
    let sampled_maps = AtomicUsize::new(0);
    let image = fill(width, height, |y, out| {
        let mut sampled_row = 0;
        let mut shares = [0.0_f32; MAX_BAKED_LAYERS];
        for (x, target) in out.as_chunks_mut::<RGBA>().0.iter_mut().enumerate() {
            let uv = (centre(x, size.0), centre(y, size.1));
            layered.shares(mask_at(mask, x, y), &mut shares);
            let mut slopes = own.map_or([0.0_f32; 2], |own| slope(sampled(own, (x, y), size)));
            for (index, ((layer, tile), share)) in layered
                .layers
                .iter()
                .zip(&maps.tiles)
                .zip(shares)
                .enumerate()
            {
                let (Some(tile), Some((_, repeats))) = (tile, &layer.normal) else {
                    continue;
                };
                if share <= 0.0 {
                    continue;
                }
                sampled_row |= 1 << index;
                let [across, down, _] =
                    layer_sampling::sample(tile.image(), *repeats, uv, decode_slope);
                slopes[0] = across.mul_add(share, slopes[0]);
                slopes[1] = down.mul_add(share, slopes[1]);
            }
            let [across, down] = slopes;
            let length = across.hypot(down).max(1.0);
            let (across, down) = (across / length, down / length);
            let depth = across
                .mul_add(-across, down.mul_add(-down, 1.0))
                .max(0.0)
                .sqrt();
            *target = [
                slope_byte(across),
                OPAQUE - slope_byte(down),
                slope_byte(depth),
                OPAQUE,
            ];
        }
        sampled_maps.fetch_or(sampled_row, Ordering::Relaxed);
    })?;
    let sampled_maps = sampled_maps.into_inner();
    Ok(LayerPixels {
        image,
        sampled_maps,
        sampled_specular: 0,
        used_layers: sampled_maps,
    })
}

#[cfg(test)]
mod tests;
