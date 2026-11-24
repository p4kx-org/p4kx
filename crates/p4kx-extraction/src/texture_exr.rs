// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_texture::{HdrImage, TextureError, cube_cross_hdr, write_exr};
use rayon::prelude::*;

use crate::output_file::{write_error, write_replacing_protected};
use crate::texture_preview_png::input::PreviewInputs;
use crate::{ConversionError, CubemapLayout, Texture, TextureImageFormat, TextureImagePlan};

#[path = "texture_exr_receipt.rs"]
mod receipt;

pub use crate::textures::image_receipt::PngOrigins as ExrOrigins;

const CUBE_FACES: usize = 6;

#[derive(Clone, Copy)]
enum Publication {
    Ordinary,
    WithOrigins,
}

/// What an OpenEXR export wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExrReport {
    /// The exported mip level, 0 being the largest.
    pub level: u32,
    /// Width of each image in pixels.
    pub width: u32,
    /// Height of each image in pixels.
    pub height: u32,
    /// The written files: one, or six faces of a cubemap.
    pub files: Vec<PathBuf>,
}

impl Texture<'_> {
    /// Reads and decodes mip `level`, 0 being the largest, of a high-range texture to linear
    /// floating-point RGB without clipping its range.
    ///
    /// # Errors
    ///
    /// Returns a [`ConversionError`] wrapping [`TextureError::UnsupportedImage`] when the mip
    /// holds several images, and the errors of [`Texture::decode_surfaces_hdr`].
    pub fn decode_mip_hdr(&self, level: u32) -> Result<HdrImage, ConversionError> {
        self.check_high_range()?;
        self.read_mip(level)?
            .decode_hdr()
            .map_err(|error| ConversionError::of_texture(self.name(), error))
    }

    /// Reads and decodes every image of mip `level` of a high-range texture, a cubemap's six
    /// faces in DDS order, to linear floating-point RGB. An alpha tail is not read, as the
    /// images have no alpha channel.
    ///
    /// # Errors
    ///
    /// Returns a [`ConversionError`] wrapping [`TextureError::NotHighRange`] before reading
    /// anything when the format is not high-range, and the errors of [`Texture::read_mip`].
    pub fn decode_surfaces_hdr(&self, level: u32) -> Result<Vec<HdrImage>, ConversionError> {
        self.check_high_range()?;
        self.read_mip(level)?
            .decode_surfaces_hdr()
            .map_err(|error| ConversionError::of_texture(self.name(), error))
    }

    /// Decodes mip `level`, 0 being the largest, of a high-range texture and writes it to the
    /// file `out` as OpenEXR with 32-bit float RGB, replacing it; a cubemap is written as six
    /// face files, an array as one file per element and a volume as one file per slice.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Texture::exr_to_file_as`].
    pub fn exr_to_file(&self, out: &Path, level: u32) -> Result<ExrReport, ConversionError> {
        self.exr_to_file_as(out, level, CubemapLayout::Faces)
    }

    /// Decodes mip `level` of a high-range texture and writes it as OpenEXR to the file `out`,
    /// or a cubemap as `layout` says; existing files are replaced. Everything is decoded before
    /// anything is written, and each file is written under a temporary name first.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::OutputIsInput`] when a file to write is the header or one of
    /// the parts, [`ConversionError::Write`] when a file cannot be written, a
    /// [`ConversionError`] wrapping [`TextureError::NotHighRange`] when the format is not
    /// high-range, and the errors of [`Texture::decode_surfaces_hdr`].
    pub fn exr_to_file_as(
        &self,
        out: &Path,
        level: u32,
        layout: CubemapLayout,
    ) -> Result<ExrReport, ConversionError> {
        self.write_exr_images(out, level, layout, Publication::Ordinary)
            .map(|(report, _)| report)
    }

    /// Writes OpenEXR images with byte-bound receipts and the origins of the report fields.
    ///
    /// # Errors
    /// Returns an input, output or evidence error.
    pub fn exr_to_file_with_origins(
        &self,
        out: &Path,
        level: u32,
        layout: CubemapLayout,
    ) -> Result<(ExrReport, ExrOrigins), ConversionError> {
        let (report, origins) =
            self.write_exr_images(out, level, layout, Publication::WithOrigins)?;
        let origins = origins
            .ok_or_else(|| write_error(out, std::io::Error::other("OpenEXR origin is missing")))?;
        Ok((report, origins))
    }

    fn write_exr_images(
        &self,
        out: &Path,
        level: u32,
        layout: CubemapLayout,
        publication: Publication,
    ) -> Result<(ExrReport, Option<ExrOrigins>), ConversionError> {
        let files: Vec<PathBuf> =
            TextureImagePlan::new(self.header(), level, layout, TextureImageFormat::Exr)
                .map_err(|error| ConversionError::of_texture(self.name(), error))?
                .map(|image| image.path(out))
                .collect();
        for file in &files {
            self.refuse_input(file)?;
        }
        let (surfaces, inputs) = match publication {
            Publication::Ordinary => (self.decode_surfaces_hdr(level)?, None),
            Publication::WithOrigins => {
                self.check_high_range()?;
                let (read, color) = self.read_color_mip(level)?;
                let surfaces = read
                    .mip
                    .decode_surfaces_hdr()
                    .map_err(|error| ConversionError::of_texture(self.name(), error))?;
                (surfaces, Some(PreviewInputs { color, alpha: None }))
            }
        };
        let cross = layout == CubemapLayout::Cross && self.header().faces() as usize == CUBE_FACES;
        let images = self.layout_exr_images(surfaces, cross)?;
        if images.len() != files.len() {
            return Err(ConversionError::of_texture(
                self.name(),
                TextureError::UnsupportedImage {
                    images: self.header().images(level),
                },
            ));
        }
        let origins = if let Some(inputs) = inputs {
            Some(receipt::publish(
                self, &inputs, out, level, cross, &files, &images,
            )?)
        } else {
            files
                .par_iter()
                .zip(&images)
                .try_for_each(|(file, image)| self.write_exr_file(file, image))?;
            None
        };
        let (width, height) = images
            .first()
            .map_or((0, 0), |image| (image.width(), image.height()));
        Ok((
            ExrReport {
                level,
                width,
                height,
                files,
            },
            origins,
        ))
    }

    fn layout_exr_images(
        &self,
        surfaces: Vec<HdrImage>,
        cross: bool,
    ) -> Result<Vec<HdrImage>, ConversionError> {
        if !cross {
            return Ok(surfaces);
        }
        surfaces
            .chunks(CUBE_FACES)
            .map(|faces| {
                cube_cross_hdr(faces)
                    .map_err(|error| ConversionError::of_texture(self.name(), error))
            })
            .collect()
    }

    fn write_exr_file(&self, file: &Path, image: &HdrImage) -> Result<(), ConversionError> {
        write_replacing_protected(file, self.write_protection()?, None, |writer| {
            write_exr(image, writer)
                .map_err(|error| ConversionError::of_texture(self.name(), error))
        })
    }

    fn check_high_range(&self) -> Result<(), ConversionError> {
        let format = self.header().format();
        if format.is_high_range() {
            Ok(())
        } else {
            Err(ConversionError::of_texture(
                self.name(),
                TextureError::NotHighRange(format),
            ))
        }
    }
}
