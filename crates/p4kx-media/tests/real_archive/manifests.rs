// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Component, Path};

use p4kx_extraction::{Archive, Checksum, Entry};
use p4kx_media::{Algorithm, Artifact, ByteScope, Digest, Identity, Index, Source};
use sha2::{Digest as _, Sha256};

use super::TestResult;

const MAX_MANIFEST_BYTES: u64 = 1 << 20;
pub(super) const MAX_ARTIFACT_BYTES: u64 = 16 << 20;

pub(super) fn verify(
    archive: &Archive,
    output: &Path,
    index: &Index,
    selected: &[&Entry],
) -> TestResult {
    let base = output.join(&index.artifact_base);
    verify_sources(archive, &base, index, selected)?;
    let inventory = checked(&base, &index.artifact_manifest, MAX_MANIFEST_BYTES)?;
    let inventory: Vec<Artifact> = std::str::from_utf8(&inventory)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    assert!(!inventory.is_empty(), "the artifact inventory is populated");
    let mut remaining = MAX_ARTIFACT_BYTES;
    for artifact in &inventory {
        let _ = checked(&base, artifact, remaining)?;
        remaining = remaining
            .checked_sub(artifact.size)
            .ok_or("artifact test budget exceeded")?;
    }
    Ok(())
}

fn verify_sources(
    archive: &Archive,
    base: &Path,
    index: &Index,
    selected: &[&Entry],
) -> TestResult {
    let sources = checked(base, &index.source_manifest, MAX_MANIFEST_BYTES)?;
    let sources: Vec<Source> = std::str::from_utf8(&sources)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    assert_eq!(
        sources.len(),
        selected.len(),
        "the independent source manifest is complete"
    );
    for (source, entry) in sources.iter().zip(selected.iter().copied()) {
        let physical = archive
            .entries()
            .iter()
            .position(|candidate| std::ptr::eq(candidate, entry))
            .ok_or("selected physical Entry absent")?;
        assert!(
            matches!(&source.identity, Identity::Entry { containers, entry: identity }
            if containers.is_empty() && identity.entry_index == physical && identity.path == entry.path()),
            "the manifest retains exact physical source identity"
        );
        assert!(
            source.root == index.source.digest,
            "the source is bound to this root digest"
        );
        let raw = source.raw.as_ref().ok_or("manifest raw reference absent")?;
        assert_eq!(
            raw.size,
            entry.uncompressed_size(),
            "retained decoded size agrees"
        );
        let bytes = check_payload(base, &raw.path, raw.size, &raw.digest, MAX_ARTIFACT_BYTES)?;
        assert_eq!(bytes.len() as u64, raw.size, "the raw reference is usable");
    }
    Ok(())
}

fn checked(base: &Path, artifact: &Artifact, maximum: u64) -> TestResult<Vec<u8>> {
    check_payload(
        base,
        &artifact.path,
        artifact.size,
        &artifact.digest,
        maximum,
    )
}

pub(super) fn check_payload(
    base: &Path,
    path: &str,
    size: u64,
    digest: &Digest,
    maximum: u64,
) -> TestResult<Vec<u8>> {
    assert!(
        Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_))),
        "artifact paths remain relative to the generation"
    );
    let file = base.join(path);
    assert!(
        fs::metadata(&file)?.len() <= maximum,
        "the real verification stays bounded"
    );
    let bytes = fs::read(file)?;
    assert_eq!(bytes.len() as u64, size, "artifact length agrees");
    assert_eq!(
        digest.algorithm,
        Algorithm::Sha256,
        "artifact digest algorithm"
    );
    assert!(
        digest.scope == ByteScope::ArtifactFile
            || (path.starts_with("raw/") && digest.scope == ByteScope::DecodedEntryPayload),
        "the digest names the actual retained byte scope"
    );
    assert!(
        Checksum::Sha256(Sha256::digest(&bytes).into()).hex() == digest.hex,
        "artifact hash agrees without printing source identity or bytes"
    );
    Ok(bytes)
}
