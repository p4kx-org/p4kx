// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_archive::Archive;
use p4kx_texture::{
    DdsHeader, DdsRawHeader, LocatedMip, MergeReport, Mip, MipPayloadLocation, RgbaImage,
    TextureError, merge, merge_alpha, read_mip_with_location,
};

use crate::disk_parts::DiskParts;
use crate::output_file::{read_error, write_replacing_protected};
use crate::path_index::PathIndex;
use crate::source_protection::SourceProtection;
use crate::texture_parts::{Found, Parts};
use crate::texture_preview_png::input::{PreviewInputs, TexturePreviewInput};
use crate::{ConversionError, DdsSourceInfo, ExtractionError, UnknownEntries, texture_header};

/// A DDS texture whose header is an Entry of an Archive or a file on disk, with its sibling parts
/// `.1` . `.N` and any alpha tail `.a` with its parts `.1a` . `.Na` next to it.
#[derive(Debug)]
pub struct Texture<'a> {
    pub(super) protection: SourceProtection,
    name: String,
    header_size: u64,
    header: DdsHeader,
    raw_header: DdsRawHeader,
    pub(super) parts: Parts<'a>,
    pub(super) alpha: AlphaTail<'a>,
}

#[derive(Debug)]
pub(super) enum AlphaTail<'a> {
    Absent,
    Present(Parts<'a>),
    LookupFailed(DiskParts),
}

impl<'a> Texture<'a> {
    /// Opens the texture whose header is the Entry at `path`, matched with either separator and
    /// ignoring ASCII case, with its alpha tail when the Archive has one; a path ending in `.a`
    /// opens an alpha tail on its own. The alpha tail is only read where it is used.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::UnknownEntry`] when no Entry has that path, and another
    /// [`ConversionError`] when the Entry cannot be read or holds no DDS header.
    pub fn from_archive(archive: &'a Archive, path: &str) -> Result<Self, ConversionError> {
        let found = Parts::in_archive(archive, path)
            .ok_or_else(|| UnknownEntries(vec![path.to_owned()]))?;
        Self::from_found(found)
    }

    /// Opens the texture like [`Texture::from_archive`], finding the header, the parts and the
    /// alpha tail through `index` instead of scanning the Archive.
    ///
    /// # Errors
    ///
    /// As [`Texture::from_archive`].
    pub fn from_index(index: &PathIndex<'a>, path: &str) -> Result<Self, ConversionError> {
        let found =
            Parts::in_index(index, path).ok_or_else(|| UnknownEntries(vec![path.to_owned()]))?;
        Self::from_found(found)
    }

    fn from_found(found: Found<'a>) -> Result<Self, ConversionError> {
        Self::from_parts(found.colour, found.alpha)
    }

    /// Opens the texture of `parts` with the alpha tail of `alpha`, parsing its header.
    pub(crate) fn from_parts(
        parts: Parts<'a>,
        alpha: Option<Parts<'a>>,
    ) -> Result<Self, ConversionError> {
        let protection = SourceProtection::new(match &parts {
            Parts::Source { header, .. } => header.archive_path(),
            Parts::Files(_) => None,
        });
        let (name, header_size, header) = match &parts {
            Parts::Source { header, .. } => {
                let name = header.path().to_owned();
                let size = header.uncompressed_size();
                let reader = header
                    .open(false)
                    .map_err(|error| conversion_error(error, &name))?;
                (name.clone(), size, texture_header::read(reader, &name)?)
            }
            Parts::Files(parts) => {
                let path = parts.header();
                let name = parts.name().display().to_string();
                let file = parts
                    .open_header()
                    .map_err(|source| read_error(path, source))?;
                let size = file
                    .metadata()
                    .map_err(|source| read_error(path, source))?
                    .len();
                (name.clone(), size, texture_header::read(file, &name)?)
            }
        };
        Ok(Self {
            protection,
            name,
            header_size,
            header: header.normalized,
            raw_header: header.raw,
            parts,
            alpha: alpha.map_or(AlphaTail::Absent, AlphaTail::Present),
        })
    }

    /// Returns the name of the header: its Entry path or its path on disk.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the parsed DDS header.
    #[must_use]
    pub const fn header(&self) -> &DdsHeader {
        &self.header
    }

    /// Returns all captured source header words and separate normalized topology without I/O.
    #[must_use]
    pub fn source_metadata(&self) -> DdsSourceInfo {
        DdsSourceInfo::of_parts(&self.raw_header, &self.header, self.header_size)
    }

    /// Returns whether an alpha or smoothness header was found, without reading it.
    ///
    /// A failed disk lookup is reported by [`Self::alpha_tail`] when alpha is used.
    #[must_use]
    pub const fn has_alpha_tail(&self) -> bool {
        matches!(self.alpha, AlphaTail::Present(_))
    }

    /// Opens the alpha or smoothness tail as a texture of its own, or returns `None` when there
    /// is none.
    ///
    /// # Errors
    ///
    /// The inner result is a [`ConversionError`] naming the tail when it cannot be read or holds
    /// no DDS header; the texture itself stays usable.
    #[must_use]
    pub fn alpha_tail(&self) -> Option<Result<Self, ConversionError>> {
        let parts = match &self.alpha {
            AlphaTail::Absent => return None,
            AlphaTail::Present(parts) => parts.clone(),
            AlphaTail::LookupFailed(parts) => Parts::Files(parts.clone()),
        };
        Some(Self::from_parts(parts, None).map(|mut texture| {
            texture.protection = self.protection.clone();
            texture
        }))
    }

    /// Returns the number of sibling parts the header needs.
    ///
    /// # Errors
    ///
    /// Returns a [`TextureError`] when the format's size is unknown or the header's tail is not a
    /// run of whole mips.
    pub fn part_count(&self) -> Result<u32, TextureError> {
        self.header.part_count(self.header_size)
    }

    /// Writes the merged texture, without its alpha tail, as one standard DDS file to `out`.
    ///
    /// # Errors
    ///
    /// Returns a [`ConversionError`] naming the part when a part is missing, truncated, oversized
    /// or unreadable, and another one when the texture cannot be merged or `out` fails.
    pub fn merge_into(&self, out: &mut impl Write) -> Result<MergeReport, ConversionError> {
        let header = self.open_header()?;
        merge(header, self.header_size, &self.parts, out)
            .map_err(|error| ConversionError::of_texture(&self.name, error))
    }

    /// Writes the merged texture to the file `out`, replacing it; the file is written under a
    /// temporary name first, so a failed merge leaves nothing behind.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::OutputIsInput`] when `out` is the header or one of its parts,
    /// [`ConversionError::Read`] when a disk input cannot be resolved, opened or inspected,
    /// [`ConversionError::Texture`] wrapping [`TextureError::ReadHeader`] when a disk input header
    /// cannot be read for overwrite protection, [`ConversionError::Write`] when the file cannot
    /// be written, and the errors of
    /// [`Texture::merge_into`].
    pub fn merge_to_file(&self, out: &Path) -> Result<MergeReport, ConversionError> {
        self.refuse_input(out)?;
        write_replacing_protected(out, self.protection.context()?, None, |writer| {
            self.merge_into(writer)
        })
    }

    pub(crate) fn write_protection(
        &self,
    ) -> Result<Option<&crate::ArchiveWriteProtection>, ConversionError> {
        self.protection.context()
    }

    /// Reads mip `level`, 0 being the largest, from its part or from the header.
    ///
    /// # Errors
    ///
    /// Returns a [`ConversionError`] naming the part when the mip's part is missing, truncated,
    /// oversized or unreadable, and another one when the level does not exist.
    pub fn read_mip(&self, level: u32) -> Result<Mip, ConversionError> {
        self.read_mip_with_source(level).map(|read| read.mip)
    }

    fn read_mip_with_source(&self, level: u32) -> Result<LocatedMip, ConversionError> {
        let header = self.open_header()?;
        read_mip_with_location(header, self.header_size, &self.parts, level)
            .map_err(|error| ConversionError::of_texture(&self.name, error))
    }

    /// Reads and decodes mip `level`, 0 being the largest, to 8-bit RGBA with the alpha tail
    /// merged into the alpha channel.
    ///
    /// # Errors
    ///
    /// Returns a [`ConversionError`] wrapping [`TextureError::UnsupportedImage`] when the mip
    /// holds several images, and the errors of [`Texture::decode_surfaces`].
    pub fn decode_mip(&self, level: u32) -> Result<RgbaImage, ConversionError> {
        let several = || {
            ConversionError::of_texture(
                &self.name,
                TextureError::UnsupportedImage {
                    images: self.header.images(level),
                },
            )
        };
        if self.header.images(level) > 1 {
            return Err(several());
        }
        let mut images = self.decode_surfaces(level)?;
        match (images.pop(), images.is_empty()) {
            (Some(image), true) => Ok(image),
            _ => Err(several()),
        }
    }

    /// Reads and decodes every image of mip `level`, a cubemap's six faces in DDS order, an
    /// array's elements or a volume's slices, to 8-bit RGBA with the alpha tail merged into the
    /// alpha channel.
    ///
    /// # Errors
    ///
    /// Returns a [`ConversionError`] wrapping [`TextureError::UnsupportedDecode`] before reading
    /// any mip when the format of the texture or its alpha tail has no decoder, the error of
    /// opening the alpha tail when it is broken, [`TextureError::AlphaTailMips`] when the tail
    /// lacks the level, [`TextureError::AlphaTailSize`] when the tail does not fit the texture,
    /// and the errors of [`Texture::read_mip`]. A tail that cannot be used is an error, never
    /// silently dropped.
    pub fn decode_surfaces(&self, level: u32) -> Result<Vec<RgbaImage>, ConversionError> {
        self.decode_surfaces_with_inputs(level)
            .map(|(images, _)| images)
    }

    pub(crate) fn decode_surfaces_with_inputs(
        &self,
        level: u32,
    ) -> Result<(Vec<RgbaImage>, PreviewInputs<'a, MipPayloadLocation>), ConversionError> {
        let alpha = self.checked_alpha_tail(level)?;
        let read = self.read_mip_with_source(level)?;
        let mut images = self.decode_mip_data(&read.mip)?;
        let color = TexturePreviewInput::of_read(&self.parts, read.raw_header, read.payload);
        let alpha = Self::merge_alpha_input(alpha, level, &mut images)?;
        Ok((images, PreviewInputs { color, alpha }))
    }

    /// Returns the size of the header in bytes.
    pub(crate) const fn header_size(&self) -> u64 {
        self.header_size
    }

    /// Returns where the header and the parts are stored.
    pub(crate) const fn parts(&self) -> &Parts<'a> {
        &self.parts
    }

    /// Decodes every image of `mip`, read from this texture, to 8-bit RGBA without the alpha
    /// tail.
    pub(crate) fn decode_mip_data(&self, mip: &Mip) -> Result<Vec<RgbaImage>, ConversionError> {
        mip.decode_surfaces()
            .map_err(|error| ConversionError::of_texture(&self.name, error))
    }

    /// Checks that the texture and its alpha tail decode at `level` before any mip is read, and
    /// opens the tail.
    pub(crate) fn checked_alpha_tail(&self, level: u32) -> Result<Option<Self>, ConversionError> {
        self.check_decodable()?;
        let alpha = self.alpha_tail().transpose()?;
        if let Some(alpha) = &alpha {
            alpha.check_decodable()?;
            if level >= alpha.header.mip_count() && level < self.header.mip_count() {
                return Err(ConversionError::of_texture(
                    &alpha.name,
                    TextureError::AlphaTailMips {
                        level,
                        mip_count: alpha.header.mip_count(),
                    },
                ));
            }
        }
        Ok(alpha)
    }

    /// Merges mip `level` of the alpha tail `alpha` into the alpha channel of `images`.
    pub(crate) fn with_alpha_tail(
        alpha: Option<Self>,
        level: u32,
        mut images: Vec<RgbaImage>,
    ) -> Result<Vec<RgbaImage>, ConversionError> {
        Self::merge_alpha_input(alpha, level, &mut images)?;
        Ok(images)
    }

    fn merge_alpha_input(
        alpha: Option<Self>,
        level: u32,
        images: &mut [RgbaImage],
    ) -> Result<Option<TexturePreviewInput<'a, MipPayloadLocation>>, ConversionError> {
        let Some(alpha) = alpha else {
            return Ok(None);
        };
        let (tails, inputs) = alpha.decode_surfaces_with_inputs(level)?;
        alpha.merge_decoded_alpha(level, images, &tails)?;
        Ok(Some(inputs.color))
    }

    fn merge_decoded_alpha(
        &self,
        level: u32,
        images: &mut [RgbaImage],
        tails: &[RgbaImage],
    ) -> Result<(), ConversionError> {
        if tails.len() != images.len() {
            return Err(ConversionError::of_texture(
                &self.name,
                TextureError::UnsupportedImage {
                    images: self.header.images(level),
                },
            ));
        }
        for (image, tail) in images.iter_mut().zip(tails) {
            merge_alpha(image, tail, self.header.format())
                .map_err(|error| ConversionError::of_texture(&self.name, error))?;
        }
        Ok(())
    }

    fn check_decodable(&self) -> Result<(), ConversionError> {
        let format = self.header.format();
        if format.is_decodable() {
            Ok(())
        } else {
            Err(ConversionError::of_texture(
                &self.name,
                TextureError::UnsupportedDecode(format),
            ))
        }
    }
}

/// Returns `error` of opening the data of the file `name` as a [`ConversionError`].
pub(super) fn conversion_error(error: ExtractionError, name: &str) -> ConversionError {
    match error {
        ExtractionError::Archive(error) => ConversionError::Archive(error),
        other => ConversionError::Read {
            path: PathBuf::from(name),
            source: io::Error::other(other),
        },
    }
}
