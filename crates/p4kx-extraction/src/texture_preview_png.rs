// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod evidence;
pub(crate) mod input;

use p4kx_texture::{DdsPngEncoding, MipByteSpans, write_png_with_transfer};

use self::input::PreviewInputs;
use crate::{ConversionError, SourceEvidenceError, SourceUses, Texture};

pub use self::evidence::{PreviewPngEvidence, PreviewTextureEvidence};

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
    encoding: DdsPngEncoding,
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
        let mut evidence = self.inputs.register(uses, self.image.level)?;
        evidence.png_encoding = Some(self.encoding);
        Ok(evidence)
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
    /// [`ConversionError`] when the texture has no such face.
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
        let observed = self.decode_preview_selected(level, face)?;
        let image = observed.image;
        let mut png = Vec::new();
        let encoding = observed
            .inputs
            .png_encoding()
            .map_err(|error| ConversionError::of_texture(self.name(), error))?;
        write_png_with_transfer(&image, encoding.transfer, &mut png)
            .map_err(|error| ConversionError::of_texture(self.name(), error))?;
        Ok(ObservedPreviewPng {
            image: PreviewPng {
                level,
                width: image.width(),
                height: image.height(),
                png,
            },
            encoding,
            inputs: observed.inputs,
        })
    }
}
