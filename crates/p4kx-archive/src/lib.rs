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
mod build;
mod central_directory;
mod checksum;
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
mod end_records;
mod entry;
mod entry_decoder;
mod entry_metadata;
mod entry_reader;
mod error;
mod file_source;
mod format_error;
mod hasher;
mod installation;
mod issue;
mod key_check;
mod key_demand;
mod key_slot;
mod layout;
mod open_options;
mod recovery;
mod source;
mod stored_hash;
mod verify;

pub use archive::{Archive, MAX_DECODED_SKIP};
pub use build::{Build, BuildError};
pub use checksum::Checksum;
pub use compression::Compression;
pub use container::{
    MAX_COMPRESSION_RATIO, MAX_CONTAINER_DEPTH, MAX_CONTAINER_ENTRIES, MAX_CONTAINER_LEN,
};
pub use data_error::DataError;
pub use dos_calendar::DosCalendar;
pub use dos_timestamp::DosTimestamp;
pub use effective_encoding::EffectiveEncoding;
pub use entry::Entry;
pub use entry_metadata::EntryMetadata;
pub use entry_reader::EntryReader;
pub use error::ArchiveError;
pub use hasher::ChecksumHasher;
pub use installation::InstallationStatus;
pub use issue::{EntryIssue, MAX_ENTRY_ISSUES};
pub use key_check::KeyCheck;
pub use key_demand::{KeyDemand, KeyDemandSnapshot};
pub use open_options::OpenOptions;
pub use recovery::{MAX_RECOVERED_ENTRIES, MAX_RECOVERY_SCAN};
