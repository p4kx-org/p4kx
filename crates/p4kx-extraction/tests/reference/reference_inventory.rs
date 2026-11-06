// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{Archive, Compression};
use serde::Serialize;

use super::reference_digest;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::{Digest, MAX_LOGICAL_SOURCES};
use super::reference_paths;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntryStamp<'a> {
    path: &'a str,
    method: u16,
    encrypted: bool,
    compressed_bytes: u64,
    decoded_bytes: u64,
    crc32c: Option<u32>,
    sha256: Option<[u8; 32]>,
    sha256_verified: bool,
}

const fn method(compression: Compression) -> u16 {
    match compression {
        Compression::Store => 0,
        Compression::Deflate => 8,
        Compression::Zstd => 100,
        Compression::Other(method) => method,
    }
}

pub(crate) fn entries(archive: &Archive) -> Result<BTreeMap<String, usize>> {
    if archive.entries().len() > MAX_LOGICAL_SOURCES {
        return Err(ReferenceError::new(
            Code::Bounds,
            "Archive source inventory exceeds its bound",
        ));
    }
    let mut entries = BTreeMap::new();
    for (index, entry) in archive.entries().iter().enumerate() {
        let path = reference_paths::key(entry.path())?;
        if entries.insert(path, index).is_some() {
            return Err(ReferenceError::new(
                Code::Identity,
                "Archive has ambiguous source paths",
            ));
        }
    }
    Ok(entries)
}

pub(crate) fn fingerprint(archive: &Archive, entries: &BTreeMap<String, usize>) -> Result<Digest> {
    let stamps = entries
        .values()
        .map(|index| {
            let entry = archive.entries().get(*index).ok_or_else(|| {
                ReferenceError::new(
                    Code::Identity,
                    "source inventory has an invalid Entry index",
                )
            })?;
            Ok(EntryStamp {
                path: entry.path(),
                method: method(entry.compression()),
                encrypted: entry.is_encrypted(),
                compressed_bytes: entry.compressed_size(),
                decoded_bytes: entry.uncompressed_size(),
                crc32c: entry.crc32c(),
                sha256: entry.sha256(),
                sha256_verified: entry.is_sha256_verified(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    reference_digest::value(&stamps)
}
