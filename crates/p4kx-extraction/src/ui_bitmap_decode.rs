// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gfx::{Assets, Bitmap, RgbaImage};
use p4kx_texture::MipByteSpans;

use crate::texture_preview_png::input::PreviewInputs;
use crate::ui_external::ExternalPath;
use crate::{ConversionError, Texture, UiMovie};

pub(crate) enum BitmapPixels<'archive> {
    Embedded {
        used_jpeg_tables: bool,
    },
    External {
        selection: ExternalPath,
        inputs: Box<PreviewInputs<'archive, MipByteSpans>>,
    },
}

pub(crate) struct BitmapRead<'archive> {
    pub(crate) order: usize,
    pub(crate) pixels: BitmapPixels<'archive>,
}

pub(crate) struct DecodedPixels<'archive> {
    pub(crate) image: RgbaImage,
    pub(crate) pixels: BitmapPixels<'archive>,
}

impl<'archive> UiMovie<'archive> {
    pub(crate) fn embedded_bitmap_pixels(
        &self,
        assets: &Assets<'_>,
        bitmap: &Bitmap<'_>,
    ) -> Result<DecodedPixels<'archive>, ConversionError> {
        let decoded = assets
            .decode_bitmap_observed(bitmap)
            .map_err(|source| self.asset_error(bitmap.id, source))?;
        Ok(DecodedPixels {
            image: decoded.image,
            pixels: BitmapPixels::Embedded {
                used_jpeg_tables: decoded.used_jpeg_tables,
            },
        })
    }

    pub(crate) fn external_bitmap_pixels(
        &self,
        id: u16,
        texture: &Texture<'archive>,
        selection: ExternalPath,
    ) -> Result<DecodedPixels<'archive>, ConversionError> {
        let (decoded, inputs) = texture.decode_mip_with_inputs(0)?;
        let (width, height) = (decoded.width(), decoded.height());
        let image = RgbaImage::from_pixels(width, height, decoded.into_pixels())
            .map_err(|source| self.asset_error(id, source))?;
        Ok(DecodedPixels {
            image,
            pixels: BitmapPixels::External {
                selection,
                inputs: Box::new(inputs),
            },
        })
    }
}
