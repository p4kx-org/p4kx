// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_texture::{
    DdsAlphaAssociation, DdsAlphaTransform, DdsPngEncoding, RgbaImage, TextureError, cube_cross,
    write_png_with_transfer,
};
use rayon::prelude::*;

#[path = "texture_png_receipt.rs"]
mod receipt;

pub use self::receipt::PngOrigins;
use crate::output_file::{write_error, write_replacing_protected};
use crate::{ConversionError, Texture, TextureImageFormat, TextureImagePlan};

const CUBE_FACES: usize = 6;

/// The names of a cubemap's faces in DDS order +X, âˆ’X, +Y, âˆ’Y, +Z, âˆ’Z, as the face files of
/// [`CubemapLayout::Faces`] carry them.
pub const CUBE_FACE_NAMES: [&str; 6] = ["posx", "negx", "posy", "negy", "posz", "negz"];

/// How a PNG export writes a cubemap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CubemapLayout {
    /// One file per face, `<stem>_posx.<extension>` â€¦ `<stem>_negz.<extension>` next to the
    /// requested file.
    #[default]
    Faces,
    /// One horizontal cross of all six faces in the requested file.
    Cross,
}

/// What a PNG export wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PngReport {
    /// The exported mip level, 0 being the largest.
    pub level: u32,
    /// Width of each PNG in pixels.
    pub width: u32,
    /// Height of each PNG in pixels.
    pub height: u32,
    /// The written files: one, or six faces of a cubemap.
    pub files: Vec<PathBuf>,
    /// Whether an alpha tail was merged into the alpha channel.
    pub alpha_tail: bool,
    /// The transfer and channel processing of the encoded PNG samples.
    pub encoding: DdsPngEncoding,
    /// Original declaration from the DDS colour header.
    pub alpha_association: DdsAlphaAssociation,
    /// RGB unassociation applied to produce straight colour.
    pub alpha_transform: DdsAlphaTransform,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Publication {
    Ordinary,
    Receipts,
}

impl Texture<'_> {
    /// Decodes mip `level`, 0 being the largest, and writes it to the file `out` as an 8-bit RGBA
    /// PNG, replacing it; a cubemap is written as six face files, an array as one file per
    /// element and a volume as one file per slice.
    ///
    /// Everything is decoded before anything is written, and each file is written under a
    /// temporary name first, so a failed file leaves no partial file behind; when writing one
    /// image fails, images already written stay.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Texture::png_to_file_as`].
    pub fn png_to_file(&self, out: &Path, level: u32) -> Result<PngReport, ConversionError> {
        self.png_to_file_as(out, level, CubemapLayout::Faces)
    }

    /// Decodes mip `level`, 0 being the largest, with its alpha tail, and writes it as 8-bit RGBA
    /// PNG to the file `out`; existing files are replaced.
    ///
    /// A texture of several images is written as one file per image next to `out`, its stem
    /// followed by `_layerN` for an array element, a cubemap face's name or `_faceN`, and
    /// `_sliceN` for a volume slice, in that order, numbered to the width of the last number.
    /// With [`CubemapLayout::Cross`], a cubemap's six faces are one cross per array element.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::OutputIsInput`] when a file to write is the header or one of
    /// the parts, [`ConversionError::Write`] when a file cannot be written, and the errors of
    /// [`Texture::decode_surfaces`].
    pub fn png_to_file_as(
        &self,
        out: &Path,
        level: u32,
        layout: CubemapLayout,
    ) -> Result<PngReport, ConversionError> {
        let (report, _) = self.write_png_images(out, level, layout, Publication::Ordinary)?;
        Ok(report)
    }

    /// Writes PNG files and their byte-bound receipts with exact transfer evidence.
    ///
    /// # Errors
    /// Returns an input, output or evidence error.
    pub fn png_to_file_with_origins(
        &self,
        out: &Path,
        level: u32,
        layout: CubemapLayout,
    ) -> Result<(PngReport, PngOrigins), ConversionError> {
        let (report, origins) = self.write_png_images(out, level, layout, Publication::Receipts)?;
        let origins = origins
            .ok_or_else(|| write_error(out, std::io::Error::other("PNG origin is missing")))?;
        Ok((report, origins))
    }

    fn write_png_images(
        &self,
        out: &Path,
        level: u32,
        layout: CubemapLayout,
        publication: Publication,
    ) -> Result<(PngReport, Option<PngOrigins>), ConversionError> {
        let cross = layout == CubemapLayout::Cross;
        let files: Vec<PathBuf> =
            TextureImagePlan::new(self.header(), level, layout, TextureImageFormat::Png)
                .map_err(|error| ConversionError::of_texture(self.name(), error))?
                .map(|image| image.path(out))
                .collect();
        for file in &files {
            self.refuse_input(file)?;
        }
        let (surfaces, inputs, alpha_transform) = self.decode_png_surfaces(level)?;
        let encoding = inputs
            .png_encoding()
            .map_err(|error| ConversionError::of_texture(self.name(), error))?;
        let images = self.layout_cube_crosses(surfaces, cross)?;
        if images.len() != files.len() {
            return Err(ConversionError::of_texture(
                self.name(),
                TextureError::UnsupportedImage {
                    images: self.header().images(level),
                },
            ));
        }
        let origins = if publication == Publication::Receipts {
            let plan = receipt::PreparedPng::new(
                self,
                &inputs,
                layout,
                (encoding, alpha_transform),
                &files,
                out,
            )?;
            plan.write_images(&inputs, level, (encoding, alpha_transform), &files, &images)?;
            Some(plan.finish())
        } else {
            files
                .par_iter()
                .zip(&images)
                .try_for_each(|(file, image)| self.write_png_file(file, image, encoding))?;
            None
        };
        let (width, height) = images
            .first()
            .map_or((0, 0), |image| (image.width(), image.height()));
        Ok((
            PngReport {
                level,
                width,
                height,
                files,
                alpha_tail: self.has_alpha_tail(),
                encoding,
                alpha_association: inputs.source_alpha().0,
                alpha_transform,
            },
            origins,
        ))
    }

    fn write_png_file(
        &self,
        out: &Path,
        image: &RgbaImage,
        encoding: DdsPngEncoding,
    ) -> Result<(), ConversionError> {
        write_replacing_protected(out, self.write_protection()?, None, |writer| {
            write_png_with_transfer(image, encoding.transfer, writer)
                .map_err(|error| ConversionError::of_texture(self.name(), error))
        })
    }
}

impl Texture<'_> {
    pub(crate) fn layout_cube_crosses(
        &self,
        images: Vec<RgbaImage>,
        cross: bool,
    ) -> Result<Vec<RgbaImage>, ConversionError> {
        if !(cross && self.header().faces() as usize == CUBE_FACES) {
            return Ok(images);
        }
        images
            .chunks(CUBE_FACES)
            .map(|faces| {
                cube_cross(faces).map_err(|error| ConversionError::of_texture(self.name(), error))
            })
            .collect()
    }
}
