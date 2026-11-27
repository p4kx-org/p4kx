// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use p4kx_extraction::Checksum;
use p4kx_media::{
    Algorithm, Artifact, Availability, Identity, Index, MediaKind, MediaRecord, Source, Status,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{TestResult, fixture};

const POSITIVE: &[(MediaKind, &str)] = &[
    (MediaKind::Archive, "archive-entry"),
    (MediaKind::Shader, "shader-property"),
    (MediaKind::Material, "material-library-header"),
    (MediaKind::Texture, "texture.dds-image"),
    (MediaKind::Model, "model-mesh"),
    (MediaKind::Animation, "animation-controller"),
    (
        MediaKind::AnimationDefinition,
        "animation-definition-named-caf",
    ),
    (MediaKind::Audio, "audio-wem-conversion"),
    (MediaKind::AudioControl, "audio-control-definition"),
];

pub(super) fn generation(first: &Path, second: &Path, index: &Index) -> TestResult {
    let built_in = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for &(kind, namespace) in POSITIVE {
        provider(first, second, index, kind, namespace, &built_in)?;
    }
    manifests(first, second, index)
}

fn provider(
    first: &Path,
    second: &Path,
    index: &Index,
    kind: MediaKind,
    namespace: &str,
    built_in: &jsonschema::Validator,
) -> TestResult {
    let stream = index
        .kinds
        .iter()
        .find(|row| row.provider.kind == kind)
        .ok_or("missing production provider")?;
    assert!(
        matches!(stream.provider.availability, Availability::Available),
        "{kind:?} is available"
    );
    let bytes = same_artifact(first, second, &stream.artifact)?;
    let rows = rows::<Value>(&bytes)?;
    assert_eq!(
        stream.counts.records.written,
        rows.len() as u64,
        "{kind:?} record tally"
    );
    let mut positive = false;
    let mut unsupported_cache = false;
    for row in rows {
        built_in
            .validate(&row)
            .map_err(|error| format!("{kind:?}: {error}: {row}"))?;
        let record = serde_json::from_value::<MediaRecord>(row)?;
        assert_eq!(record.kind, kind, "rows belong to their own provider");
        assert_eq!(
            record.status,
            Status::Complete,
            "each advertised projection succeeded"
        );
        positive |= record.subresource.namespace == namespace;
        if kind == MediaKind::Shader && record.subresource.namespace == "shader-cache-inventory" {
            unsupported_cache = true;
            assert!(
                record
                    .issues
                    .iter()
                    .any(|issue| issue.code == "shader.cache.unsupported-semantics"),
                "cache semantics stay explicitly unsupported"
            );
        }
    }
    assert!(positive, "{kind:?} has a positive {namespace} row");
    if kind == MediaKind::Shader {
        assert!(
            unsupported_cache,
            "selected cache carries a typed inventory row"
        );
        assert!(
            stream
                .provider
                .field_groups
                .iter()
                .any(|group| { group.name == "compiled-cache-semantics" && !group.projected }),
            "shader advertises unsupported cache semantics explicitly"
        );
    }
    Ok(())
}

fn manifests(first: &Path, second: &Path, index: &Index) -> TestResult {
    let sources = same_artifact(first, second, &index.source_manifest)?;
    let artifacts = same_artifact(first, second, &index.artifact_manifest)?;
    let receipts = index
        .field_origins
        .as_ref()
        .ok_or("current field origins")?;
    for artifact in [
        &receipts.sources,
        &receipts.uses,
        &receipts.sets,
        &receipts.fields,
    ] {
        same_artifact(first, second, artifact)?;
    }
    let mut paths = BTreeSet::new();
    let mut png = false;
    let mut ogg = false;
    for artifact in rows::<Artifact>(&artifacts)? {
        assert!(
            paths.insert(artifact.path.clone()),
            "once per artifact path"
        );
        let bytes = same_artifact(first, second, &artifact)?;
        if let Some(extension) = Path::new(&artifact.path).extension() {
            if extension.eq_ignore_ascii_case("png") {
                assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "actual PNG bytes");
                png = true;
            }
            if extension.eq_ignore_ascii_case("ogg") {
                assert!(bytes.starts_with(b"OggS"), "actual Ogg bytes");
                ogg = true;
            }
        }
    }
    assert!(
        png && ogg,
        "both real converted artifact formats are present"
    );
    for base in [first, second] {
        assert_eq!(
            sidecars(base)?,
            paths,
            "manifest covers every emitted sidecar exactly"
        );
    }
    source_rows(first, index, &sources, &paths)
}

fn sidecars(base: &Path) -> TestResult<BTreeSet<String>> {
    let mut paths = BTreeSet::new();
    for directory in ["raw", "chunks"] {
        for entry in fs::read_dir(base.join(directory))? {
            let entry = entry?;
            assert!(
                entry.file_type()?.is_file(),
                "store contains only regular files"
            );
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "artifact name is not UTF-8")?;
            paths.insert(format!("{directory}/{name}"));
        }
    }
    Ok(paths)
}

fn source_rows(
    first: &Path,
    index: &Index,
    bytes: &[u8],
    artifacts: &BTreeSet<String>,
) -> TestResult {
    let schema = jsonschema::validator_for(&p4kx_media::source_schema())?;
    let rows = rows::<Value>(bytes)?;
    assert_eq!(
        index.source_entries.written,
        rows.len() as u64,
        "retained identity count"
    );
    assert!(
        index.source_entries.skipped > 0,
        "filter skips are separate"
    );
    let mut duplicates = Vec::new();
    let mut cache = false;
    for row in rows {
        schema.validate(&row).map_err(|error| error.to_string())?;
        let source = serde_json::from_value::<Source>(row)?;
        let Identity::Entry { containers, entry } = source.identity else {
            return Err("expected an original physical Entry".into());
        };
        let path = entry.path.replace('\\', "/");
        assert_ne!(
            path,
            fixture::FILTERED,
            "excluded payload has no raw receipt"
        );
        let raw = source.raw.ok_or("retained source payload")?;
        assert!(
            artifacts.contains(&raw.path),
            "raw source is covered by the artifact manifest"
        );
        let payload = fs::read(first.join(&raw.path))?;
        assert_eq!(raw.size, payload.len() as u64, "raw payload size");
        assert_eq!(raw.digest.hex, hash(&payload), "raw payload hash");
        if path == fixture::DUPLICATE {
            assert_eq!(containers.len(), 1, "the exact nested owner is retained");
            duplicates.push((entry.entry_index, payload));
        } else if path == fixture::CACHE {
            assert_eq!(
                payload,
                fixture::CACHE_BYTES,
                "unknown cache preserves its bytes"
            );
            cache = true;
        }
    }
    assert_eq!(
        duplicates,
        [(0, fixture::FIRST.to_vec()), (1, fixture::SECOND.to_vec())],
        "duplicate source names retain their physical ordinals, order and exact bytes"
    );
    assert!(cache, "unsupported selected cache has a raw manifest row");
    Ok(())
}

fn same_artifact(first: &Path, second: &Path, artifact: &Artifact) -> TestResult<Vec<u8>> {
    let primary = fs::read(first.join(&artifact.path))?;
    assert_eq!(
        primary,
        fs::read(second.join(&artifact.path))?,
        "exact repeated bytes at {}",
        artifact.path
    );
    assert_eq!(
        artifact.size,
        primary.len() as u64,
        "artifact.length for {}",
        artifact.path
    );
    assert_eq!(
        artifact.digest.algorithm,
        Algorithm::Sha256,
        "artifact digest algorithm"
    );
    assert_eq!(
        artifact.digest.hex,
        hash(&primary),
        "independent SHA-256 for {}",
        artifact.path
    );
    Ok(primary)
}

fn rows<T: DeserializeOwned>(bytes: &[u8]) -> TestResult<Vec<T>> {
    std::str::from_utf8(bytes)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn hash(bytes: &[u8]) -> String {
    Checksum::Sha256(Sha256::digest(bytes).into()).hex()
}
