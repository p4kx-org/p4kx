// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reads Star Citizen `Data.p4k` Archives in two layouts: v1, a ZIP64 container whose central
//! directory carries CIG-specific extra blocks, and v2, fixed 204-byte directory records with a
//! name table behind a trailer ending in `Jiji`. Entries may be stored, deflate or zstd compressed
//! and AES-128-CBC encrypted with a key the caller supplies; their data is streamed through
//! decryption and decompression, and an Archive without a key still lists and reads everything
//! unencrypted. An Entry
//! that is itself a plain ZIP archive, such as a socpak container, opens as an Archive of its own.

mod archive;
mod archive_metadata;
mod available_metadata_span;
mod build;
mod central_directory;
mod checksum;
mod cig_install_metadata;
mod compression;
mod container;
mod data_error;
mod decode;
mod decrypt;
mod directory_v2;
mod directory_v2_trailer;
mod dos_calendar;
mod dos_timestamp;
mod effective_encoding;
mod encryption_declarations;
mod end_records;
mod entry;
mod entry_decoder;
mod entry_identity;
mod entry_metadata;
mod entry_reader;
mod entry_record_format;
mod error;
mod extra_block_header;
mod file_source;
mod format_error;
mod hasher;
mod installation;
mod issue;
mod key_check;
mod key_demand;
mod key_demand_counts;
mod key_slot;
mod layout;
mod metadata_access;
mod metadata_extra;
mod metadata_extras;
mod metadata_range;
mod metadata_read_error;
mod metadata_record;
mod metadata_records;
mod metadata_region;
mod metadata_regions;
mod metadata_section;
mod metadata_span;
mod metadata_table_records;
mod metadata_table_row;
mod metadata_tables;
mod metadata_view;
mod metadata_zip_record;
mod open_options;
mod recovery;
mod source;
mod stored_hash;
mod v2_directory_metadata;
mod verify;
mod zip64_end_metadata;
mod zip64_locator_metadata;
mod zip_directory_metadata;
mod zip_end_metadata;
mod zip_header_metadata;

pub use archive::{Archive, MAX_DECODED_SKIP};
pub use archive_metadata::{ArchiveLayoutKind, ArchiveMetadata, ArchiveMetadataStatus};
pub use available_metadata_span::AvailableMetadataSpan;
pub use build::{Build, BuildError};
pub use checksum::Checksum;
pub use cig_install_metadata::CigInstallMetadata;
pub use compression::Compression;
pub use container::{
    MAX_COMPRESSION_RATIO, MAX_CONTAINER_DEPTH, MAX_CONTAINER_ENTRIES, MAX_CONTAINER_LEN,
};
pub use data_error::DataError;
pub use dos_calendar::DosCalendar;
pub use dos_timestamp::DosTimestamp;
pub use effective_encoding::EffectiveEncoding;
pub use encryption_declarations::{
    DeclaredEncryption, EncryptionDeclarationSource, EncryptionDeclarations,
    StoredEncryptionDeclaration,
};
pub use entry::Entry;
pub use entry_metadata::EntryMetadata;
pub use entry_reader::EntryReader;
pub use entry_record_format::EntryRecordFormat;
pub use error::ArchiveError;
pub use hasher::ChecksumHasher;
pub use installation::InstallationStatus;
pub use issue::{EntryIssue, MAX_ENTRY_ISSUES};
pub use key_check::KeyCheck;
pub use key_demand::{DemandScope, KeyDemand, KeyDemandSnapshot};
pub use key_demand_counts::UnreadEntries;
pub use metadata_access::{MAX_METADATA_CHUNK, MetadataAccess, MetadataCost};
pub use metadata_extra::{MetadataExtra, MetadataExtraStatus};
pub use metadata_extras::MetadataExtras;
pub use metadata_range::MetadataRange;
pub use metadata_read_error::MetadataReadError;
pub use metadata_record::MetadataRecord;
pub use metadata_records::MetadataRecords;
pub use metadata_region::{MetadataRegion, MetadataRegionStatus};
pub use metadata_section::MetadataSection;
pub use metadata_span::MetadataSpan;
pub use metadata_table_records::MetadataTableRecords;
pub use metadata_table_row::{
    MetadataFreeStatus, MetadataFreeValue, MetadataInstallValue, MetadataProgressBound,
    MetadataReservedWord, MetadataTableRow, MetadataTableValue,
};
pub use metadata_tables::{MetadataTableProblem, MetadataTables};
pub use metadata_view::MetadataView;
pub use open_options::OpenOptions;
pub use recovery::{MAX_RECOVERED_ENTRIES, MAX_RECOVERY_SCAN};
pub use v2_directory_metadata::V2DirectoryMetadata;
pub use zip_directory_metadata::ZipDirectoryMetadata;
pub use zip_end_metadata::ZipEndMetadata;
pub use zip_header_metadata::ZipHeaderMetadata;
pub use zip64_end_metadata::Zip64EndMetadata;
pub use zip64_locator_metadata::Zip64LocatorMetadata;
