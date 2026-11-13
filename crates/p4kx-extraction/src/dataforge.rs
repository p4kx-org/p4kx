// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use p4kx_archive::{Archive, ArchiveError, Entry};
use p4kx_dataforge::{DataForge, DataForgeError};
use thiserror::Error;

use crate::{ArchiveChanged, DataForgeOrigin};

/// The Entries that may hold DataForge, the preferred one first.
pub const DATAFORGE_ENTRIES: [&str; 2] = ["Data\\Game2.dcb", "Data\\Game.dcb"];

/// The largest uncompressed DataForge Entry that is read, 2 GiB; larger sizes are rejected.
pub const MAX_DATAFORGE_SIZE: u64 = 2 << 30;

const READ_CHUNK: u64 = 1 << 20;

/// The bytes of an Archive's DataForge Entry, read into memory.
#[derive(Debug)]
pub struct DataForgeEntry {
    path: String,
    bytes: Vec<u8>,
    origin: DataForgeOrigin,
}

impl DataForgeEntry {
    /// Reads the DataForge Entry of `archive`: `Data\Game2.dcb`, or `Data\Game.dcb` when the
    /// Archive has no `Game2.dcb`.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeLoadError::Missing`] when the Archive holds neither Entry,
    /// [`DataForgeLoadError::TooLarge`] when its size exceeds [`MAX_DATAFORGE_SIZE`], and another
    /// [`DataForgeLoadError`] when the Entry cannot be read.
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
        let entry = Self::locate(archive).ok_or(DataForgeLoadError::Missing)?;
        let path = entry.path().to_owned();
        let size = entry.uncompressed_size();
        check_size(&path, size)?;
        let reader = archive
            .open_entry(entry)
            .map_err(DataForgeLoadError::Archive)?;
        let origin = archive
            .entry_origin(entry)
            .map_or(DataForgeOrigin::Unknown, DataForgeOrigin::Archive);
        let mut loaded = Self::read_stream(path, size, reader, stop)?;
        loaded.set_origin(origin);
        Ok(loaded)
    }

    /// Reads `size` bytes of `reader` in mebibytes, polling `stop` before each one.
    pub(crate) fn read_stream(
        path: String,
        size: u64,
        reader: impl Read,
        stop: &dyn Fn() -> bool,
    ) -> Result<Self, DataForgeLoadError> {
        let mut bytes = Vec::new();
        let reserve = usize::try_from(size).unwrap_or(usize::MAX);
        if bytes.try_reserve_exact(reserve).is_err() {
            return Err(too_large(path, size));
        }
        let mut reader = reader.take(size);
        loop {
            if stop() {
                return Err(DataForgeLoadError::Cancelled);
            }
            let before = bytes.len();
            (&mut reader)
                .take(READ_CHUNK)
                .read_to_end(&mut bytes)
                .map_err(|source| DataForgeLoadError::Read {
                    path: path.clone(),
                    source,
                })?;
            if bytes.len() == before {
                break;
            }
        }
        bytes.shrink_to_fit();
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
    /// The Entry is larger than [`MAX_DATAFORGE_SIZE`] or than the memory that can be reserved.
    #[error("the DataForge Entry {path} claims {size} bytes, more than the limit of {limit} bytes")]
    TooLarge {
        /// The Entry path.
        path: String,
        /// The uncompressed size the Archive gives.
        size: u64,
        /// The largest size that is read.
        limit: u64,
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
    /// Returns whether reading again could succeed: the failure came from the disk, a changed
    /// file, a missing thread or a stop asked for by the caller ([`Cancelled`](Self::Cancelled)),
    /// not from the content of the Entry.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Missing | Self::TooLarge { .. } | Self::Corrupt(_) => false,
            Self::Read { source, .. } => !matches!(
                source.kind(),
                io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof
            ),
            Self::Archive(_) | Self::ArchiveChanged(_) | Self::Thread(_) | Self::Cancelled => true,
        }
    }
}

/// Returns [`DataForgeLoadError::TooLarge`] when `size` exceeds [`MAX_DATAFORGE_SIZE`].
pub(crate) fn check_size(path: &str, size: u64) -> Result<(), DataForgeLoadError> {
    check_size_within(path, size, MAX_DATAFORGE_SIZE)
}

/// Returns [`DataForgeLoadError::TooLarge`] when `size` exceeds `limit`.
pub(crate) fn check_size_within(
    path: &str,
    size: u64,
    limit: u64,
) -> Result<(), DataForgeLoadError> {
    if size > limit {
        return Err(DataForgeLoadError::TooLarge {
            path: path.to_owned(),
            size,
            limit,
        });
    }
    Ok(())
}

const fn too_large(path: String, size: u64) -> DataForgeLoadError {
    DataForgeLoadError::TooLarge {
        path,
        size,
        limit: MAX_DATAFORGE_SIZE,
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

    use super::*;

    #[test]
    fn a_generic_stream_has_no_trusted_archive_origin() -> Result<(), Box<dyn Error>> {
        let bytes = b"synthetic stream";
        let entry = DataForgeEntry::read_stream(
            "Game2.dcb".to_owned(),
            u64::try_from(bytes.len())?,
            bytes.as_slice(),
            &|| false,
        )?;
        assert!(
            matches!(entry.origin(), DataForgeOrigin::Unknown),
            "a path-like label does not establish an Archive origin"
        );
        Ok(())
    }

    #[test]
    fn the_limit_of_a_read_is_two_gibibytes_and_is_reported() {
        assert_eq!(MAX_DATAFORGE_SIZE, 2 << 30);
        assert!(check_size("a", MAX_DATAFORGE_SIZE).is_ok());
        assert!(matches!(
            check_size("a", MAX_DATAFORGE_SIZE + 1),
            Err(DataForgeLoadError::TooLarge {
                limit: MAX_DATAFORGE_SIZE,
                ..
            })
        ));
    }
}
