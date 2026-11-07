// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use p4kx_archive::Archive;
use p4kx_texture::{
    DdsHeader, MAX_HEADER_LEN, MergeReport, Mip, RgbaImage, TextureError, merge, merge_alpha,
    read_mip,
};

use crate::output_file::{read_error, write_replacing_protected};
use crate::path_index::PathIndex;
use crate::source_protection::SourceProtection;
use crate::texture_parts::{Found, PartReader, Parts, is_same_file};
use crate::{ConversionError, ExtractionError, UnknownEntries};

/// A DDS texture whose header is an Entry of an Archive or a file on disk, with its sibling parts
/// `.1` … `.N` and any alpha tail `.a` with its parts `.1a` … `.Na` next to it.
#[derive(Debug)]
pub struct Texture<'a> {
    protection: SourceProtection,
    name: String,
    header_size: u64,
    header: DdsHeader,
    parts: Parts<'a>,
    alpha: Option<Parts<'a>>,
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
            Parts::Files { .. } => None,
        });
        let (name, header_size, header) = match &parts {
            Parts::Source { header, .. } => {
                let name = header.path().to_owned();
                let size = header.uncompressed_size();
                let reader = header
                    .open(false)
                    .map_err(|error| conversion_error(error, &name))?;
                (name.clone(), size, parse_start(reader, &name)?)
            }
            Parts::Files { header: path, .. } => {
                let name = path.display().to_string();
                let file = File::open(path).map_err(|source| read_error(path, source))?;
                let size = file
                    .metadata()
                    .map_err(|source| read_error(path, source))?
                    .len();
                (name.clone(), size, parse_start(file, &name)?)
            }
        };
        Ok(Self {
            protection,
            name,
            header_size,
            header,
            parts,
            alpha,
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

    /// Returns whether the texture has an alpha or smoothness tail, without reading it.
    #[must_use]
    pub const fn has_alpha_tail(&self) -> bool {
        self.alpha.is_some()
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
        self.alpha.clone().map(|parts| {
            Self::from_parts(parts, None).map(|mut texture| {
                texture.protection = self.protection.clone();
                texture
            })
        })
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
    /// [`ConversionError::Write`] when the file cannot be written, and the errors of
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
        let header = self.open_header()?;
        read_mip(header, self.header_size, &self.parts, level)
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
        let alpha = self.checked_alpha_tail(level)?;
        let images = self.decode_mip_data(&self.read_mip(level)?)?;
        Self::with_alpha_tail(alpha, level, images)
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
        let Some(alpha) = alpha else {
            return Ok(images);
        };
        let tails = alpha.decode_surfaces(level)?;
        if tails.len() != images.len() {
            return Err(ConversionError::of_texture(
                &alpha.name,
                TextureError::UnsupportedImage {
                    images: alpha.header.images(level),
                },
            ));
        }
        for (image, tail) in images.iter_mut().zip(&tails) {
            merge_alpha(image, tail, alpha.header.format())
                .map_err(|error| ConversionError::of_texture(&alpha.name, error))?;
        }
        Ok(images)
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

    pub(crate) fn refuse_input(&self, out: &Path) -> Result<(), ConversionError> {
        if is_same_file(out, self.input_files()) {
            return Err(ConversionError::OutputIsInput {
                path: out.to_path_buf(),
            });
        }
        Ok(())
    }

    /// Opens the header's data from its start.
    pub(crate) fn open_header(&self) -> Result<PartReader<'_>, ConversionError> {
        match &self.parts {
            Parts::Source { header, verify, .. } => header
                .open(*verify)
                .map(PartReader::Source)
                .map_err(|error| conversion_error(error, header.path())),
            Parts::Files { header, .. } => File::open(header)
                .map(PartReader::File)
                .map_err(|source| read_error(header, source)),
        }
    }

    fn input_files(&self) -> Vec<PathBuf> {
        let own = match &self.parts {
            Parts::Source { .. } => Vec::new(),
            Parts::Files { header, .. } => {
                let count = self.part_count().unwrap_or(0);
                std::iter::once(header.clone())
                    .chain((1..=count).filter_map(|index| self.parts.file_of(index)))
                    .collect()
            }
        };
        let alpha = match (self.alpha_tail(), &self.alpha) {
            (Some(Ok(tail)), _) => tail.input_files(),
            (Some(Err(_)), Some(Parts::Files { header, .. })) => vec![header.clone()],
            _ => Vec::new(),
        };
        own.into_iter().chain(alpha).collect()
    }
}

impl Texture<'static> {
    /// Opens the texture whose header is the file at `path`, with its parts `<path>.1` … and any
    /// alpha tail `<path>.a` next to it; a path ending in `.a` opens an alpha tail on its own.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::NoSuchFile`] when the file does not exist, and another
    /// [`ConversionError`] when it cannot be read or holds no DDS header.
    pub fn from_file(path: &Path) -> Result<Self, ConversionError> {
        if let Err(source) = std::fs::metadata(path) {
            return Err(if source.kind() == io::ErrorKind::NotFound {
                ConversionError::NoSuchFile {
                    path: path.to_path_buf(),
                }
            } else {
                read_error(path, source)
            });
        }
        Self::from_found(Parts::on_disk(path))
    }
}

/// Returns `error` of opening the data of the file `name` as a [`ConversionError`].
fn conversion_error(error: ExtractionError, name: &str) -> ConversionError {
    match error {
        ExtractionError::Archive(error) => ConversionError::Archive(error),
        other => ConversionError::Read {
            path: PathBuf::from(name),
            source: io::Error::other(other),
        },
    }
}

fn parse_start(reader: impl Read, name: &str) -> Result<DdsHeader, ConversionError> {
    let mut start = Vec::with_capacity(MAX_HEADER_LEN);
    reader
        .take(MAX_HEADER_LEN as u64)
        .read_to_end(&mut start)
        .map_err(|source| ConversionError::of_texture(name, TextureError::ReadHeader(source)))?;
    DdsHeader::parse(&start).map_err(|error| ConversionError::of_texture(name, error))
}
