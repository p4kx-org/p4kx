// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest as _, Sha256};

use super::{Error, ErrorKind, check};

const BLOCK_BYTES: usize = 16 << 10;

/// The measured byte length and SHA-256 of a local file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint {
    /// Number of bytes actually consumed.
    pub bytes: u64,
    /// SHA-256 of those consumed bytes.
    pub sha256: [u8; 32],
}

/// Bounded file bytes and the fingerprint of those bytes.
pub struct LoadedFile {
    /// The consumed file bytes.
    pub bytes: Vec<u8>,
    /// Their measured identity.
    pub fingerprint: Fingerprint,
}

fn consume(
    path: &Path,
    max_bytes: u64,
    cancelled: impl Fn() -> bool,
    mut accept: impl FnMut(&[u8]),
) -> Result<Fingerprint, Error> {
    check(&cancelled)?;
    let mut file = File::open(path).map_err(|error| Error::new(ErrorKind::Io, error))?;
    let before = file
        .metadata()
        .map_err(|error| Error::new(ErrorKind::Io, error))?;
    if before.len() > max_bytes {
        return Err(Error::new(
            ErrorKind::Bounds,
            "reference file exceeds its byte bound",
        ));
    }
    let modified = before
        .modified()
        .map_err(|error| Error::new(ErrorKind::Io, error))?;
    let mut buffer = [0; BLOCK_BYTES];
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    loop {
        check(&cancelled)?;
        let count = file
            .read(&mut buffer)
            .map_err(|error| Error::new(ErrorKind::Io, error))?;
        if count == 0 {
            break;
        }
        bytes = bytes
            .checked_add(count as u64)
            .filter(|bytes| *bytes <= max_bytes)
            .ok_or_else(|| Error::new(ErrorKind::Bounds, "reference file grew beyond its bound"))?;
        let part = buffer
            .get(..count)
            .ok_or_else(|| Error::new(ErrorKind::Io, "invalid read count"))?;
        hasher.update(part);
        accept(part);
    }
    let after = file
        .metadata()
        .map_err(|error| Error::new(ErrorKind::Io, error))?;
    if bytes != before.len()
        || after.len() != before.len()
        || modified
            != after
                .modified()
                .map_err(|error| Error::new(ErrorKind::Io, error))?
    {
        return Err(Error::new(
            ErrorKind::Io,
            "reference file changed during reading",
        ));
    }
    Ok(Fingerprint {
        bytes,
        sha256: hasher.finalize().into(),
    })
}

/// Fingerprints a local file without retaining its contents.
///
/// # Errors
///
/// Returns an error for I/O, observed changes, cancellation or exceeded bounds.
pub fn fingerprint(
    path: &Path,
    max_bytes: u64,
    cancelled: impl Fn() -> bool,
) -> Result<Fingerprint, Error> {
    consume(path, max_bytes, cancelled, |_| {})
}

/// Reads a bounded local file and fingerprints the bytes consumed.
///
/// # Errors
///
/// Returns an error for I/O, observed changes, cancellation or exceeded bounds.
pub fn load(
    path: &Path,
    max_bytes: u64,
    cancelled: impl Fn() -> bool,
) -> Result<LoadedFile, Error> {
    let mut bytes = Vec::new();
    let fingerprint = consume(path, max_bytes, cancelled, |part| {
        bytes.extend_from_slice(part);
    })?;
    Ok(LoadedFile { bytes, fingerprint })
}
