// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Observed reads of decoded texture previews and their source headers.

use p4kx_texture::{MipByteSpans, RgbaImage, TextureError, sheet_columns, tile};

use crate::texture_job::{Member, Role, role};
use crate::texture_preview_png::input::PreviewInputs;
use crate::{ConversionError, PreviewPngEvidence, SourceEvidenceError, SourceUses, Texture};

pub use crate::dds_source_info::{DdsSourceWord, raw_dds_source_words};
pub use crate::model_textures::ObservedModelTexture;
pub use crate::texture_preview_png::input::PreviewHeaderEvidence;

/// A decoded preview and its exact original DDS inputs.
#[derive(Debug)]
pub struct ObservedPreviewImage<'a> {
    level: u32,
    pub(crate) image: RgbaImage,
    pub(crate) inputs: PreviewInputs<'a, MipByteSpans>,
}

/// The original inputs retained independently of a decoded texture image.
#[derive(Debug)]
pub struct ObservedMipInput<'a> {
    level: u32,
    inputs: PreviewInputs<'a, MipByteSpans>,
}

impl ObservedMipInput<'_> {
    /// Registers the actual selected mip inputs in one evidence inventory.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn evidence(&self, uses: &SourceUses) -> Result<PreviewPngEvidence, SourceEvidenceError> {
        self.inputs.register(uses, self.level)
    }
}

impl<'a> ObservedPreviewImage<'a> {
    /// Returns the decoded image.
    #[must_use]
    pub const fn image(&self) -> &RgbaImage {
        &self.image
    }

    /// Takes the decoded image.
    #[must_use]
    pub fn into_image(self) -> RgbaImage {
        self.image
    }

    /// Takes the pixels and their independent observed source.
    #[must_use]
    pub fn into_parts(self) -> (RgbaImage, ObservedMipInput<'a>) {
        (
            self.image,
            ObservedMipInput {
                level: self.level,
                inputs: self.inputs,
            },
        )
    }

    /// Registers the actual decoded inputs in one evidence inventory.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn evidence(&self, uses: &SourceUses) -> Result<PreviewPngEvidence, SourceEvidenceError> {
        self.inputs.register(uses, self.level)
    }
}

impl<'a> Texture<'a> {
    /// Returns whether the Entry at `path` is the header of a texture or of an alpha tail, the
    /// Entries a texture is opened from; its parts `.dds.N` and `.dds.Na` are not.
    #[must_use]
    pub fn is_header_path(path: &str) -> bool {
        matches!(
            role(path),
            Some(Role::Header | Role::Member(_, Member::AlphaHeader))
        )
    }

    /// Reads and decodes mip `level` as the one image a preview shows: the mip itself with the
    /// alpha tail merged in, a cubemap's six faces laid out as a horizontal cross, or the images
    /// of an array, a volume or several cubemaps tiled on a sheet, element by element and slice
    /// by slice from the top left. The size of the image is
    /// [`DdsHeader::preview_size`](p4kx_texture::DdsHeader::preview_size).
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Texture::decode_surfaces`], and a [`ConversionError`] wrapping a
    /// texture error when the cross or the sheet cannot be laid out.
    pub fn decode_preview(&self, level: u32) -> Result<RgbaImage, ConversionError> {
        self.decode_preview_observed(level)
            .map(ObservedPreviewImage::into_image)
    }

    /// Decodes the complete mip and retains the actual original source spans without encoding it.
    ///
    /// # Errors
    ///
    /// Returns the errors of a decoded preview.
    pub fn decode_preview_observed(
        &self,
        level: u32,
    ) -> Result<ObservedPreviewImage<'a>, ConversionError> {
        self.decode_preview_selected(level, None)
    }

    pub(crate) fn decode_mip_observed(
        &self,
        level: u32,
    ) -> Result<ObservedPreviewImage<'a>, ConversionError> {
        let (image, inputs) = self.decode_mip_with_inputs(level)?;
        Ok(ObservedPreviewImage {
            level,
            image,
            inputs,
        })
    }

    /// Binds the header already read when opening this texture.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence inventory was interrupted.
    pub fn preview_header_evidence(
        &self,
        uses: &SourceUses,
    ) -> Result<PreviewHeaderEvidence, SourceEvidenceError> {
        PreviewHeaderEvidence::of_header(uses, self.parts(), self.raw_header())
    }

    pub(crate) fn decode_preview_selected(
        &self,
        level: u32,
        face: Option<usize>,
    ) -> Result<ObservedPreviewImage<'a>, ConversionError> {
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
        Ok(ObservedPreviewImage {
            level,
            image,
            inputs,
        })
    }

    fn layout_preview(&self, images: Vec<RgbaImage>) -> Result<RgbaImage, ConversionError> {
        match <[RgbaImage; 1]>::try_from(self.layout_cube_crosses(images, true)?) {
            Ok([only]) => Ok(only),
            Err(units) => {
                let count = units.len() as u64;
                let overflow = || {
                    ConversionError::of_texture(
                        self.name(),
                        TextureError::InvalidHeader("the texture size overflows"),
                    )
                };
                let columns = u32::try_from(sheet_columns(count)).map_err(|_| overflow())?;
                tile(&units, columns)
                    .map_err(|error| ConversionError::of_texture(self.name(), error))
            }
        }
    }
}
