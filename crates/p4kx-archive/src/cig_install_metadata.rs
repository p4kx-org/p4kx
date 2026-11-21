// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::format_error::FormatError;
use crate::metadata_tables::{MetadataTables, ZIP_PROGRESS_BYTES};

/// Positively recognised CIG comment declarations, independent of payload verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CigInstallMetadata {
    sector_size: u16,
    free_count: u64,
    tables: MetadataTables,
}

impl CigInstallMetadata {
    pub(crate) fn read(
        comment: &[u8],
        count: u64,
        directory: u64,
        source_len: u64,
    ) -> Result<Option<Self>, FormatError> {
        if comment.len() != 16 || !comment.starts_with(b"CIG\0\x01\0") {
            return Ok(None);
        }
        let mut reader = ByteReader::new(comment);
        reader.skip(6)?;
        let sector_size = reader.read_u16()?;
        let free_count = reader.read_u64()?;
        Ok(Some(Self {
            sector_size,
            free_count,
            tables: MetadataTables::before(
                directory,
                count,
                free_count,
                ZIP_PROGRESS_BYTES as u64,
                source_len,
            ),
        }))
    }

    /// Returns the unmodified 16-bit sector-size word.
    #[must_use]
    pub const fn sector_size(self) -> u16 {
        self.sector_size
    }
    /// Returns the unmodified free-record count.
    #[must_use]
    pub const fn free_count(self) -> u64 {
        self.free_count
    }
    /// Returns checked table locations or their typed invalid declaration.
    #[must_use]
    pub const fn tables(self) -> MetadataTables {
        self.tables
    }
}
