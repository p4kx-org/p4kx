// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_archive::{Archive, Entry};
use p4kx_texture::{DdsHeader, DdsRawHeader, MergeReport, TextureError, merge};

use super::disk_mip::DiskRead;
use crate::disk_parts::DiskParts;
use crate::output_file::{read_error, write_replacing_protected};
use crate::path_index::PathIndex;
use crate::source::SourceEntry;
use crate::source_protection::SourceProtection;
use crate::texture_parts::{Found, Parts};
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
    pub(super) disk_header: Option<crate::ObservedFileOrigin>,
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
        let found = Parts::in_archive(archive, path, None)
            .ok_or_else(|| UnknownEntries(vec![path.to_owned()]))?;
        Self::from_found(found)
    }

    /// Opens the physical header Entry selected from its owning Archive.
    ///
    /// # Errors
    ///
    /// Returns an unknown-Entry error for a foreign Entry, or the original DDS read error.
    pub fn from_entry(archive: &'a Archive, entry: &'a Entry) -> Result<Self, ConversionError> {
        let path = entry.path();
        let missing = || UnknownEntries(vec![path.to_owned()]);
        archive.observed_entry(entry).ok_or_else(missing)?;
        let selected = SourceEntry::of_archive(archive, entry);
        let found = Parts::in_archive(archive, path, Some(selected)).ok_or_else(missing)?;
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
        let (name, header_size, header, disk_header) = match &parts {
            Parts::Source { header, .. } => {
                let name = header.path().to_owned();
                let size = header.uncompressed_size();
                let reader = header
                    .open(false)
                    .map_err(|error| conversion_error(error, &name))?;
                (
                    name.clone(),
                    size,
                    texture_header::read(reader, &name)?,
                    None,
                )
            }
            Parts::Files(parts) => {
                let path = parts.header();
                let name = parts.name().display().to_string();
                let (actual, file) = parts
                    .open_with_path(None)
                    .map_err(|source| read_error(path, source))?;
                let size = file
                    .metadata()
                    .map_err(|source| read_error(path, source))?
                    .len();
                let mut read = DiskRead::new(actual, file);
                let header = texture_header::read(&mut read, &name)?;
                (name, size, header, read.origin())
            }
        };
        Ok(Self {
            protection,
            name,
            header_size,
            header: header.normalized,
            raw_header: header.raw,
            disk_header,
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

    pub(crate) const fn raw_header(&self) -> DdsRawHeader {
        self.raw_header
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

    /// Returns the size of the header in bytes.
    pub(crate) const fn header_size(&self) -> u64 {
        self.header_size
    }

    /// Returns where the header and the parts are stored.
    pub(crate) const fn parts(&self) -> &Parts<'a> {
        &self.parts
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
