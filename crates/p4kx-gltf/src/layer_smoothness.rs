// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_texture::TextureError;

use crate::layer_sampling::{self, Tile, TileChannels};
use crate::texture_loader::LoadedTexture;
use crate::texture_pixels;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SmoothnessChannel {
    Alpha,
    Red,
}

pub(crate) struct SmoothnessMap<'a> {
    tile: Tile<'a>,
    repeats: [f32; 2],
    channel: SmoothnessChannel,
}

impl<'a> SmoothnessMap<'a> {
    pub(crate) fn new(
        texture: &'a LoadedTexture,
        repeats: [f32; 2],
        size: (usize, usize),
        channel: SmoothnessChannel,
    ) -> Result<Self, TextureError> {
        let channels = match channel {
            SmoothnessChannel::Alpha => TileChannels::Rgba,
            SmoothnessChannel::Red => TileChannels::Rgb,
        };
        let tile =
            layer_sampling::tile_of(Cow::Borrowed(&texture.image), repeats, size, None, channels)?;
        Ok(Self {
            tile,
            repeats,
            channel,
        })
    }

    pub(crate) fn sample(&self, uv: (f32, f32)) -> f32 {
        match self.channel {
            SmoothnessChannel::Alpha => {
                layer_sampling::sample_alpha(self.tile.image(), self.repeats, uv)
            }
            SmoothnessChannel::Red => {
                let [red, _, _] = layer_sampling::sample(
                    self.tile.image(),
                    self.repeats,
                    uv,
                    texture_pixels::unit,
                );
                red
            }
        }
    }
}

#[cfg(test)]
mod tests;
