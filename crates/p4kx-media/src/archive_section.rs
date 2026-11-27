// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ArchiveMetadataVersion, ArchiveRegion, MetadataSection};
use schemars::JsonSchema;
use serde::Serialize;

use crate::archive_raw::ArchiveRawSpan;

/// An original metadata section without a semantic claim for opaque bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArchiveSection {
    /// Original complete directory, including omitted folders.
    Directory,
    /// Fixed v2 trailer.
    V2Trailer,
    /// Classic ZIP end record.
    ZipEnd,
    /// Fixed ZIP64 end record.
    Zip64End,
    /// Selected ZIP64 locator.
    Zip64Locator,
    /// Classic ZIP comment.
    ZipComment,
    /// Uninterpreted bytes following the ZIP comment.
    ZipFollowing,
    /// ZIP64 extensible data.
    Zip64Extension,
    /// Original v2 base records.
    V2BaseDirectory,
    /// Original v2 extension records.
    V2ExtensionDirectory,
    /// Original v2 base names.
    V2BaseNames,
    /// Original v2 extension names.
    V2ExtensionNames,
    /// First opaque trailer slot.
    V2OpaqueFirst,
    /// Second opaque trailer slot.
    V2OpaqueSecond,
    /// Unparsed v2 trie cache.
    V2Trie,
    /// Declared installation progress.
    Installation,
    /// Declared free-space records.
    FreeRanges,
}

pub(crate) const SECTIONS: [(ArchiveSection, MetadataSection); 17] = [
    (ArchiveSection::Directory, MetadataSection::Directory),
    (ArchiveSection::V2Trailer, MetadataSection::V2Trailer),
    (ArchiveSection::ZipEnd, MetadataSection::ZipEnd),
    (ArchiveSection::Zip64End, MetadataSection::Zip64End),
    (ArchiveSection::Zip64Locator, MetadataSection::Zip64Locator),
    (ArchiveSection::ZipComment, MetadataSection::ZipComment),
    (ArchiveSection::ZipFollowing, MetadataSection::ZipFollowing),
    (
        ArchiveSection::Zip64Extension,
        MetadataSection::Zip64Extension,
    ),
    (
        ArchiveSection::V2BaseDirectory,
        MetadataSection::V2BaseDirectory,
    ),
    (
        ArchiveSection::V2ExtensionDirectory,
        MetadataSection::V2ExtensionDirectory,
    ),
    (ArchiveSection::V2BaseNames, MetadataSection::V2BaseNames),
    (
        ArchiveSection::V2ExtensionNames,
        MetadataSection::V2ExtensionNames,
    ),
    (
        ArchiveSection::V2OpaqueFirst,
        MetadataSection::V2OpaqueFirst,
    ),
    (
        ArchiveSection::V2OpaqueSecond,
        MetadataSection::V2OpaqueSecond,
    ),
    (ArchiveSection::V2Trie, MetadataSection::V2Trie),
    (ArchiveSection::Installation, MetadataSection::Installation),
    (ArchiveSection::FreeRanges, MetadataSection::FreeRanges),
];

/// Presence of a source section without conflating absent, invalid and empty.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArchiveSectionPresence {
    /// This format has no such authored section.
    Absent,
    /// The reader established the section's raw declaration.
    Declared,
    /// Its declaration is malformed and does not establish a location.
    InvalidDeclaration,
    /// Recovery cannot establish this global section.
    UnavailableLayout,
}

/// One section declaration and its independently verified raw coverage.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct ArchiveSectionData {
    /// Source-layout payload contract.
    pub version: ArchiveMetadataVersion,
    /// Original logical section.
    pub section: ArchiveSection,
    /// Presence and structural availability of the declaration.
    pub presence: ArchiveSectionPresence,
    /// Exact declaration and readable prefix when established.
    pub region: Option<ArchiveRegion>,
    /// Fully retained available prefix, independently of declaration completeness.
    pub raw: Option<ArchiveRawSpan>,
}
