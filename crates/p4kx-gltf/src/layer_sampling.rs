// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_texture::{DdsTransfer, RgbaImage, TextureError};

use crate::texture_pixels::{OPAQUE, RGBA, colour_decoder, encode, fill, texel};

pub(crate) struct Tile<'a> {
    image: Cow<'a, RgbaImage>,
    colour: Option<DdsTransfer>,
}

impl Tile<'_> {
    pub(crate) fn image(&self) -> &RgbaImage {
        &self.image
    }

    pub(crate) const fn colour(&self) -> Option<DdsTransfer> {
        self.colour
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TileChannels {
    Rgb,
    Rgba,
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "image sides are far below 2^24 and the ratio is positive"
)]
pub(crate) fn tile_len(output: usize, repeats: f32, source: usize) -> usize {
    let wanted = (output as f32 / repeats).round() as usize;
    wanted.clamp(1, source.max(1))
}

/// Returns `image` averaged down to `width` Ãƒâ€” `height` texels, a box filter; the averages of a
/// colour map are taken in linear light.
fn minify(
    image: &RgbaImage,
    (width, height): (usize, usize),
    colour: Option<DdsTransfer>,
    channels: TileChannels,
) -> Result<RgbaImage, TextureError> {
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
                *target = average_box(image, (left, top, right, bottom), colour, channels);
            }
        },
    )
}

pub(crate) const fn round_byte(value: f32) -> u8 {
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
pub(crate) fn tile_of(
    image: Cow<'_, RgbaImage>,
    repeats: [f32; 2],
    size: (usize, usize),
    colour: Option<DdsTransfer>,
    channels: TileChannels,
) -> Result<Tile<'_>, TextureError> {
    let (width, height) = (image.width() as usize, image.height() as usize);
    if width == 0 || height == 0 {
        return Ok(Tile { image, colour });
    }
    let wanted = (
        tile_len(size.0, repeats[0], width),
        tile_len(size.1, repeats[1], height),
    );
    if wanted == (width, height) {
        Ok(Tile { image, colour })
    } else {
        Ok(Tile {
            image: Cow::Owned(minify(&image, wanted, colour, channels)?),
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
pub(crate) fn axis(coordinate: f32, repeats: f32, length: usize) -> (usize, usize, f32) {
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
pub(crate) fn sample(
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

#[cfg(test)]
pub(crate) fn minify_rgb(
    image: &RgbaImage,
    size: (usize, usize),
    colour: Option<DdsTransfer>,
) -> Result<RgbaImage, TextureError> {
    minify(image, size, colour, TileChannels::Rgb)
}

pub(crate) fn sample_alpha(image: &RgbaImage, repeats: [f32; 2], (u, v): (f32, f32)) -> f32 {
    let (left, right, across) = axis(u, repeats[0], image.width() as usize);
    let (top, bottom, down) = axis(v, repeats[1], image.height() as usize);
    let corners = [
        (left, top, (1.0 - across) * (1.0 - down)),
        (right, top, across * (1.0 - down)),
        (left, bottom, (1.0 - across) * down),
        (right, bottom, across * down),
    ];
    corners
        .into_iter()
        .map(|(x, y, weight)| f32::from(texel(image, x, y)[3]) / 255.0 * weight)
        .sum()
}

fn average_box(
    image: &RgbaImage,
    (left, top, right, bottom): (usize, usize, usize, usize),
    colour: Option<DdsTransfer>,
    channels: TileChannels,
) -> [u8; RGBA] {
    let decode = colour.map_or(f32::from as fn(u8) -> f32, colour_decoder);
    let mut sum = [0.0_f32; 3];
    let mut alpha = 0.0_f32;
    for row in top..bottom {
        for column in left..right {
            let [red, green, blue, value] = texel(image, column, row);
            if channels == TileChannels::Rgba {
                alpha += f32::from(value);
            }
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
    let alpha = if channels == TileChannels::Rgba {
        round_byte(alpha / count)
    } else {
        OPAQUE
    };
    [red, green, blue, alpha]
}
