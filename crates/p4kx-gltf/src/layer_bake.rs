// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_texture::{DdsTransfer, RgbaImage, TextureError};

use crate::layer_plan::{LayerPlan, Layered, MAX_BAKED_LAYERS};
use crate::texture_loader::LoadedTexture;
use crate::texture_pixels::{OPAQUE, RGBA, colour_decoder, encode, fill, sampled, texel};

const BYTE_UNIT: f32 = 1.0 / 255.0;
const HALF_BYTE: f32 = 127.5;

struct Tile<'a> {
    image: Cow<'a, RgbaImage>,
    colour: Option<DdsTransfer>,
}

/// What a layer is blended from: the map of one layer in the form it is sampled from.
pub(crate) struct Maps<'a> {
    tiles: Vec<Option<Tile<'a>>>,
}

fn byte(value: u8) -> f32 {
    f32::from(value) * BYTE_UNIT
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "image sides are far below 2^24 and the ratio is positive"
)]
fn tile_len(output: usize, repeats: f32, source: usize) -> usize {
    let wanted = (output as f32 / repeats).round() as usize;
    wanted.clamp(1, source.max(1))
}

/// Returns `image` averaged down to `width` × `height` texels, a box filter; the averages of a
/// colour map are taken in linear light.
fn minify(
    image: &RgbaImage,
    (width, height): (usize, usize),
    colour: Option<DdsTransfer>,
) -> Result<RgbaImage, TextureError> {
    let decode = colour.map_or(f32::from as fn(u8) -> f32, colour_decoder);
    let (source_width, source_height) = (image.width() as usize, image.height() as usize);
    fill(
        u32::try_from(width).unwrap_or(u32::MAX),
        u32::try_from(height).unwrap_or(u32::MAX),
        |y, out| {
            let top = y * source_height / height;
            let bottom = ((y + 1) * source_height / height).clamp(top + 1, source_height);
            for (x, target) in out.as_chunks_mut::<RGBA>().0.iter_mut().enumerate() {
                let left = x * source_width / width;
                let right = ((x + 1) * source_width / width).clamp(left + 1, source_width);
                let mut sum = [0.0_f32; 3];
                for row in top..bottom {
                    for column in left..right {
                        let [red, green, blue, _] = texel(image, column, row);
                        for (total, value) in sum.iter_mut().zip([red, green, blue]) {
                            *total += decode(value);
                        }
                    }
                }
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "the box holds far fewer than 2^24 texels"
                )]
                let count = ((bottom - top) * (right - left)) as f32;
                let [red, green, blue] = sum.map(|total| total / count);
                let [red, green, blue] = if colour.is_some() {
                    [red, green, blue].map(encode)
                } else {
                    [red, green, blue].map(round_byte)
                };
                *target = [red, green, blue, OPAQUE];
            }
        },
    )
}

const fn round_byte(value: f32) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is between 0 and 255"
    )]
    let byte = value.round().clamp(0.0, 255.0) as u8;
    byte
}

/// Returns the tile of `image` that repeats `repeats` times over an output of `size` texels: the
/// image itself when it is not larger than that, else averaged down to the texels one repeat
/// covers, so that a heavily tiled layer gives its mean colour and not noise.
fn tile_of(
    image: &RgbaImage,
    repeats: [f32; 2],
    size: (usize, usize),
    colour: Option<DdsTransfer>,
) -> Result<Tile<'_>, TextureError> {
    let (width, height) = (image.width() as usize, image.height() as usize);
    if width == 0 || height == 0 {
        return Ok(Tile {
            image: Cow::Borrowed(image),
            colour,
        });
    }
    let wanted = (
        tile_len(size.0, repeats[0], width),
        tile_len(size.1, repeats[1], height),
    );
    if wanted == (width, height) {
        Ok(Tile {
            image: Cow::Borrowed(image),
            colour,
        })
    } else {
        Ok(Tile {
            image: Cow::Owned(minify(image, wanted, colour)?),
            colour: colour.map(|_| DdsTransfer::Srgb),
        })
    }
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "tile sides are far below 2^24"
)]
fn axis(coordinate: f32, repeats: f32, length: usize) -> (usize, usize, f32) {
    let scaled = (coordinate * repeats)
        .rem_euclid(1.0)
        .mul_add(length as f32, -0.5);
    let floor = scaled.floor();
    let side = length.max(1) as isize;
    let first = (floor as isize).rem_euclid(side) as usize;
    let second = (first + 1) % length.max(1);
    (first, second, scaled - floor)
}

/// Returns the texel of `tile` at (`u`, `v`) of the output, with the tile repeated `repeats`
/// times, interpolated between the four nearest texels and wrapped; `decode` turns a byte into
/// the value interpolated.
fn sample(
    tile: &RgbaImage,
    repeats: [f32; 2],
    (u, v): (f32, f32),
    decode: fn(u8) -> f32,
) -> [f32; 3] {
    let (left, right, across) = axis(u, repeats[0], tile.width() as usize);
    let (top, bottom, down) = axis(v, repeats[1], tile.height() as usize);
    let corners = [
        (texel(tile, left, top), (1.0 - across) * (1.0 - down)),
        (texel(tile, right, top), across * (1.0 - down)),
        (texel(tile, left, bottom), (1.0 - across) * down),
        (texel(tile, right, bottom), across * down),
    ];
    let mut mixed = [0.0_f32; 3];
    for ([red, green, blue, _], weight) in corners {
        for (total, value) in mixed.iter_mut().zip([red, green, blue]) {
            *total = decode(value).mul_add(weight, *total);
        }
    }
    mixed
}

impl<'a> Maps<'a> {
    /// Prepares the maps of the layers for an output of `size` texels: `images` has the loaded map
    /// of each layer, in the order of the layers, and `normal` says whether they are normal maps.
    pub(crate) fn new(
        layered: &Layered,
        images: &[Option<&'a LoadedTexture>],
        size: (usize, usize),
        normal: bool,
    ) -> Result<Self, TextureError> {
        let tiles = layered
            .layers
            .iter()
            .zip(images)
            .map(|(layer, image)| {
                let map = if normal {
                    &layer.normal
                } else {
                    &layer.diffuse
                };
                match (image, map) {
                    (Some(image), Some((_, repeats))) => {
                        let colour = (!normal).then(|| image.decoded_transfer());
                        tile_of(&image.image, *repeats, size, colour).map(Some)
                    }
                    _ => Ok(None),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { tiles })
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

fn layer_colour(layer: &LayerPlan, tile: Option<&Tile<'_>>, uv: (f32, f32)) -> [f32; 3] {
    let own = match (tile, &layer.diffuse) {
        (Some(tile), Some((_, repeats))) => {
            let transfer = tile.colour.unwrap_or(DdsTransfer::Unknown);
            let [red, green, blue] = sample(&tile.image, *repeats, uv, colour_decoder(transfer));
            let [tint_red, tint_green, tint_blue] = layer.colour;
            [red * tint_red, green * tint_green, blue * tint_blue]
        }
        _ => layer.colour,
    };
    let mut mixed = own;
    for (value, specular) in mixed.iter_mut().zip(layer.specular) {
        *value = (specular - *value).mul_add(layer.metal, *value);
    }
    mixed
}

pub(crate) struct LayerPixels {
    pub(crate) image: RgbaImage,
    pub(crate) sampled_maps: usize,
    pub(crate) used_layers: usize,
}

/// Returns the base colour image of `layered` of `size` texels: the layers' diffuse maps, tiled,
/// multiplied by the layers' colours, moved towards their specular colour by their metalness
/// and blended by the layers' shares, which `mask` (of that size) gives where the material has
/// one. A layer without a loaded map keeps its plain colour.
pub(crate) fn base_colour(
    layered: &Layered,
    maps: &Maps<'_>,
    mask: Option<&RgbaImage>,
    (width, height): (u32, u32),
) -> Result<LayerPixels, TextureError> {
    let size = (width as usize, height as usize);
    let sampled_maps = AtomicUsize::new(0);
    let used_layers = AtomicUsize::new(0);
    let image = fill(width, height, |y, out| {
        let mut sampled_row = 0;
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
                let colour = layer_colour(layer, tile.as_ref(), uv);
                for (total, value) in sum.iter_mut().zip(colour) {
                    *total = value.mul_add(share, *total);
                }
            }
            let [red, green, blue] = sum;
            *target = [encode(red), encode(green), encode(blue), OPAQUE];
        }
        sampled_maps.fetch_or(sampled_row, Ordering::Relaxed);
        used_layers.fetch_or(layer_row, Ordering::Relaxed);
    })?;
    Ok(LayerPixels {
        image,
        sampled_maps: sampled_maps.into_inner(),
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
                let [across, down, _] = sample(&tile.image, *repeats, uv, decode_slope);
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
        used_layers: sampled_maps,
    })
}

#[cfg(test)]
mod tests;
