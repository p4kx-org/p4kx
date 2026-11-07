// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::format_error::FormatError;
use crate::metadata_range::MetadataRange;

/// Original words of the ZIP64 locator actually selected by the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zip64LocatorMetadata {
    range: MetadataRange,
    disk: u32,
    target: u64,
    disks: u32,
}

impl Zip64LocatorMetadata {
    pub(crate) fn read(bytes: &[u8], offset: u64, source_len: u64) -> Result<Self, FormatError> {
        let mut reader = ByteReader::with_base(bytes, offset);
        if reader.read_u32()? != 0x0706_4b50 {
            return Err(FormatError::corrupt(
                offset,
                "ZIP64 locator signature missing",
            ));
        }
        Ok(Self {
            range: MetadataRange::checked(offset, 20, source_len)?,
            disk: reader.read_u32()?,
            target: reader.read_u64()?,
            disks: reader.read_u32()?,
        })
    }

    /// Returns the actual selected locator bytes.
    #[must_use]
    pub const fn range(self) -> MetadataRange {
        self.range
    }
    /// Returns the original ZIP64 end-record disk word.
    #[must_use]
    pub const fn disk(self) -> u32 {
        self.disk
    }
    /// Returns the original target offset in owning-source coordinates.
    #[must_use]
    pub const fn target(self) -> u64 {
        self.target
    }
    /// Returns the original total-disk declaration.
    #[must_use]
    pub const fn disks(self) -> u32 {
        self.disks
    }
}
