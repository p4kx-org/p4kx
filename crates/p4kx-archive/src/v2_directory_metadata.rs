// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::metadata_range::MetadataRange;
use crate::metadata_region::MetadataRegion;
use crate::metadata_tables::{FREE_RECORD_BYTES, MetadataTables, V2_PROGRESS_BYTES};

/// Original fixed v2 declarations, with source-relative checked metadata extents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct V2DirectoryMetadata {
    pub(crate) base_count: u64,
    pub(crate) extension_count: u64,
    pub(crate) directory_offset: u64,
    pub(crate) base_directory_size: u64,
    pub(crate) extension_directory_size: u64,
    pub(crate) names_offset: u64,
    pub(crate) base_names_size: u64,
    pub(crate) extension_names_size: u64,
    pub(crate) payload_end: u64,
    pub(crate) freelist_count: u64,
    pub(crate) trie_offset: u64,
    pub(crate) trie_size: u64,
    pub(crate) sector_size: u64,
    pub(crate) flags: u8,
    pub(crate) layout_version: u16,
    pub(crate) magic: u32,
    pub(crate) trailer: MetadataRange,
}

impl V2DirectoryMetadata {
    /// Returns the original base record count.
    #[must_use]
    pub const fn base_count(&self) -> u64 {
        self.base_count
    }
    /// Returns the original extension record count.
    #[must_use]
    pub const fn extension_count(&self) -> u64 {
        self.extension_count
    }
    /// Returns the original combined directory offset.
    #[must_use]
    pub const fn directory_offset(&self) -> u64 {
        self.directory_offset
    }
    /// Returns the original base directory byte count.
    #[must_use]
    pub const fn base_directory_size(&self) -> u64 {
        self.base_directory_size
    }
    /// Returns the original extension directory byte count.
    #[must_use]
    pub const fn extension_directory_size(&self) -> u64 {
        self.extension_directory_size
    }
    /// Returns the original combined name-table offset.
    #[must_use]
    pub const fn names_offset(&self) -> u64 {
        self.names_offset
    }
    /// Returns the original base name-table byte count.
    #[must_use]
    pub const fn base_names_size(&self) -> u64 {
        self.base_names_size
    }
    /// Returns the original extension name-table byte count.
    #[must_use]
    pub const fn extension_names_size(&self) -> u64 {
        self.extension_names_size
    }
    /// Returns the original payload-end word used by the installation reader.
    #[must_use]
    pub const fn payload_end(&self) -> u64 {
        self.payload_end
    }
    /// Returns the original free-record count.
    #[must_use]
    pub const fn free_count(&self) -> u64 {
        self.freelist_count
    }
    /// Returns the raw trie-cache offset word, including zero and invalid declarations.
    #[must_use]
    pub const fn trie_offset(&self) -> u64 {
        self.trie_offset
    }
    /// Returns the raw trie-cache size word.
    #[must_use]
    pub const fn trie_size(&self) -> u64 {
        self.trie_size
    }
    /// Returns the raw physical-sector-size word without an alignment inference.
    #[must_use]
    pub const fn sector_size(&self) -> u64 {
        self.sector_size
    }
    /// Returns all raw installation flag bits.
    #[must_use]
    pub const fn flags(&self) -> u8 {
        self.flags
    }
    /// Returns the actual stored layout-version word.
    #[must_use]
    pub const fn layout_version(&self) -> u16 {
        self.layout_version
    }
    /// Returns the actual stored magic word.
    #[must_use]
    pub const fn magic(&self) -> u32 {
        self.magic
    }
    /// Returns the fixed trailer bytes, including both opaque digest slots.
    #[must_use]
    pub const fn trailer(&self) -> MetadataRange {
        self.trailer
    }
    /// Returns trie availability without claiming that its contents were parsed.
    pub(crate) fn trie(&self, source_len: u64) -> MetadataRegion {
        MetadataRegion::of(self.trie_offset, self.trie_size, source_len)
    }

    pub(crate) fn tables(&self, source_len: u64) -> MetadataTables {
        let sizes = self
            .base_count
            .checked_add(self.extension_count)
            .and_then(|count| count.checked_mul(V2_PROGRESS_BYTES as u64))
            .zip(self.freelist_count.checked_mul(FREE_RECORD_BYTES as u64));
        match sizes {
            Some((progress, free)) => {
                MetadataTables::at(self.payload_end, progress, free, source_len)
            }
            None => MetadataTables::Invalid(crate::metadata_tables::MetadataTableProblem::Overflow),
        }
    }
}
