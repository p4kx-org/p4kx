// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::DecimalU64;
use p4kx_archive::{
    CigInstallMetadata, Zip64EndMetadata, Zip64LocatorMetadata, ZipDirectoryMetadata,
    ZipEndMetadata,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::ArchiveByteRange;
use crate::ArchiveRegion;
use crate::ArchiveTableDeclaration;

/// Selected directory plus every original ZIP and ZIP64 end declaration.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(transform = crate::archive_projection_schema::require_properties)]
#[non_exhaustive]
pub struct ArchiveZipLayout {
    /// Selected source-relative directory extent.
    pub records: ArchiveByteRange,
    /// Original record count before folder admission.
    pub count: DecimalU64,
    /// Original classic ZIP words, including ZIP64 sentinels.
    pub end: ArchiveZipEnd,
    /// Selected ZIP64 end declaration.
    pub zip64: Option<ArchiveZip64End>,
    /// Selected locator, including its actual physical position.
    pub locator: Option<ArchiveZip64Locator>,
    /// Positively recognised CIG comment declaration.
    pub installation: Option<ArchiveCigInstallation>,
}

impl ArchiveZipLayout {
    /// Projects cached ZIP declarations without accessing record or table bodies.
    #[must_use]
    pub fn of(value: &ZipDirectoryMetadata) -> Self {
        Self {
            records: ArchiveByteRange::of(value.records()),
            count: DecimalU64::new(value.count()),
            end: ArchiveZipEnd::of(*value.end()),
            zip64: value.zip64().copied().map(ArchiveZip64End::of),
            locator: value.locator().copied().map(ArchiveZip64Locator::of),
            installation: value
                .installation()
                .copied()
                .map(ArchiveCigInstallation::of),
        }
    }
}

/// Original classic ZIP end words without replacing sentinels by selected values.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(transform = crate::archive_projection_schema::require_properties)]
#[non_exhaustive]
pub struct ArchiveZipEnd {
    /// Actual fixed end-record extent.
    pub fixed: ArchiveByteRange,
    /// Original disk word.
    #[schemars(range(min = 0, max = u16::MAX))]
    pub disk: u16,
    /// Original directory disk word.
    #[schemars(range(min = 0, max = u16::MAX))]
    pub directory_disk: u16,
    /// Original per-disk count.
    #[schemars(range(min = 0, max = u16::MAX))]
    pub entries_on_disk: u16,
    /// Original total count.
    #[schemars(range(min = 0, max = u16::MAX))]
    pub entries: u16,
    /// Original 32-bit directory-size word.
    #[schemars(range(min = 0, max = u32::MAX))]
    pub directory_size: u32,
    /// Original 32-bit directory-offset word.
    #[schemars(range(min = 0, max = u32::MAX))]
    pub directory_offset: u32,
    /// Original comment-length word.
    #[schemars(range(min = 0, max = u16::MAX))]
    pub comment_length: u16,
    /// Exact comment declaration and readable prefix.
    pub comment: ArchiveRegion,
    /// Uninterpreted bytes after the declared comment.
    pub following: Option<ArchiveByteRange>,
}

impl ArchiveZipEnd {
    fn of(value: ZipEndMetadata) -> Self {
        Self {
            fixed: ArchiveByteRange::of(value.fixed()),
            disk: value.disk(),
            directory_disk: value.directory_disk(),
            entries_on_disk: value.entries_on_disk(),
            entries: value.entries(),
            directory_size: value.directory_size(),
            directory_offset: value.directory_offset(),
            comment_length: value.comment_len(),
            comment: ArchiveRegion::of(value.comment()),
            following: value.following().map(ArchiveByteRange::of),
        }
    }
}

/// Original ZIP64 words without assuming multi-disk support or extension semantics.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(transform = crate::archive_projection_schema::require_properties)]
#[non_exhaustive]
pub struct ArchiveZip64End {
    /// Actual fixed record extent.
    pub fixed: ArchiveByteRange,
    /// Size word excluding signature and size word.
    pub declared_size: DecimalU64,
    /// Original version-made-by word.
    #[schemars(range(min = 0, max = u16::MAX))]
    pub version_made_by: u16,
    /// Original version-needed word.
    #[schemars(range(min = 0, max = u16::MAX))]
    pub version_needed: u16,
    /// Original disk word.
    #[schemars(range(min = 0, max = u32::MAX))]
    pub disk: u32,
    /// Original directory disk word.
    #[schemars(range(min = 0, max = u32::MAX))]
    pub directory_disk: u32,
    /// Original per-disk count.
    pub entries_on_disk: DecimalU64,
    /// Original total count.
    pub entries: DecimalU64,
    /// Original directory-size word.
    pub directory_size: DecimalU64,
    /// Original directory-offset word.
    pub directory_offset: DecimalU64,
    /// Authored extension, absent only when its size cannot establish an extent.
    pub extension: Option<ArchiveRegion>,
}

impl ArchiveZip64End {
    fn of(value: Zip64EndMetadata) -> Self {
        Self {
            fixed: ArchiveByteRange::of(value.fixed()),
            declared_size: DecimalU64::new(value.declared_size()),
            version_made_by: value.version_made_by(),
            version_needed: value.version_needed(),
            disk: value.disk(),
            directory_disk: value.directory_disk(),
            entries_on_disk: DecimalU64::new(value.entries_on_disk()),
            entries: DecimalU64::new(value.entries()),
            directory_size: DecimalU64::new(value.directory_size()),
            directory_offset: DecimalU64::new(value.directory_offset()),
            extension: value.extension().map(ArchiveRegion::of),
        }
    }
}

/// Original selected locator words and its actual source position.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct ArchiveZip64Locator {
    /// Actual locator extent.
    pub range: ArchiveByteRange,
    /// Original ZIP64 end-record disk word.
    #[schemars(range(min = 0, max = u32::MAX))]
    pub disk: u32,
    /// Original target offset, independently of the locator's actual position.
    pub target: DecimalU64,
    /// Original total-disk declaration.
    #[schemars(range(min = 0, max = u32::MAX))]
    pub disks: u32,
}

impl ArchiveZip64Locator {
    const fn of(value: Zip64LocatorMetadata) -> Self {
        Self {
            range: ArchiveByteRange::of(value.range()),
            disk: value.disk(),
            target: DecimalU64::new(value.target()),
            disks: value.disks(),
        }
    }
}

/// Positively recognised CIG installation declaration without payload verification claims.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct ArchiveCigInstallation {
    /// Unmodified 16-bit sector-size word.
    #[schemars(range(min = 0, max = u16::MAX))]
    pub sector_size: u16,
    /// Unmodified free-row count.
    pub free_count: DecimalU64,
    /// Independently checked table locations or explicit declaration failure.
    pub tables: ArchiveTableDeclaration,
}

impl ArchiveCigInstallation {
    const fn of(value: CigInstallMetadata) -> Self {
        Self {
            sector_size: value.sector_size(),
            free_count: DecimalU64::new(value.free_count()),
            tables: ArchiveTableDeclaration::of(value.tables()),
        }
    }
}
