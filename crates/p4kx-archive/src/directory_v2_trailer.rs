// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::format_error::FormatError;
use crate::layout;
use crate::source::Source;

pub(crate) const TRAILER_LEN: usize = 175;
pub(crate) const RECORD_LEN: usize = 204;

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

    fn sum(&self, base: u64, extension: u64, what: &str) -> Result<u64, FormatError> {
        base.checked_add(extension).ok_or_else(|| {
            FormatError::corrupt(self.directory_offset, format!("combined {what} overflow"))
        })
    }
}
