// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{Archive, Compression};
use serde::{Serialize, Serializer, ser::SerializeSeq as _};

use super::reference_digest;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::Digest;
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
    reference_digest::value(&Inventory { archive, entries })
}

struct Inventory<'a> {
    archive: &'a Archive,
    entries: &'a BTreeMap<String, usize>,
}

impl Serialize for Inventory<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.entries.len()))?;
        for index in self.entries.values() {
            let entry = self.archive.entries().get(*index).ok_or_else(|| {
                serde::ser::Error::custom("source inventory has an invalid Entry index")
            })?;
            sequence.serialize_element(&stamp(entry))?;
        }
        sequence.end()
    }
}

fn stamp(entry: &p4kx_extraction::Entry) -> EntryStamp<'_> {
    EntryStamp {
        path: entry.path(),
        method: method(entry.compression()),
        encrypted: entry.is_encrypted(),
        compressed_bytes: entry.compressed_size(),
        decoded_bytes: entry.uncompressed_size(),
        crc32c: entry.crc32c(),
        sha256: entry.sha256(),
        sha256_verified: entry.is_sha256_verified(),
    }
}

#[cfg(test)]
mod tests {
    use p4kx_testkit::TestEntry;

    use super::{entries, fingerprint, reference_digest, stamp};
    use crate::TestResult;
    use crate::manifest_fixtures::Fixture;

    #[test]
    fn streamed_fingerprint_retains_normalized_sorted_array_bytes() -> TestResult {
        let fixture = Fixture::new(vec![
            TestEntry::stored("Data/Z.xml", b"plain"),
            TestEntry::zstd(r"DATA\C.xml", b"compressed"),
            TestEntry::deflated("Data/a.bin", b"encrypted").encrypted(),
        ])?;
        let index = entries(&fixture.archive)?;
        let expected: Vec<_> = index
            .values()
            .map(|&position| stamp(&fixture.archive.entries()[position]))
            .collect();
        assert_eq!(
            fingerprint(&fixture.archive, &index)?,
            reference_digest::value(&expected)?,
            "the complete streamed inventory is hashed identically to a JSON array"
        );
        Ok(())
    }
}
