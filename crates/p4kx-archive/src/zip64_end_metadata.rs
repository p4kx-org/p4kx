// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::format_error::FormatError;
use crate::metadata_range::MetadataRange;
use crate::metadata_region::MetadataRegion;

/// Raw ZIP64 end-record words with a separately bounded extension declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zip64EndMetadata {
    fixed: MetadataRange,
    declared_size: u64,
    version_made_by: u16,
    version_needed: u16,
    disk: u32,
    directory_disk: u32,
    entries_on_disk: u64,
    entries: u64,
    directory_size: u64,
    directory_offset: u64,
    extension: Option<MetadataRegion>,
}

impl Zip64EndMetadata {
    pub(crate) fn read(bytes: &[u8], offset: u64, source_len: u64) -> Result<Self, FormatError> {
        let mut reader = ByteReader::with_base(bytes, offset);
        if reader.read_u32()? != 0x0606_4b50 {
            return Err(FormatError::corrupt(
                offset,
                "ZIP64 end record signature missing",
            ));
        }
        let declared_size = reader.read_u64()?;
        let version_made_by = reader.read_u16()?;
        let version_needed = reader.read_u16()?;
        let disk = reader.read_u32()?;
        let directory_disk = reader.read_u32()?;
        let entries_on_disk = reader.read_u64()?;
        let entries = reader.read_u64()?;
        let directory_size = reader.read_u64()?;
        let directory_offset = reader.read_u64()?;
        let fixed = MetadataRange::checked(offset, 56, source_len)?;
        let extension = declared_size
            .checked_sub(44)
            .map(|len| MetadataRegion::of(fixed.end(), len, source_len));
        Ok(Self {
            fixed,
            declared_size,
            version_made_by,
            version_needed,
            disk,
            directory_disk,
            entries_on_disk,
            entries,
            directory_size,
            directory_offset,
            extension,
        })
    }

    /// Returns the actual fixed 56-byte record.
    #[must_use]
    pub const fn fixed(self) -> MetadataRange {
        self.fixed
    }
    /// Returns the source size word, excluding the signature and size word itself.
    #[must_use]
    pub const fn declared_size(self) -> u64 {
        self.declared_size
    }
    /// Returns the original version-made-by word.
    #[must_use]
    pub const fn version_made_by(self) -> u16 {
        self.version_made_by
    }
    /// Returns the original version-needed word.
    #[must_use]
    pub const fn version_needed(self) -> u16 {
        self.version_needed
    }
    /// Returns the original disk-number word.
    #[must_use]
    pub const fn disk(self) -> u32 {
        self.disk
    }
    /// Returns the original directory-disk word.
    #[must_use]
    pub const fn directory_disk(self) -> u32 {
        self.directory_disk
    }
    /// Returns the source per-disk count.
    #[must_use]
    pub const fn entries_on_disk(self) -> u64 {
        self.entries_on_disk
    }
    /// Returns the source total count.
    #[must_use]
    pub const fn entries(self) -> u64 {
        self.entries
    }
    /// Returns the source directory-size declaration.
    #[must_use]
    pub const fn directory_size(self) -> u64 {
        self.directory_size
    }
    /// Returns the source directory-offset declaration.
    #[must_use]
    pub const fn directory_offset(self) -> u64 {
        self.directory_offset
    }
    /// Returns the declared extension, absent only for an invalid fixed-record size.
    #[must_use]
    pub const fn extension(self) -> Option<MetadataRegion> {
        self.extension
    }
}
