// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::DecimalU64;
use p4kx_archive::V2DirectoryMetadata;
use schemars::JsonSchema;
use serde::Serialize;

use crate::ArchiveByteRange;

/// All original v2 trailer words without authentication or alignment inference.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct ArchiveV2Layout {
    /// Original base record count.
    pub base_count: DecimalU64,
    /// Original extension record count.
    pub extension_count: DecimalU64,
    /// Original combined directory start.
    pub directory_offset: DecimalU64,
    /// Original base directory size.
    pub base_directory_size: DecimalU64,
    /// Original extension directory size.
    pub extension_directory_size: DecimalU64,
    /// Original combined names start.
    pub names_offset: DecimalU64,
    /// Original base names size.
    pub base_names_size: DecimalU64,
    /// Original extension names size.
    pub extension_names_size: DecimalU64,
    /// Original payload-end word.
    pub payload_end: DecimalU64,
    /// Original free-row count.
    pub free_count: DecimalU64,
    /// Original trie-cache start, including invalid declarations.
    pub trie_offset: DecimalU64,
    /// Original trie-cache size.
    pub trie_size: DecimalU64,
    /// Original sector-size word, without an alignment claim.
    pub sector_size: DecimalU64,
    /// All authored installation flag bits.
    #[schemars(range(min = 0, max = u8::MAX))]
    pub flags: u8,
    /// Actual authored layout-version word.
    #[schemars(range(min = 0, max = u16::MAX))]
    pub layout_version: u16,
    /// Actual authored magic word.
    #[schemars(range(min = 0, max = u32::MAX))]
    pub magic: u32,
    /// Actual fixed trailer extent; both opaque slots are retained separately.
    pub trailer: ArchiveByteRange,
}

impl ArchiveV2Layout {
    /// Projects every cached original v2 trailer word.
    #[must_use]
    pub const fn of(value: &V2DirectoryMetadata) -> Self {
        Self {
            base_count: DecimalU64::new(value.base_count()),
            extension_count: DecimalU64::new(value.extension_count()),
            directory_offset: DecimalU64::new(value.directory_offset()),
            base_directory_size: DecimalU64::new(value.base_directory_size()),
            extension_directory_size: DecimalU64::new(value.extension_directory_size()),
            names_offset: DecimalU64::new(value.names_offset()),
            base_names_size: DecimalU64::new(value.base_names_size()),
            extension_names_size: DecimalU64::new(value.extension_names_size()),
            payload_end: DecimalU64::new(value.payload_end()),
            free_count: DecimalU64::new(value.free_count()),
            trie_offset: DecimalU64::new(value.trie_offset()),
            trie_size: DecimalU64::new(value.trie_size()),
            sector_size: DecimalU64::new(value.sector_size()),
            flags: value.flags(),
            layout_version: value.layout_version(),
            magic: value.magic(),
            trailer: ArchiveByteRange::of(value.trailer()),
        }
    }
}
