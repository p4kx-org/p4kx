// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsString;
use std::io::{self, Read};
use std::path::{MAIN_SEPARATOR_STR, Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use p4kx_foundation::ArchiveKey;

use crate::archive_metadata::ArchiveMetadata;
use crate::build::{self, ObservedBuild};
use crate::container::{self, MAX_CONTAINER_DEPTH, MAX_CONTAINER_ENTRIES, MAX_CONTAINER_LEN};
use crate::entry_identity::ContainerChain;
use crate::format_error::FormatError;
use crate::issue::{Directory, IssueLog};
use crate::key_slot::KeySlot;
use crate::origin_ancestor::OriginAncestor;
use crate::source::Source;
use crate::{
    ArchiveError, ArchiveInputOrigin, Build, BuildManifestObservation, Entry, EntryIssue,
    EntryReader, InstallationStatus, OpenOptions, entry_reader, layout,
};

mod key;
mod metadata;
mod origin;

/// The most decoded bytes [`Archive::open_entry_at`] skips in a compressed Entry to reach an offset.
pub const MAX_DECODED_SKIP: u64 = 1 << 30;

/// An opened Archive with its Entries, or a container opened from one of its Entries.
#[derive(Debug)]
pub struct Archive {
    path: PathBuf,
    location: PathBuf,
    container_chain: ContainerChain,
    origin_ancestors: Arc<[OriginAncestor]>,
    input_origin: ArchiveInputOrigin,
    file_len: u64,
    file_modified: Option<SystemTime>,
    depth: usize,
    decoded_budget: u64,
    source: Source,
    entries: Vec<Entry>,
    log: IssueLog,
    installation_flags: Option<u8>,
    metadata: ArchiveMetadata,
    build: Option<Build>,
    build_manifest_observation: BuildManifestObservation,
    key: KeySlot,
}

impl Archive {
    /// Returns the source installation declaration, independently of payload integrity.
    #[must_use]
    pub const fn installation_status(&self) -> InstallationStatus {
        match self.installation_flags {
            None => InstallationStatus::Unavailable,
            Some(flags) => InstallationStatus::DeclaredComplete { flags },
        }
    }

    /// Opens the Archive at `path` and reads its central directory, in the v1 or the v2 layout.
    ///
    /// The file is opened read-only and stays readable and writable for other processes such as
    /// the launcher and the game. The Build is read from the `build_manifest.id` next to the
    /// file; a missing or unreadable manifest leaves it unknown. No launcher file is read and the
    /// Build has no public patch, see [`Build::with_public_patch`].
    ///
    /// # Errors
    ///
    /// Returns an [`ArchiveError`] when the file cannot be read or is not a valid Archive.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ArchiveError> {
        Self::open_with(path, OpenOptions::default())
    }

    /// Opens the Archive at `path` like [`Archive::open`], as `options` say.
    ///
    /// With [`OpenOptions::recover`], an Archive whose end records are missing or whose central
    /// directory is damaged is rebuilt from the directory records a bounded scan finds, and
    /// [`Archive::issues`] lists [`EntryIssue::Recovered`]. Recovered Entries are read and
    /// verified like any other. Without it, such an Archive fails to open.
    ///
    /// # Errors
    ///
    /// Returns an [`ArchiveError`] when the file cannot be read or is not a valid Archive and, with
    /// recovery, holds no recoverable record; the error is then the one the normal open gave.
    pub fn open_with(path: impl AsRef<Path>, options: OpenOptions) -> Result<Self, ArchiveError> {
        let path = path.as_ref();
        let source = Source::open(path).map_err(|error| FormatError::from(error).at(path))?;
        let directory = if options.recover {
            layout::read_entries_recovering(&source)
        } else {
            layout::read_entries(&source, None)
        };
        let Directory {
            entries,
            log,
            installation_flags,
            metadata,
        } = directory.map_err(|error| error.at(path))?;
        let observed = path
            .parent()
            .map_or_else(ObservedBuild::missing, build::read_observed);
        Ok(Self {
            path: path.to_path_buf(),
            location: path.to_path_buf(),
            container_chain: ContainerChain::default(),
            origin_ancestors: Arc::default(),
            input_origin: ArchiveInputOrigin::new(),
            file_len: source.len(),
            file_modified: source.modified(),
            depth: 0,
            decoded_budget: MAX_CONTAINER_LEN,
            source,
            entries,
            log,
            installation_flags,
            metadata,
            build: observed.build,
            build_manifest_observation: observed.observation,
            key: KeySlot::with(options.key),
        })
    }

    /// Opens the container held in `entry`, one of this Archive's Entries, as an Archive of its
    /// own whose Entries are listed and read the same way, without extracting it to disk.
    ///
    /// A stored container is read in place; a compressed or encrypted one is decoded into memory.
    /// The container shares the file, its size, modification time and Build with this Archive.
    /// Containers open up to [`MAX_CONTAINER_DEPTH`] deep, the compressed ones of a chain decode
    /// to at most [`MAX_CONTAINER_LEN`] bytes together, and their Entries are checked against the
    /// other container limits of this crate.
    ///
    /// # Errors
    ///
    /// Returns [`ArchiveError::Container`] naming the Entry, with the reason as its source.
    pub fn open_container(&self, entry: &Entry) -> Result<Self, ArchiveError> {
        let mut location = OsString::from(self.location.as_os_str());
        location.push(MAIN_SEPARATOR_STR);
        location.push(&entry.path);
        let location = PathBuf::from(location);
        let container = self.key_for(entry).and_then(|key| {
            self.read_container(entry, key.as_ref())
                .map_err(|error| error.at(&location))
        });
        match container {
            Ok((
                source,
                decoded_budget,
                Directory {
                    entries,
                    log,
                    installation_flags,
                    metadata,
                },
            )) => Ok(Self {
                path: self.path.clone(),
                location,
                container_chain: self.container_chain.child(entry.index),
                origin_ancestors: self.child_origins(entry),
                input_origin: self.input_origin.clone(),
                file_len: self.file_len,
                file_modified: self.file_modified,
                depth: self.depth + 1,
                decoded_budget,
                source,
                entries,
                log,
                installation_flags,
                metadata,
                build: self.build.clone(),
                build_manifest_observation: self.build_manifest_observation.clone(),
                key: self.key.clone(),
            }),
            Err(error) => Err(ArchiveError::Container {
                path: self.location.clone(),
                entry: entry.path.clone(),
                source: Box::new(error),
            }),
        }
    }

    fn read_container(
        &self,
        entry: &Entry,
        key: Option<&ArchiveKey>,
    ) -> Result<(Source, u64, Directory), FormatError> {
        if self.depth >= MAX_CONTAINER_DEPTH {
            return Err(FormatError::LimitExceeded(format!(
                "containers nest deeper than {MAX_CONTAINER_DEPTH} levels"
            )));
        }
        let (source, budget) = container::source_of(&self.source, entry, self.decoded_budget, key)?;
        let max_entries = Some(MAX_CONTAINER_ENTRIES as u64);
        let entries = container::admit(layout::read_entries(&source, max_entries)?, &source)?;
        Ok((source, budget, entries))
    }

    /// Returns the path of the Archive file, also for a container opened from it.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the path of the Archive file followed by the Entry path of every container down to
    /// this one; errors name it.
    #[must_use]
    pub fn location(&self) -> &Path {
        &self.location
    }

    /// Returns the size of the Archive file in bytes when it was opened.
    #[must_use]
    pub const fn file_len(&self) -> u64 {
        self.file_len
    }

    /// Returns how many decoded bytes this Archive keeps in memory: `0` for an Archive read from
    /// its file and for a stored container of it, the size of the decoded buffer for a compressed
    /// or encrypted container, and for a stored container of such a container the size of that
    /// buffer, which the container keeps alive.
    #[must_use]
    pub fn resident_len(&self) -> u64 {
        self.source.resident_len()
    }

    /// Returns the modification time of the Archive file when it was opened, or `None` when the
    /// platform records none.
    #[must_use]
    pub const fn file_modified(&self) -> Option<SystemTime> {
        self.file_modified
    }

    /// Returns the Entries in central-directory order.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Returns the first [`MAX_ENTRY_ISSUES`](crate::MAX_ENTRY_ISSUES) defects found in the directory, each with the
    /// position in [`entries`](Self::entries) of the Entry it belongs to, in that order; an Entry
    /// has at most one defect of each kind. An Archive without defects returns an empty slice.
    #[must_use]
    pub fn issues(&self) -> &[(usize, EntryIssue)] {
        &self.log.listed
    }

    /// Returns the number of defects found in the directory, also those
    /// [`issues`](Self::issues) no longer lists.
    #[must_use]
    pub const fn issue_count(&self) -> usize {
        self.log.count
    }

    /// Returns the path bytes as stored of the Entry at `index` in [`entries`](Self::entries) when
    /// its path is not UTF-8, in which case [`Entry::path`] holds them with invalid bytes replaced
    /// by U+FFFD, or `None` for any other Entry. Takes time logarithmic in the number of such
    /// paths.
    #[must_use]
    pub fn raw_path(&self, index: usize) -> Option<&[u8]> {
        self.log.raw_path(index)
    }

    /// Returns the Build, or `None` when it is unknown.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    /// Returns the eager original manifest-read observation shared by this Archive's containers.
    #[must_use]
    pub const fn build_manifest_observation(&self) -> &BuildManifestObservation {
        &self.build_manifest_observation
    }

    /// Opens the decrypted and decompressed data of `entry`, which must be one of this Archive's
    /// Entries.
    ///
    /// # Errors
    ///
    /// Returns [`ArchiveError::UnsupportedCompression`] for compression methods other than store,
    /// deflate and zstd, and another [`ArchiveError`] when the local header is malformed or cannot
    /// be read, or encrypted data is not a whole number of AES blocks.
    pub fn open_entry(&self, entry: &Entry) -> Result<EntryReader<'_>, ArchiveError> {
        self.open_entry_at(entry, 0)
    }

    /// Opens the data of `entry` like [`open_entry`](Self::open_entry) and checks it against the
    /// checksum of the directory while it streams, without a second pass or buffering: the read
    /// that reaches the end of the data fails with a
    /// [`DataError::ChecksumMismatch`](crate::DataError::ChecksumMismatch) when they differ.
    /// The check costs one checksum over the decoded bytes and, for a v1 Entry that states a SHA-256
    /// ([`Entry::sha256`]), one over its stored bytes; a reader that stops before the end checks
    /// nothing.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`open_entry`](Self::open_entry).
    pub fn open_entry_verified(&self, entry: &Entry) -> Result<EntryReader<'_>, ArchiveError> {
        let key = self.key_for(entry)?;
        entry_reader::open_verified(&self.source, entry, key.as_ref())
            .map_err(|error| error.at(&self.location))
    }

    /// Opens the decrypted and decompressed data of `entry` from byte `offset` on.
    ///
    /// Stored Entries start there directly, encrypted ones at the AES block holding `offset`.
    /// Compressed Entries are decoded from their start, and the bytes before `offset` are skipped,
    /// at most [`MAX_DECODED_SKIP`].
    ///
    /// # Errors
    ///
    /// Returns [`ArchiveError::OffsetBeyondEnd`] when `offset` exceeds the Entry size,
    /// [`ArchiveError::SkipTooLong`] when a compressed Entry would decode more than
    /// [`MAX_DECODED_SKIP`] bytes to reach `offset`, and the errors of [`Archive::open_entry`],
    /// or [`ArchiveError::Io`] when the skipped data cannot be read or decoded.
    pub fn open_entry_at(
        &self,
        entry: &Entry,
        offset: u64,
    ) -> Result<EntryReader<'_>, ArchiveError> {
        let size = entry.uncompressed_size();
        if offset > size {
            return Err(ArchiveError::OffsetBeyondEnd {
                path: self.location.clone(),
                entry: entry.path().to_owned(),
                offset,
                size,
            });
        }
        let key = self.key_for(entry)?;
        let (mut reader, skip) = entry_reader::open_at(&self.source, entry, offset, key.as_ref())
            .map_err(|error| error.at(&self.location))?;
        if skip > MAX_DECODED_SKIP {
            return Err(ArchiveError::SkipTooLong {
                path: self.location.clone(),
                entry: entry.path().to_owned(),
                offset,
                limit: MAX_DECODED_SKIP,
            });
        }
        io::copy(&mut (&mut reader).take(skip), &mut io::sink()).map_err(|source| {
            ArchiveError::Io {
                path: self.location.clone(),
                source,
            }
        })?;
        Ok(reader)
    }
}
