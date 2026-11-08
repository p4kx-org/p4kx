// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::texture::{self, AlphaTail, Texture};
use crate::disk_parts::DiskParts;
use crate::output_file::read_error;
use crate::texture_parts::{PartReader, Parts, is_same_file};
use crate::{ConversionError, DiskTextures, TextureError};

impl Texture<'_> {
    pub(crate) fn refuse_input(&self, out: &Path) -> Result<(), ConversionError> {
        if is_same_file(out, self.input_files()?) {
            return Err(ConversionError::OutputIsInput {
                path: out.to_path_buf(),
            });
        }
        Ok(())
    }

    /// Opens the bound header from its start.
    pub(crate) fn open_header(&self) -> Result<PartReader<'_>, ConversionError> {
        match &self.parts {
            Parts::Source { header, verify, .. } => header
                .open(*verify)
                .map(PartReader::Source)
                .map_err(|error| texture::conversion_error(error, header.path())),
            Parts::Files(parts) => parts
                .open_header()
                .map(PartReader::File)
                .map_err(|source| read_error(parts.header(), source)),
        }
    }

    fn input_files(&self) -> Result<Vec<PathBuf>, ConversionError> {
        let mut own = match &self.parts {
            Parts::Source { .. } => Vec::new(),
            Parts::Files(parts) => parts.input_files(self.part_count().unwrap_or(0))?,
        };
        let alpha = match (self.alpha_tail(), &self.alpha) {
            (Some(Ok(tail)), _) => tail.input_files()?,
            (
                Some(Err(ConversionError::Texture {
                    source:
                        TextureError::NotDds
                        | TextureError::TruncatedHeader { .. }
                        | TextureError::InvalidHeader(_),
                    ..
                })),
                AlphaTail::Present(Parts::Files(parts)),
            ) => parts.input_files(0)?,
            (
                Some(Err(error)),
                AlphaTail::Present(Parts::Files(_)) | AlphaTail::LookupFailed(_),
            ) => return Err(error),
            _ => Vec::new(),
        };
        own.extend(alpha);
        Ok(own)
    }
}

impl Texture<'static> {
    /// Opens an explicit disk header and its linked colour and alpha parts in one run.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::NoSuchFile`] for a missing header and the original read or
    /// DDS-header error otherwise.
    pub fn from_file(path: &Path) -> Result<Self, ConversionError> {
        DiskTextures::default().open(path)
    }

    pub(crate) fn from_disk(files: &DiskTextures, path: &Path) -> Result<Self, ConversionError> {
        fs::metadata(path).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                ConversionError::NoSuchFile {
                    path: path.to_path_buf(),
                }
            } else {
                read_error(path, source)
            }
        })?;
        let parts = DiskParts::new(files, path).map_err(|source| read_error(path, source))?;
        let alpha = parts.alpha_candidate();
        let mut texture = Self::from_parts(Parts::Files(parts), None)?;
        texture.alpha = alpha.map_or(AlphaTail::Absent, |parts| {
            match parts.header_exists() {
                Ok(true) => AlphaTail::Present(Parts::Files(parts)),
                Ok(false) => AlphaTail::Absent,
                // The run retains the failure; opening the tail replays its original cause.
                Err(_) => AlphaTail::LookupFailed(parts),
            }
        });
        Ok(texture)
    }
}
