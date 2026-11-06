// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::format_error::FormatError;
use crate::layout;
use crate::source::Source;

pub(crate) const TRAILER_LEN: usize = 175;
pub(crate) const RECORD_LEN: usize = 204;
const PROGRESS_BYTES: u64 = 8;
const FREELIST_BYTES: u64 = 16;
const CACHE_AND_SECTOR_BYTES: usize = 24;
const INCOMPLETE_FLAG: u8 = 1;

/// The separate base and extension declarations of a v2 directory.
pub(crate) struct Trailer {
    base_count: u64,
    extension_count: u64,
    pub(crate) directory_offset: u64,
    base_directory_size: u64,
    extension_directory_size: u64,
    pub(crate) names_offset: u64,
    base_names_size: u64,
    extension_names_size: u64,
    pub(crate) payload_end: u64,
    freelist_count: u64,
    pub(crate) flags: u8,
}

impl Trailer {
    /// Reads and validates all directory ranges before their contents are allocated.
    pub(crate) fn read(
        source: &Source,
        offset: u64,
        max_entries: Option<u64>,
    ) -> Result<Self, FormatError> {
        let bytes = source.read_at(offset, TRAILER_LEN)?;
        let mut reader = ByteReader::with_base(&bytes, offset);
        let trailer = Self {
            base_count: reader.read_u64()?,
            extension_count: reader.read_u64()?,
            directory_offset: reader.read_u64()?,
            base_directory_size: reader.read_u64()?,
            extension_directory_size: reader.read_u64()?,
            names_offset: reader.read_u64()?,
            base_names_size: reader.read_u64()?,
            extension_names_size: reader.read_u64()?,
            payload_end: reader.read_u64()?,
            freelist_count: reader.read_u64()?,
            flags: {
                reader.skip(CACHE_AND_SECTOR_BYTES)?;
                reader.read_bytes(1)?.first().copied().ok_or_else(|| {
                    FormatError::corrupt(offset, "v2 installation status is cut off")
                })?
            },
        };
        trailer.validate(offset, max_entries)?;
        Ok(trailer)
    }

    pub(crate) fn directory_size(&self) -> Result<u64, FormatError> {
        self.sum(
            self.base_directory_size,
            self.extension_directory_size,
            "directory sizes",
        )
    }

    pub(crate) fn names_size(&self) -> Result<u64, FormatError> {
        self.sum(
            self.base_names_size,
            self.extension_names_size,
            "name sizes",
        )
    }

    fn validate(&self, trailer_offset: u64, max_entries: Option<u64>) -> Result<(), FormatError> {
        if self.flags & INCOMPLETE_FLAG != 0 {
            return Err(FormatError::IncompleteInstallation { flags: self.flags });
        }
        let count = self.sum(self.base_count, self.extension_count, "Entry counts")?;
        layout::check_entry_count(count, max_entries)?;
        for (count, size, section) in [
            (self.base_count, self.base_directory_size, "base"),
            (
                self.extension_count,
                self.extension_directory_size,
                "extension",
            ),
        ] {
            if count.checked_mul(RECORD_LEN as u64) != Some(size) {
                return Err(FormatError::corrupt(
                    trailer_offset,
                    format!("{section} directory size does not match its {count} Entries"),
                ));
            }
        }
        self.validate_installation(count, trailer_offset)?;
        let directory_end = self
            .directory_offset
            .checked_add(self.directory_size()?)
            .ok_or_else(|| FormatError::corrupt(trailer_offset, "directory range overflows"))?;
        if directory_end != self.names_offset {
            return Err(FormatError::corrupt(
                trailer_offset,
                "name table does not follow the complete base and extension directory",
            ));
        }
        let names_end = self
            .names_offset
            .checked_add(self.names_size()?)
            .ok_or_else(|| FormatError::corrupt(trailer_offset, "name table range overflows"))?;
        if directory_end > trailer_offset || names_end > trailer_offset {
            return Err(FormatError::corrupt(
                trailer_offset,
                "directory or name table lies outside the file",
            ));
        }
        Ok(())
    }

    /// Reads the per-Entry installed-byte counters after their range has been validated.
    pub(crate) fn progress(&self, source: &Source) -> Result<Vec<u8>, FormatError> {
        let count = self.sum(self.base_count, self.extension_count, "Entry counts")?;
        let size = count
            .checked_mul(PROGRESS_BYTES)
            .and_then(|size| usize::try_from(size).ok())
            .ok_or_else(|| {
                FormatError::corrupt(
                    self.payload_end,
                    "installed-byte counters do not fit into memory",
                )
            })?;
        Ok(source.read_at(self.payload_end, size)?)
    }
    fn validate_installation(&self, count: u64, trailer_offset: u64) -> Result<(), FormatError> {
        let install_size = count
            .checked_mul(PROGRESS_BYTES)
            .and_then(|progress| {
                self.freelist_count
                    .checked_mul(FREELIST_BYTES)?
                    .checked_add(progress)
            })
            .ok_or_else(|| {
                FormatError::corrupt(trailer_offset, "installation metadata size overflows")
            })?;
        if self
            .payload_end
            .checked_add(install_size)
            .is_none_or(|end| end > self.directory_offset)
        {
            return Err(FormatError::corrupt(
                trailer_offset,
                "installation metadata is truncated or overlaps the directory",
            ));
        }
        Ok(())
    }

    fn sum(&self, base: u64, extension: u64, what: &str) -> Result<u64, FormatError> {
        base.checked_add(extension).ok_or_else(|| {
            FormatError::corrupt(self.directory_offset, format!("combined {what} overflow"))
        })
    }
}
