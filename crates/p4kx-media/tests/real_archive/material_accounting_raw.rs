// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

use p4kx_extraction::{Archive, ArchiveWatch, Checksum, Entry};
use p4kx_media::{Algorithm, ByteScope, RawReference};
use sha2::{Digest as _, Sha256};

use super::super::shader_accounting::require;
use super::TestResult;

pub(super) fn check_payload(
    archive: &Archive,
    watch: &ArchiveWatch,
    entry: &Entry,
    base: &Path,
    raw: &RawReference,
    parser_limit: usize,
) -> TestResult<Option<Vec<u8>>> {
    let relative = Path::new(&raw.path);
    require(
        raw.path.starts_with("raw/")
            && relative
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
        "raw artifact path is confined to the generation",
    )?;
    require(
        raw.digest.algorithm == Algorithm::Sha256
            && raw.digest.scope == ByteScope::DecodedEntryPayload,
        "raw digest states the actual byte scope",
    )?;
    let mut retained = File::open(base.join(relative))?;
    require(
        retained.metadata()?.len() == raw.size && raw.size == entry.uncompressed_size(),
        "retained and original decoded sizes agree",
    )?;
    let mut original = archive.open_entry_verified(entry)?;
    let retain = raw.size <= u64::try_from(parser_limit)?;
    let mut bytes = retain.then(Vec::new);
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 16 << 10];
    let mut expected = [0_u8; 16 << 10];
    loop {
        watch.poll()?;
        let length = retained.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        let chunk = buffer.get(..length).ok_or("raw verification buffer")?;
        let actual = expected
            .get_mut(..length)
            .ok_or("original verification buffer")?;
        original.read_exact(actual)?;
        require(actual == chunk, "retained and original decoded bytes agree")?;
        hash.update(chunk);
        if let Some(bytes) = bytes.as_mut() {
            bytes.extend_from_slice(chunk);
        }
    }
    require(
        original.read(&mut expected)? == 0,
        "the raw artifact covers the complete Entry",
    )?;
    require(
        Checksum::Sha256(hash.finalize().into()).hex() == raw.digest.hex,
        "the raw receipt agrees with the verified bytes",
    )?;
    Ok(bytes)
}
