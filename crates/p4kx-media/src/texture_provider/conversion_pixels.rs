// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::texture_codec::{self, DdsAlphaTransform, DdsPngEncoding, DdsTransfer};
use p4kx_extraction::{HdrImage, RgbaImage, TextureError, TextureImage, TextureImageFormat};

use super::super::conversion_mip::Loaded;
use super::super::conversion_payload::Alpha;

pub(super) enum Pixels {
    Png(RgbaImage),
    Exr(HdrImage),
}

impl Pixels {
    pub(super) fn decode(
        color: &Loaded,
        alpha: Option<&Loaded>,
        image: &TextureImage,
        index: u64,
        png: Option<DdsPngEncoding>,
        transform: DdsAlphaTransform,
    ) -> Result<Self, TextureError> {
        let color = &color.mip.mip;
        let format = color.header().format();
        let unavailable = || TextureError::UnsupportedImage {
            images: color.surfaces(),
        };
        let surface = color.surface(index).ok_or_else(unavailable)?;
        if image.format == TextureImageFormat::Exr {
            return texture_codec::decode_hdr(format, image.width, image.height, surface)
                .map(Self::Exr);
        }
        let mut rgba = texture_codec::decode(format, image.width, image.height, surface)?;
        if let Some(png) = png {
            transform.image(png.transfer, &mut rgba);
        }
        if let Some(alpha) = alpha {
            let alpha = &alpha.mip.mip;
            if alpha.surfaces() != color.surfaces() {
                return Err(TextureError::UnsupportedImage {
                    images: alpha.surfaces(),
                });
            }
            let surface = alpha.surface(index).ok_or_else(unavailable)?;
            let format = alpha.header().format();
            let decoded = texture_codec::decode(format, alpha.width(), alpha.height(), surface)?;
            texture_codec::merge_alpha(&mut rgba, &decoded, format)?;
        }
        Ok(Self::Png(rgba))
    }

    pub(super) fn encode(
        &self,
        transfer: DdsTransfer,
        sink: &mut dyn std::io::Write,
    ) -> Result<(), TextureError> {
        match self {
            Self::Png(image) => texture_codec::write_png_with_transfer(image, transfer, sink),
            Self::Exr(image) => texture_codec::write_exr(image, sink),
        }
    }

    pub(super) fn alpha(&self, split: bool) -> Alpha {
        match self {
            Self::Png(_) if split => Alpha::DecodedSplitChannel,
            Self::Png(_) => Alpha::DecodedChannels,
            Self::Exr(image) if image.alpha().is_some() => Alpha::DecodedChannels,
            Self::Exr(_) => Alpha::NoAlphaChannel,
        }
    }
}
