// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::TryReserveError;
use std::io::{self, Read};

use p4kx_archive::{Archive, ArchiveError, Entry};
use p4kx_dataforge::{DataForge, DataForgeError};
use thiserror::Error;

use crate::decoded_buffer::{AppendError, DecodedBuffer};
use crate::source::{SourceEntry, SourceReader};
use crate::{ArchiveChanged, DataForgeOrigin, DataForgeReadOptions};

/// The Entries that may hold DataForge, the preferred one first.
pub const DATAFORGE_ENTRIES: [&str; 2] = ["Data\\Game2.dcb", "Data\\Game.dcb"];

const READ_CHUNK: usize = 1 << 20;

/// The bytes of an Archive's DataForge Entry, read into memory.
#[derive(Debug)]
pub struct DataForgeEntry {
    path: String,
    bytes: Vec<u8>,
    origin: DataForgeOrigin,
}

impl DataForgeEntry {
    /// Binds the bytes of one completed decoded read to its physical input.
    pub(crate) fn from_decoded_source(
        entry: &SourceEntry<'_>,
        reader: &SourceReader<'_>,
        bytes: Vec<u8>,
    ) -> Self {
        let origin = entry.observed_entry().map_or_else(
            || {
                reader
                    .file_origin()
                    .map_or(DataForgeOrigin::Unknown, DataForgeOrigin::File)
            },
            |entry| DataForgeOrigin::Archive(entry.snapshot()),
        );
        Self {
            path: entry.path().to_owned(),
            bytes,
            origin,
        }
    }

    /// Reads the DataForge Entry of `archive`: `Data\Game2.dcb`, or `Data\Game.dcb` when the
    /// Archive has no `Game2.dcb`.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::read_until_with_options`].
    pub fn read(archive: &Archive) -> Result<Self, DataForgeLoadError> {
        Self::read_until(archive, &|| false)
    }

    /// Reads the DataForge Entry like [`DataForgeEntry::read`], polling `stop` before each
    /// mebibyte.
    ///
    /// # Errors
    ///
    /// The same as [`DataForgeEntry::read`], and [`DataForgeLoadError::Cancelled`] once `stop`
    /// returned `true`.
    pub fn read_until(
        archive: &Archive,
        stop: &dyn Fn() -> bool,
    ) -> Result<Self, DataForgeLoadError> {
        Self::read_until_with_options(archive, DataForgeReadOptions::default(), stop)
    }

    /// Reads the Archive's DataForge Entry with caller-selected decoded input admission.
    ///
    /// # Errors
    ///
    /// Returns a missing Entry, admission, allocation, read or cancellation error.
    pub fn read_until_with_options(
        archive: &Archive,
        options: DataForgeReadOptions,
        stop: &dyn Fn() -> bool,
    ) -> Result<Self, DataForgeLoadError> {
        let entry = Self::locate(archive).ok_or(DataForgeLoadError::Missing)?;
        let path = entry.path().to_owned();
        let input = options.admit(&path, entry.uncompressed_size())?;
        if stop() {
            return Err(DataForgeLoadError::Cancelled);
        }
        let reader = archive
            .open_entry(entry)
            .map_err(DataForgeLoadError::Archive)?
            .reject_overlong();
        let origin = archive
            .entry_origin(entry)
            .map_or(DataForgeOrigin::Unknown, DataForgeOrigin::Archive);
        let mut loaded = Self::read_stream(path, input, reader, stop)?;
        loaded.set_origin(origin);
        Ok(loaded)
    }

    pub(crate) fn read_stream(
        path: String,
        input: DecodedBuffer,
        mut reader: impl Read,
        stop: &dyn Fn() -> bool,
    ) -> Result<Self, DataForgeLoadError> {
        let mut bytes = Vec::new();
        if stop() {
            return Err(DataForgeLoadError::Cancelled);
        }
        let mut scratch = Vec::new();
        let chunk_size = input.size().clamp(1, READ_CHUNK);
        scratch
            .try_reserve_exact(chunk_size)
            .map_err(|source| DataForgeLoadError::allocation(&path, chunk_size, source))?;
        scratch.resize(chunk_size, 0_u8);
        loop {
            if stop() {
                return Err(DataForgeLoadError::Cancelled);
            }
            let remaining = input.size().saturating_sub(bytes.len());
            let admitted = remaining.saturating_add(1).min(scratch.len());
            let window = scratch.get_mut(..admitted).ok_or_else(|| {
                DataForgeLoadError::read(
                    &path,
                    io::Error::new(io::ErrorKind::InvalidData, "the reader window is invalid"),
                )
            })?;
            let count = match reader.read(window) {
                Ok(0) => break,
                Ok(count) => count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(source) => return Err(DataForgeLoadError::read(&path, source)),
            };
            let chunk = window.get(..count).ok_or_else(|| {
                DataForgeLoadError::read(
                    &path,
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "the reader returned an invalid count",
                    ),
                )
            })?;
            input
                .append(&mut bytes, chunk)
                .map_err(|error| match error {
                    AppendError::Overlong(source) => DataForgeLoadError::read(&path, source),
                    AppendError::Allocation { requested, source } => {
                        DataForgeLoadError::allocation(&path, requested, source)
                    }
                })?;
        }
        input
            .finish(&bytes)
            .map_err(|source| DataForgeLoadError::read(&path, source))?;
        Ok(Self {
            path,
            bytes,
            origin: DataForgeOrigin::Unknown,
        })
    }

    /// Returns the Entry [`DataForgeEntry::read`] reads, or `None` when the Archive holds no
    /// DataForge; case and separator kind of the path do not matter.
    #[must_use]
    pub fn locate(archive: &Archive) -> Option<&Entry> {
        DATAFORGE_ENTRIES.iter().find_map(|wanted| {
            archive
                .entries()
                .iter()
                .find(|entry| same_path(entry.path(), wanted))
        })
    }

    /// Returns the path of the Entry the DataForge was read from.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the measured number of original payload bytes supplied to DataForge.
    #[must_use]
    pub const fn byte_len(&self) -> u64 {
        self.bytes.len() as u64
    }

    pub(crate) fn set_origin(&mut self, origin: DataForgeOrigin) {
        self.origin = origin;
    }

    /// Returns the physical input retained when these bytes were read.
    /// Standalone files retain their own input; generic streams remain Unknown.
    #[must_use]
    pub const fn origin(&self) -> &DataForgeOrigin {
        &self.origin
    }

    /// Opens the DataForge over the Entry's bytes.
    ///
    /// # Errors
    ///
    /// Returns a [`DataForgeError`] for an unsupported version or a corrupt database.
    pub fn open(&self) -> Result<DataForge<'_>, DataForgeError> {
        DataForge::open(&self.bytes)
    }

    /// Returns the physical origin when the DataForge uses this exact opened input.
    #[must_use]
    pub fn origin_of(&self, forge: &DataForge<'_>) -> Option<&DataForgeOrigin> {
        forge.is_opened_over(&self.bytes).then_some(&self.origin)
    }
}

/// The DataForge Entry of an Archive cannot be found or read.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DataForgeLoadError {
    /// The Archive holds neither `Data\Game2.dcb` nor `Data\Game.dcb`.
    #[error("the Archive holds no DataForge: neither Data\\Game2.dcb nor Data\\Game.dcb")]
    Missing,
    /// The decoded size does not fit a byte vector on this platform.
    #[error(
        "the DataForge input {path} claims {size} bytes, beyond the addressable size of {limit} bytes"
    )]
    TooLarge {
        /// The Entry or file path.
        path: String,
        /// The declared decoded size.
        size: u64,
        /// The platform byte-vector limit.
        limit: u64,
    },
    /// The caller declined the declared decoded size.
    #[error(
        "the DataForge input {path} claims {size} bytes, beyond the caller's budget of {limit} bytes"
    )]
    BudgetExceeded {
        /// The Entry or file path.
        path: String,
        /// The declared decoded size.
        size: u64,
        /// The caller's decoded-byte budget.
        limit: u64,
    },
    /// The allocator refused storage for observed data or decoder scratch.
    #[error("cannot reserve {requested} bytes for the DataForge input {path}")]
    Allocation {
        /// The Entry or file path.
        path: String,
        /// The requested capacity.
        requested: usize,
        /// The allocator error.
        #[source]
        source: TryReserveError,
    },
    /// The Entry cannot be opened in the Archive.
    #[error(transparent)]
    Archive(ArchiveError),
    /// Reading, decrypting or decompressing the Entry failed.
    #[error("cannot read the DataForge Entry {path}")]
    Read {
        /// The Entry path.
        path: String,
        /// The failed read.
        #[source]
        source: io::Error,
    },
    /// The Entry was read but holds no DataForge that can be opened.
    #[error("the DataForge cannot be opened")]
    Corrupt(#[source] DataForgeError),
    /// The Archive changed on disk while its DataForge was read.
    #[error(transparent)]
    ArchiveChanged(ArchiveChanged),
    /// No thread could be started to hold the DataForge.
    #[error("cannot start the DataForge thread")]
    Thread(#[source] io::Error),
    /// The read was stopped by the caller before it ended.
    #[error("the DataForge read was cancelled")]
    Cancelled,
}

impl DataForgeLoadError {
    /// Returns whether a fresh read could succeed without repairing the input format.
    /// A different caller budget, changed resources or cancellation can affect a retry.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Missing | Self::TooLarge { .. } | Self::Corrupt(_) => false,
            Self::Read { source, .. } => !matches!(
                source.kind(),
                io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof
            ),
            Self::BudgetExceeded { .. }
            | Self::Allocation { .. }
            | Self::Archive(_)
            | Self::ArchiveChanged(_)
            | Self::Thread(_)
            | Self::Cancelled => true,
        }
    }

    fn read(path: &str, source: io::Error) -> Self {
        Self::Read {
            path: path.to_owned(),
            source,
        }
    }

    fn allocation(path: &str, requested: usize, source: TryReserveError) -> Self {
        Self::Allocation {
            path: path.to_owned(),
            requested,
            source,
        }
    }
}

fn same_path(left: &str, right: &str) -> bool {
    let key = |byte: u8| {
        if byte == b'/' {
            b'\\'
        } else {
            byte.to_ascii_lowercase()
        }
    };
    left.len() == right.len() && left.bytes().map(key).eq(right.bytes().map(key))
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{DataForgeEntry, DataForgeLoadError, DataForgeOrigin, DataForgeReadOptions};

    #[test]
    fn a_generic_stream_has_no_trusted_archive_origin() -> Result<(), Box<dyn Error>> {
        let bytes = b"synthetic stream";
        let input =
            DataForgeReadOptions::default().admit("Game2.dcb", u64::try_from(bytes.len())?)?;
        let entry =
            DataForgeEntry::read_stream("Game2.dcb".to_owned(), input, bytes.as_slice(), &|| {
                false
            })?;
        assert!(
            matches!(entry.origin(), DataForgeOrigin::Unknown),
            "a path-like label does not establish an Archive origin"
        );
        Ok(())
    }

    #[test]
    fn an_allocation_error_does_not_poison_cached_source_content() -> Result<(), &'static str> {
        let mut bytes = Vec::<u8>::new();
        let source = bytes
            .try_reserve_exact(usize::MAX)
            .err()
            .ok_or("checked allocation refusal")?;
        let allocation = DataForgeLoadError::allocation("Game2.dcb", 16, source);
        assert!(
            allocation.is_transient(),
            "allocator refusal can change between reads"
        );
        Ok(())
    }

    #[test]
    fn cancellation_stops_before_the_stream_is_touched() -> Result<(), Box<dyn Error>> {
        let input = DataForgeReadOptions::default().admit("Game2.dcb", 3)?;
        let source = std::io::repeat(1);
        let result = DataForgeEntry::read_stream("Game2.dcb".to_owned(), input, source, &|| true);
        assert!(
            matches!(result, Err(DataForgeLoadError::Cancelled)),
            "initial stop takes priority"
        );
        Ok(())
    }
}
