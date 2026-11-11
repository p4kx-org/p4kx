// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(crate) mod input;

use p4kx_texture::{MipByteSpans, TextureError, write_png};

use self::input::PreviewInputs;
use crate::{ConversionError, SourceEvidenceError, SourceUses, Texture};

pub use self::input::{PreviewPngEvidence, PreviewTextureEvidence};

/// A preview of one mip of a texture, encoded as an 8-bit RGBA PNG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewPng {
    /// The mip level shown, 0 being the largest.
    pub level: u32,
    /// The width of the image in pixels.
    pub width: u32,
    /// The height of the image in pixels.
    pub height: u32,
    /// The PNG file.
    pub png: Vec<u8>,
}

/// A PNG preview with the actual DDS inputs used to produce it.
#[derive(Debug)]
#[non_exhaustive]
pub struct ObservedPreviewPng<'a> {
    image: PreviewPng,
    inputs: PreviewInputs<'a, MipByteSpans>,
}

impl ObservedPreviewPng<'_> {
    /// Returns the encoded image and its dimensions.
    #[must_use]
    pub const fn image(&self) -> &PreviewPng {
        &self.image
    }

    /// Takes the encoded image and its dimensions.
    #[must_use]
    pub fn into_image(self) -> PreviewPng {
        self.image
    }

    /// Registers the selected color and alpha inputs in an output evidence inventory.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn evidence(&self, uses: &SourceUses) -> Result<PreviewPngEvidence, SourceEvidenceError> {
        self.inputs.register(uses, self.image.level)
    }
}

impl<'a> Texture<'a> {
    /// Decodes mip `level` with its alpha tail merged in and encodes it as one PNG: the image
    /// [`Texture::decode_preview`] gives, or with `face` the one face of a cubemap, counted in DDS
    /// order +X, −X, +Y, −Y, +Z, −Z.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Texture::decode_preview`] and [`Texture::decode_surfaces`], and a
    /// [`ConversionError`] wrapping [`TextureError::UnsupportedImage`] when the texture has no
    /// such face.
    pub fn preview_png(
        &self,
        level: u32,
        face: Option<usize>,
    ) -> Result<PreviewPng, ConversionError> {
        self.preview_png_observed(level, face)
            .map(ObservedPreviewPng::into_image)
    }

    /// Encodes one preview and retains its actual header and mip byte sources.
    ///
    /// # Errors
    ///
    /// Returns the errors of a PNG preview.
    pub fn preview_png_observed(
        &self,
        level: u32,
        face: Option<usize>,
    ) -> Result<ObservedPreviewPng<'a>, ConversionError> {
        let (mut images, inputs) = self.decode_surfaces_with_inputs(level)?;
        let bad_face = || {
            ConversionError::of_texture(
                self.name(),
                TextureError::UnsupportedImage {
                    images: self.header().images(level),
                },
            )
        };
        let image = match face {
            None => self.layout_preview(images)?,
            Some(face) => {
                if face >= images.len() || self.header().faces() < 2 {
                    return Err(bad_face());
                }
                images.swap_remove(face)
            }
        };
        let inputs = inputs
            .selected(face.map(|index| index as u64))
            .ok_or_else(bad_face)?;
        let mut png = Vec::new();
        write_png(&image, &mut png)
            .map_err(|error| ConversionError::of_texture(self.name(), error))?;
        Ok(ObservedPreviewPng {
            image: PreviewPng {
                level,
                width: image.width(),
                height: image.height(),
                png,
            },
            inputs,
        })
    }
}
