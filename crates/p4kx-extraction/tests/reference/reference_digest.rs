// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use serde::Serialize;
use sha2::{Digest as _, Sha256};

use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::{Algorithm, Digest, Fingerprint};

const BLOCK_BYTES: usize = 16 << 10;

pub(crate) fn validate(digest: &Digest) -> Result<()> {
    if digest.value.len() != 64
        || !digest
            .value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ReferenceError::new(
            Code::Provenance,
            "invalid canonical SHA-256 digest",
        ));
    }
    Ok(())
}

fn hex_digit(nibble: u8) -> char {
    const DECIMAL_DIGITS: u8 = 10;
    if nibble < DECIMAL_DIGITS {
        char::from(b'0' + nibble)
    } else {
        char::from(b'a' + nibble - DECIMAL_DIGITS)
    }
}

pub(crate) fn finish(hasher: Sha256) -> Digest {
    const NIBBLE_BITS: u32 = 4;
    const NIBBLE_MASK: u8 = 0x0f;
    const DIGITS_PER_BYTE: usize = 2;
    let bytes = hasher.finalize();
    let mut value = String::with_capacity(bytes.len() * DIGITS_PER_BYTE);
    for byte in bytes {
        value.push(hex_digit(byte >> NIBBLE_BITS));
        value.push(hex_digit(byte & NIBBLE_MASK));
    }
    Digest {
        algorithm: Algorithm::Sha256,
        value,
    }
}

pub(crate) fn reader(mut reader: impl Read, limit: u64) -> Result<Fingerprint> {
    let mut hasher = Sha256::new();
    let mut buffer = [0; BLOCK_BYTES];
    let mut bytes = 0_u64;
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?;
        if count == 0 {
            break;
        }
        bytes = bytes
            .checked_add(count as u64)
            .filter(|bytes| *bytes <= limit)
            .ok_or_else(|| {
                ReferenceError::new(Code::Bounds, "fingerprinted input exceeds its bound")
            })?;
        hasher.update(buffer.get(..count).unwrap_or_default());
    }
    Ok(Fingerprint {
        bytes,
        digest: finish(hasher),
    })
}

pub(crate) fn file(path: &Path, limit: u64) -> Result<Fingerprint> {
    let file =
        File::open(path).map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?;
    let before = file
        .metadata()
        .map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?;
    if before.len() > limit {
        return Err(ReferenceError::new(
            Code::Bounds,
            "artifact exceeds its declared size bound",
        ));
    }
    let modified = before
        .modified()
        .map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?;
    let result = reader(&file, limit)?;
    let after = file
        .metadata()
        .map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?;
    if result.bytes != before.len()
        || after.len() != before.len()
        || modified
            != after
                .modified()
                .map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?
    {
        return Err(ReferenceError::new(
            Code::Changed,
            "file changed while fingerprinting",
        ));
    }
    Ok(result)
}

struct HashWriter(Sha256);

impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) fn value(value: &impl Serialize) -> Result<Digest> {
    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(&mut writer, value)
        .map_err(|error| ReferenceError::new(Code::Manifest, error.to_string()))?;
    Ok(finish(writer.0))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_fingerprint_matches_the_standard_sha256_abc_vector() -> super::Result<()> {
        let fingerprint = super::reader(b"abc".as_slice(), 3)?;
        assert_eq!(fingerprint.bytes, 3, "fingerprinting counts consumed bytes");
        assert_eq!(
            fingerprint.digest.value,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "SHA-256 and its canonical hexadecimal encoding match the standard vector"
        );
        Ok(())
    }
}
