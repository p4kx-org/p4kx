// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded validation of the actual shader generation and retained physical sources.

use std::path::Path;

use p4kx_extraction::ArchiveWatch;
use p4kx_media::{Algorithm, Artifact, ByteScope, Index, Source};

use super::TestResult;
use super::shader_accounting::require;
use super::shader_expected::ExpectedSource;
use super::{manifests, shader_inventory};

pub(super) const OUTPUT_BYTES: u64 = 128 << 20;
const MANIFEST_BYTES: u64 = 8 << 20;

pub(super) struct Retained {
    pub(super) source: Source,
    pub(super) bytes: Vec<u8>,
}

pub(super) fn verify(
    output: &Path,
    index: &Index,
    expected: &[ExpectedSource],
    watch: &ArchiveWatch,
) -> TestResult<Vec<Retained>> {
    let base = output.join(&index.artifact_base);
    jsonschema::validator_for(&p4kx_media::index_schema())?
        .validate(&serde_json::to_value(index)?)
        .map_err(|error| error.to_string())?;
    let manifest = checked(&base, &index.source_manifest, MANIFEST_BYTES)?;
    let mut sources = Vec::new();
    let mut remaining = shader_inventory::SOURCE_BYTES;
    for line in std::str::from_utf8(&manifest)?.lines() {
        watch.poll()?;
        let source: Source = serde_json::from_str(line)?;
        let wanted = expected
            .get(sources.len())
            .ok_or("unexpected source manifest row")?;
        require(
            source.identity == wanted.identity,
            "physical nested source identity changed",
        )?;
        require(
            source.root == index.source.digest,
            "source root digest differs from index",
        )?;
        require(source.size == wanted.size, "source size changed")?;
        let raw = source
            .raw
            .as_ref()
            .ok_or("shader source has no retained bytes")?;
        require(
            raw.digest.hex == wanted.digest,
            "retained source differs from verified input",
        )?;
        require(
            raw.size == wanted.size,
            "raw source size differs from input",
        )?;
        require(
            raw.digest.algorithm == Algorithm::Sha256,
            "raw source algorithm is not SHA256",
        )?;
        require(
            raw.digest.scope == ByteScope::DecodedEntryPayload,
            "raw byte scope is incorrect",
        )?;
        let bytes = manifests::check_payload(&base, &raw.path, raw.size, &raw.digest, remaining)?;
        remaining = remaining
            .checked_sub(raw.size)
            .ok_or("retained source budget exceeded")?;
        sources.push(Retained { source, bytes });
    }
    require(
        sources.len() == expected.len(),
        "selected source is absent from its manifest",
    )?;
    verify_artifacts(&base, index, &sources, watch)?;
    Ok(sources)
}

pub(super) fn checked(base: &Path, artifact: &Artifact, maximum: u64) -> TestResult<Vec<u8>> {
    manifests::check_payload(
        base,
        &artifact.path,
        artifact.size,
        &artifact.digest,
        maximum,
    )
}

fn verify_artifacts(
    base: &Path,
    index: &Index,
    retained: &[Retained],
    watch: &ArchiveWatch,
) -> TestResult {
    let mut pending = retained
        .iter()
        .map(|retained| {
            retained
                .source
                .raw
                .as_ref()
                .ok_or("verified source has no retained raw receipt")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let manifest = checked(base, &index.artifact_manifest, MANIFEST_BYTES)?;
    let mut remaining = OUTPUT_BYTES;
    let mut count = 0_usize;
    for line in std::str::from_utf8(&manifest)?.lines() {
        watch.poll()?;
        let artifact: Artifact = serde_json::from_str(line)?;
        let position = pending
            .iter()
            .position(|expected| {
                expected.path == artifact.path
                    && expected.size == artifact.size
                    && expected.digest == artifact.digest
            })
            .ok_or("artifact inventory has no matching physical raw-retention receipt")?;
        let _matched = pending.swap_remove(position);
        let _bytes = checked(base, &artifact, remaining)?;
        remaining = remaining
            .checked_sub(artifact.size)
            .ok_or("artifact accounting budget exceeded")?;
        count += 1;
    }
    require(
        pending.is_empty(),
        "retained raw source is absent from its artifact inventory",
    )?;
    require(
        count != 0,
        "shader generation has no artifact manifest entries",
    )
}

#[cfg(test)]
#[path = "shader_artifact_inventory_tests.rs"]
mod inventory_tests;
