// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::LazyLock;

use p4kx_texture::{RgbaImage, TextureError};
use rayon::prelude::*;

pub(crate) const RGBA: usize = 4;
pub(crate) const OPAQUE: u8 = u8::MAX;
const METAL_FROM: f32 = 0.15;
const METAL_TO: f32 = 0.5;
const ENCODE_STEPS: usize = 4095;
const ENCODE_STEPS_F32: f32 = 4095.0;

static LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    std::array::from_fn(|value| {
        #[expect(clippy::cast_precision_loss, reason = "the value is below 256")]
        let encoded = value as f32 / 255.0;
        if encoded <= 0.040_45 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        }
    })
});

static ENCODED: LazyLock<[u8; ENCODE_STEPS + 1]> = LazyLock::new(|| {
    std::array::from_fn(|step| {
        #[expect(clippy::cast_precision_loss, reason = "the step is below 4096")]
        let linear = step as f32 / ENCODE_STEPS_F32;
        let encoded = if linear <= 0.003_130_8 {
            linear * 12.92
        } else {
            1.055_f32.mul_add(linear.powf(1.0 / 2.4), -0.055)
        };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the value is between 0 and 255"
        )]
        let byte = (encoded * 255.0).round().clamp(0.0, 255.0) as u8;
        byte
    })
});

#[cfg(test)]
mod tests;

/// The colours of a material that tint its textures, linear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Tints {
    /// The diffuse colour, which multiplies the diffuse map.
    pub(crate) diffuse: [f32; 3],
    /// The specular colour, which multiplies the specular map.
    pub(crate) specular: [f32; 3],
    /// The colours of the red, green and blue masks of a stencil map, which the opacity map of a
    /// stencil decal that takes the decal colours is painted with.
    pub(crate) decal: Option<[[f32; 3]; 3]>,
}

pub(crate) fn linear(value: u8) -> f32 {
    LINEAR.get(usize::from(value)).copied().unwrap_or_default()
}

pub(crate) fn encode(value: f32) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the index is between 0 and the table's last"
    )]
    let step = (value.clamp(0.0, 1.0) * ENCODE_STEPS_F32).round() as usize;
    ENCODED.get(step).copied().unwrap_or(OPAQUE)
}

fn luminance([red, green, blue]: [f32; 3]) -> f32 {
    0.0722_f32.mul_add(blue, 0.2126_f32.mul_add(red, 0.7152 * green))
}

/// Returns how metallic a surface with the linear specular colour `specular` is, 0 to 1: its
/// luminance from [`METAL_FROM`] (a dielectric's reflectance at most) to [`METAL_TO`] (a metal's
/// at least).
pub(crate) fn metal_share(specular: [f32; 3]) -> f32 {
    ((luminance(specular) - METAL_FROM) / (METAL_TO - METAL_FROM)).clamp(0.0, 1.0)
}

/// Returns the linear specular colour of a texel of the specular map, tinted.
fn specular_of([red, green, blue]: [u8; 3], tint: [f32; 3]) -> [f32; 3] {
    let [tr, tg, tb] = tint;
    [linear(red) * tr, linear(green) * tg, linear(blue) * tb]
}

/// Returns the position in a row or column of `from` texels that covers position `at` of one of
/// `to` texels, nearest texel.
fn nearest(at: usize, from: usize, to: usize) -> usize {
    (at.saturating_mul(from) / to.max(1)).min(from.saturating_sub(1))
}

pub(crate) fn texel(image: &RgbaImage, x: usize, y: usize) -> [u8; RGBA] {
    let width = image.width() as usize;
    let at = (y * width + x) * RGBA;
    image
        .pixels()
        .get(at..at + RGBA)
        .and_then(|texel| <[u8; RGBA]>::try_from(texel).ok())
        .unwrap_or([0, 0, 0, OPAQUE])
}

/// Returns the texel of `image` at the position of texel (`x`, `y`) of an image of
/// `width` × `height` texels.
pub(crate) fn sampled(
    image: &RgbaImage,
    (x, y): (usize, usize),
    (width, height): (usize, usize),
) -> [u8; RGBA] {
    texel(
        image,
        nearest(x, image.width() as usize, width),
        nearest(y, image.height() as usize, height),
    )
}

pub(crate) fn fill(
    width: u32,
    height: u32,
    row: impl Fn(usize, &mut [u8]) + Sync,
) -> Result<RgbaImage, TextureError> {
    let row_len = (width as usize).saturating_mul(RGBA);
    let mut pixels = vec![0; row_len.saturating_mul(height as usize)];
    if row_len > 0 {
        pixels
            .par_chunks_mut(row_len)
            .enumerate()
            .for_each(|(y, out)| row(y, out));
    }
    RgbaImage::new(width, height, pixels)
}

/// Returns the colour of a texel with the diffuse texel `diffuse` and the specular texel
/// `specular`, both as the maps store them: the tinted diffuse colour moved towards the tinted
/// specular colour by its metalness, encoded.
fn metal_mix(diffuse: [u8; 3], specular: [u8; 3], tints: Tints) -> [u8; 3] {
    let reflected = specular_of(specular, tints.specular);
    let metal = metal_share(reflected);
    let [red, green, blue] = diffuse.map(linear);
    let [tr, tg, tb] = tints.diffuse;
    let own = [red * tr, green * tg, blue * tb];
    let mix = |channel: usize| {
        let from = own.get(channel).copied().unwrap_or_default();
        let to = reflected.get(channel).copied().unwrap_or_default();
        encode((to - from).mul_add(metal, from))
    };
    [mix(0), mix(1), mix(2)]
}

/// Returns the base colour image: `diffuse` as it is, or with a specular map `specular` (sampled
/// to the size of `diffuse`) its tinted colour with each texel moved towards the tinted specular
/// colour by that texel's metalness, because a metal's colour is its specular colour and its
/// diffuse colour is dark. The tints are applied in linear light only when there is a specular
/// map; without, the material's diffuse colour multiplies the texture as a factor. The alpha
/// channel is kept only when `keep_alpha` is set, otherwise opaque.
pub(crate) fn base_colour(
    diffuse: &RgbaImage,
    specular: Option<&RgbaImage>,
    tints: Tints,
    keep_alpha: bool,
) -> Result<RgbaImage, TextureError> {
    let size = (diffuse.width() as usize, diffuse.height() as usize);
    fill(diffuse.width(), diffuse.height(), |y, out| {
        for (x, target) in out.as_chunks_mut::<RGBA>().0.iter_mut().enumerate() {
            let [red, green, blue, alpha] = texel(diffuse, x, y);
            let [red, green, blue] = specular.map_or([red, green, blue], |specular| {
                let [sr, sg, sb, _] = sampled(specular, (x, y), size);
                metal_mix([red, green, blue], [sr, sg, sb], tints)
            });
            *target = [red, green, blue, if keep_alpha { alpha } else { OPAQUE }];
        }
    })
}

/// Returns the base colour image of the opacity map `opacity`: white, with the red channel of the
/// map as the alpha channel, so that the material's diffuse colour is the colour of the surface.
///
/// With `decal`, the colours of the red, green and blue masks, the image is painted instead: the
/// red colour, with the green and the blue colour blended over it by their masks, and the
/// strongest mask as the alpha channel.
pub(crate) fn opacity_colour(
    opacity: &RgbaImage,
    decal: Option<[[f32; 3]; 3]>,
) -> Result<RgbaImage, TextureError> {
    fill(opacity.width(), opacity.height(), |y, out| {
        for (x, target) in out.as_chunks_mut::<RGBA>().0.iter_mut().enumerate() {
            let [red, green, blue, _] = texel(opacity, x, y);
            *target = decal.map_or([OPAQUE, OPAQUE, OPAQUE, red], |colours| {
                decal_texel(colours, [red, green, blue])
            });
        }
    })
}

/// Returns the texel of a stencil decal with the masks `masks` painted in the linear `colours`.
fn decal_texel(colours: [[f32; 3]; 3], masks: [u8; 3]) -> [u8; RGBA] {
    let [first, second, third] = colours;
    let [red, green, blue] = masks;
    let blend = |under: [f32; 3], over: [f32; 3], mask: u8| {
        let share = f32::from(mask) / f32::from(OPAQUE);
        let [low_red, low_green, low_blue] = under;
        let [high_red, high_green, high_blue] = over;
        [
            share.mul_add(high_red - low_red, low_red),
            share.mul_add(high_green - low_green, low_green),
            share.mul_add(high_blue - low_blue, low_blue),
        ]
    };
    let painted = blend(blend(first, second, green), third, blue);
    let [r, g, b] = painted.map(encode);
    [r, g, b, red.max(green).max(blue)]
}

/// Returns the normal map image of the engine's normal map `normal`: X and the reconstructed Z as
/// they are, Y inverted from the engine's convention (Y points down the image) to glTF's (up),
/// opaque.
pub(crate) fn normal_map(normal: &RgbaImage) -> Result<RgbaImage, TextureError> {
    fill(normal.width(), normal.height(), |y, out| {
        for (x, target) in out.as_chunks_mut::<RGBA>().0.iter_mut().enumerate() {
            let [red, green, blue, _] = texel(normal, x, y);
            *target = [red, OPAQUE - green, blue, OPAQUE];
        }
    })
}

/// The metallic-roughness image and what it carries.
pub(crate) struct MetallicRoughness {
    pub(crate) image: RgbaImage,
    pub(crate) roughness: bool,
    pub(crate) metallic: bool,
}

/// Returns the metallic-roughness image, glTF's roughness in green and metalness in blue, from the
/// smoothness in the alpha channel of `gloss` (roughness is its inverse) and the metalness of the
/// specular map `specular` tinted with `tint`; a channel without source is full, for the
/// material's factor to scale. The image has the size of `gloss`, or of `specular` without it;
/// `None` when both are absent.
pub(crate) fn metallic_roughness(
    gloss: Option<&RgbaImage>,
    specular: Option<&RgbaImage>,
    tint: [f32; 3],
) -> Result<Option<MetallicRoughness>, TextureError> {
    let Some(size) = gloss.or(specular) else {
        return Ok(None);
    };
    let dimensions = (size.width() as usize, size.height() as usize);
    let image = fill(size.width(), size.height(), |y, out| {
        for (x, target) in out.as_chunks_mut::<RGBA>().0.iter_mut().enumerate() {
            let roughness = gloss.map_or(OPAQUE, |gloss| {
                OPAQUE - sampled(gloss, (x, y), dimensions)[3]
            });
            let metallic = specular.map_or(OPAQUE, |specular| {
                let [red, green, blue, _] = sampled(specular, (x, y), dimensions);
                encode_share(metal_share(specular_of([red, green, blue], tint)))
            });
            *target = [OPAQUE, roughness, metallic, OPAQUE];
        }
    })?;
    Ok(Some(MetallicRoughness {
        image,
        roughness: gloss.is_some(),
        metallic: specular.is_some(),
    }))
}

fn encode_share(share: f32) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the share is between 0 and 1"
    )]
    let byte = (share.clamp(0.0, 1.0) * 255.0).round() as u8;
    byte
}
