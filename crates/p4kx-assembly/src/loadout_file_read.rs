// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Display;
use std::io::Read;

use p4kx_archive::{Archive, Entry};

use crate::{LoadoutFileError, LoadoutFileSource, MAX_XML_LOADOUT_LEN};

const READ_BLOCK_BYTES: usize = 64 << 10;

/// One actual file-read outcome paired with its original admission.
#[derive(Debug)]
#[must_use]
pub struct LoadoutFileRead {
    source: LoadoutFileSource,
    outcome: Result<Vec<u8>, LoadoutFileError>,
}

impl LoadoutFileRead {
    /// Pairs a generic provider's configured path and outcome without Archive authority.
    pub fn unbound(path: &str, outcome: Result<Vec<u8>, LoadoutFileError>) -> Self {
        Self::new(LoadoutFileSource::unbound(path), outcome)
    }

    pub(crate) const fn new(
        source: LoadoutFileSource,
        outcome: Result<Vec<u8>, LoadoutFileError>,
    ) -> Self {
        Self { source, outcome }
    }

    /// Reads an owned Archive Entry to its verified end with cancellation between blocks.
    ///
    /// The read carries Archive admission only after verification succeeds.
    pub fn read_archive(archive: &Archive, entry: &Entry, stopped: &dyn Fn() -> bool) -> Self {
        let Some(observed) = archive.observed_entry(entry) else {
            return Self::unbound(
                entry.path(),
                Err(LoadoutFileError::SourceEvidenceUnavailable),
            );
        };
        match verified_bytes(archive, entry, stopped) {
            Ok(bytes) => Self::new(LoadoutFileSource::archive(observed.snapshot()), Ok(bytes)),
            Err(error) => Self::unbound(entry.path(), Err(error)),
        }
    }

    /// Borrows the original admission of this operation.
    #[must_use]
    pub const fn source(&self) -> &LoadoutFileSource {
        &self.source
    }

    /// Borrows the exact completed read outcome.
    ///
    /// # Errors
    ///
    /// Returns the original file-read failure.
    pub fn outcome(&self) -> Result<&[u8], &LoadoutFileError> {
        self.outcome.as_deref()
    }

    /// Consumes the operation and returns its exact read outcome.
    ///
    /// # Errors
    ///
    /// Returns the original file-read failure.
    pub fn into_result(self) -> Result<Vec<u8>, LoadoutFileError> {
        self.outcome
    }

    /// Moves the unchanged source and its original outcome into the intrinsic reader.
    pub(crate) fn into_parts(self) -> (LoadoutFileSource, Result<Vec<u8>, LoadoutFileError>) {
        (self.source, self.outcome)
    }
}

fn verified_bytes(
    archive: &Archive,
    entry: &Entry,
    stopped: &dyn Fn() -> bool,
) -> Result<Vec<u8>, LoadoutFileError> {
    check_stop(stopped)?;
    let size = entry.uncompressed_size();
    if size > MAX_XML_LOADOUT_LEN {
        return Err(LoadoutFileError::TooLarge { size });
    }
    let capacity = usize::try_from(size).map_err(|_| LoadoutFileError::TooLarge { size })?;
    let mut reader = archive
        .open_entry_verified(entry)
        .map_err(unreadable)?
        .reject_overlong();
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(unreadable)?;
    bytes.resize(capacity, 0);
    for block in bytes.chunks_mut(READ_BLOCK_BYTES) {
        check_stop(stopped)?;
        reader.read_exact(block).map_err(unreadable)?;
    }
    check_stop(stopped)?;
    if reader.read(&mut [0_u8; 1]).map_err(unreadable)? != 0 {
        return Err(unreadable("decoded loadout file exceeds its Entry size"));
    }
    check_stop(stopped)?;
    Ok(bytes)
}

fn check_stop(stopped: &dyn Fn() -> bool) -> Result<(), LoadoutFileError> {
    if stopped() {
        return Err(LoadoutFileError::ObservationStopped);
    }
    Ok(())
}

fn unreadable(reason: impl Display) -> LoadoutFileError {
    LoadoutFileError::Unreadable {
        reason: reason.to_string(),
    }
}
