// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Read;
use std::path::{Component, Path};

use p4kx_extraction::{Archive, Checksum, Entry};
use p4kx_media::{Algorithm, ByteScope, RawReference};
use sha2::{Digest as _, Sha256};

use super::TestResult;

pub(super) fn require(condition: bool, message: &'static str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

pub(super) fn verify_original(
    archive: &Archive,
    entry: &Entry,
    base: &Path,
    raw: &RawReference,
) -> TestResult {
    let path = Path::new(&raw.path);
    require(
        path.components()
            .all(|component| matches!(component, Component::Normal(_))),
        "raw paths remain relative to the generation",
    )?;
    let retained = base.join(path);
    require(
        fs::metadata(&retained)?.len() == raw.size && raw.size == entry.uncompressed_size(),
        "retained and original sizes agree",
    )?;
    let retained = fs::read(retained)?;
    require(
        raw.digest.algorithm == Algorithm::Sha256
            && raw.digest.scope == ByteScope::DecodedEntryPayload
            && raw.digest.hex == Checksum::Sha256(Sha256::digest(&retained).into()).hex(),
        "raw input SHA-256 matches the declared scope",
    )?;
    let mut source = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut source)?;
    require(
        source == retained,
        "verified ATL input and retained bytes agree",
    )
}
