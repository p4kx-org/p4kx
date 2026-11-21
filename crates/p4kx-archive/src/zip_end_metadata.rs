// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::format_error::FormatError;
use crate::metadata_range::MetadataRange;
use crate::metadata_region::MetadataRegion;

/// Original classic ZIP end-record declarations and their source extents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZipEndMetadata {
    fixed: MetadataRange,
    disk: u16,
    directory_disk: u16,
    entries_on_disk: u16,
    entries: u16,
    directory_size: u32,
    directory_offset: u32,
    comment_len: u16,
    comment: MetadataRegion,
    following: Option<MetadataRange>,
}

impl ZipEndMetadata {
    pub(crate) fn read(bytes: &[u8], offset: u64, source_len: u64) -> Result<Self, FormatError> {
        let mut reader = ByteReader::with_base(bytes, offset);
        if reader.read_u32()? != 0x0605_4b50 {
            return Err(FormatError::corrupt(
                offset,
                "ZIP end-record signature missing",
            ));
        }
        let disk = reader.read_u16()?;
        let directory_disk = reader.read_u16()?;
        let entries_on_disk = reader.read_u16()?;
        let entries = reader.read_u16()?;
        let directory_size = reader.read_u32()?;
        let directory_offset = reader.read_u32()?;
        let comment_len = reader.read_u16()?;
        let fixed = MetadataRange::checked(offset, 22, source_len)?;
        let comment = MetadataRegion::of(fixed.end(), u64::from(comment_len), source_len);
        let following_start = fixed.end().checked_add(u64::from(comment_len));
        let following = following_start
            .and_then(|start| MetadataRange::available_prefix(start, u64::MAX, source_len));
        Ok(Self {
            fixed,
            disk,
            directory_disk,
            entries_on_disk,
            entries,
            directory_size,
            directory_offset,
            comment_len,
            comment,
            following,
        })
    }

    /// Returns the complete fixed 22-byte end record.
    #[must_use]
    pub const fn fixed(self) -> MetadataRange {
        self.fixed
    }
    /// Returns the original disk-number word.
    #[must_use]
    pub const fn disk(self) -> u16 {
        self.disk
    }
    /// Returns the original central-directory disk word.
    #[must_use]
    pub const fn directory_disk(self) -> u16 {
        self.directory_disk
    }
    /// Returns the original per-disk count, including a ZIP64 sentinel.
    #[must_use]
    pub const fn entries_on_disk(self) -> u16 {
        self.entries_on_disk
    }
    /// Returns the original total count, including a ZIP64 sentinel.
    #[must_use]
    pub const fn entries(self) -> u16 {
        self.entries
    }
    /// Returns the original 32-bit directory-size word.
    #[must_use]
    pub const fn directory_size(self) -> u32 {
        self.directory_size
    }
    /// Returns the original 32-bit directory-offset word.
    #[must_use]
    pub const fn directory_offset(self) -> u32 {
        self.directory_offset
    }
    /// Returns the unmodified comment-length word.
    #[must_use]
    pub const fn comment_len(self) -> u16 {
        self.comment_len
    }
    /// Returns declared comment bytes and their independently checked availability.
    #[must_use]
    pub const fn comment(self) -> MetadataRegion {
        self.comment
    }
    /// Returns bytes following a complete declared comment, without interpreting padding.
    #[must_use]
    pub const fn following(self) -> Option<MetadataRange> {
        self.following
    }
}
