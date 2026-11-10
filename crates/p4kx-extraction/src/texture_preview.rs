// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::{RgbaImage, TextureError, sheet_columns, tile};

use crate::texture_job::{Member, Role, role};
use crate::{ConversionError, Texture};

impl Texture<'_> {
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
        match <[RgbaImage; 1]>::try_from(self.decode_images(level, true)?) {
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
