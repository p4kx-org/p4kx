// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub use crate::archive_byte_range::ArchiveByteRange;
pub use crate::archive_cause::archive_error_in;
pub use crate::archive_layout_info::{ArchiveDeclarationState, ArchiveLayout, ArchiveLayoutInfo};
pub use crate::archive_layout_v2::ArchiveV2Layout;
pub use crate::archive_layout_zip::{
    ArchiveCigInstallation, ArchiveZip64End, ArchiveZip64Locator, ArchiveZipEnd, ArchiveZipLayout,
};
pub use crate::archive_material_reference::ArchiveMaterialPaths;
pub use crate::archive_metadata_version::ArchiveMetadataVersion;
pub use crate::archive_region_info::{ArchiveRegion, ArchiveRegionState};
pub use crate::archive_table_declaration::ArchiveTableDeclaration;
pub use crate::zip_header_info::ZipHeaderInfo;
pub use p4kx_archive::{
    Archive, ArchiveError, ArchiveInputOrigin, Build, BuildError, BuildManifestObservation,
    BuildManifestOrigin, BuildManifestOutcome, Checksum, Compression, DemandScope, DosCalendar,
    DosTimestamp, EffectiveEncoding, Entry, EntryIssue, EntryMetadata, EntryOrigin,
    EntryOriginStep, EntryReader, EntryRecordFormat, InstallationStatus, KeyCheck, KeyDemand,
    KeyDemandSnapshot, MAX_COMPRESSION_RATIO, MAX_CONTAINER_DEPTH, MAX_CONTAINER_ENTRIES,
    MAX_CONTAINER_LEN, MAX_RECOVERED_ENTRIES, MAX_RECOVERY_SCAN, ObservedEntry,
    ObservedEntryBinding, OpenOptions, SourceEncryption, UnreadEntries, ZipHeaderMetadata,
};
pub use p4kx_archive::{
    ArchiveLayoutKind, ArchiveMetadata, ArchiveMetadataStatus, AvailableMetadataSpan,
    CigInstallMetadata, MAX_METADATA_CHUNK, MetadataAccess, MetadataCost, MetadataExtra,
    MetadataExtraStatus, MetadataFreeStatus, MetadataFreeValue, MetadataInstallValue,
    MetadataProgressBound, MetadataRange, MetadataReadError, MetadataRecord, MetadataRegion,
    MetadataRegionStatus, MetadataReservedWord, MetadataSection, MetadataSpan,
    MetadataTableProblem, MetadataTableRow, MetadataTableValue, MetadataTables, MetadataView,
    V2DirectoryMetadata, Zip64EndMetadata, Zip64LocatorMetadata, ZipDirectoryMetadata,
    ZipEndMetadata,
};
