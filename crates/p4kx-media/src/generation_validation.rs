// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Component, Path};

use crate::checkpoint::Checkpoint;
use crate::error;
use crate::hashing;
use crate::origin_index;
use crate::origin_read_policy::OriginReadPolicy;
use crate::origin_verify::{self, OriginAvailability};
use crate::output_path;
use crate::{Algorithm, Artifact, Index, Limits, MediaError};

pub(crate) fn existing(
    output: &Path,
    limits: Limits,
    checkpoint: &Checkpoint<'_>,
) -> Result<Option<Index>, MediaError> {
    let path = output.join("media-index.json");
    if !output_path::file(&path)? {
        return Ok(None);
    }
    let mut index = origin_index::read(&path, limits.record_bytes, checkpoint)?;
    let fingerprint = hashing::generation(&mut index, checkpoint)?;
    if index.format != crate::MediaFormat::P4kxMedia
        || index.format_version != crate::MediaVersion::V1
        || !index.complete
        || !index.enumeration_complete
        || index.generation.len() != 64
        || !index
            .generation
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || index.artifact_base != format!("generations/{}", index.generation)
        || fingerprint != index.generation
    {
        return Err(MediaError::Destination(
            "existing index is not a complete supported generation".into(),
        ));
    }
    artifacts(
        &output.join(&index.artifact_base),
        &index,
        limits,
        checkpoint,
    )?;
    if index.field_origins.is_none() {
        return Ok(None);
    }
    Ok(Some(index))
}

pub(crate) fn artifacts(
    root: &Path,
    index: &Index,
    limits: Limits,
    checkpoint: &Checkpoint<'_>,
) -> Result<(), MediaError> {
    if !output_path::directory(root)? {
        return Err(MediaError::Destination("artifact base is missing".into()));
    }
    let mut remaining = limits.output_bytes;
    verify(root, &index.artifact_manifest, &mut remaining, checkpoint)?;
    verify(root, &index.source_manifest, &mut remaining, checkpoint)?;
    for kind in &index.kinds {
        verify(root, &kind.artifact, &mut remaining, checkpoint)?;
    }
    let path = root.join(&index.artifact_manifest.path);
    let mut reader = BufReader::new(File::open(&path).map_err(|source| error::io(&path, source))?);
    let mut count = 0_u64;
    loop {
        checkpoint.check()?;
        let mut line = Vec::new();
        let read = reader
            .by_ref()
            .take(limits.record_bytes.saturating_add(1))
            .read_until(b'\n', &mut line)
            .map_err(|source| error::io(&path, source))?;
        if read == 0 {
            break;
        }
        count += 1;
        if read as u64 > limits.record_bytes
            || count > limits.sources.saturating_add(limits.records)
        {
            return Err(MediaError::Destination(
                "artifact manifest exceeds its declared limits".into(),
            ));
        }
        let artifact: Artifact = serde_json::from_slice(&line)?;
        verify(root, &artifact, &mut remaining, checkpoint)?;
    }
    origins(root, index, checkpoint)
}

fn origins(root: &Path, index: &Index, checkpoint: &Checkpoint<'_>) -> Result<(), MediaError> {
    let Some(receipts) = &index.field_origins else {
        return Ok(());
    };
    let policy = OriginReadPolicy::from_receipts(receipts);
    if origin_verify::generation(root, index, *checkpoint, policy)? != OriginAvailability::Current {
        return Err(MediaError::Destination(
            "origin mapping is not a verified current generation".into(),
        ));
    }
    Ok(())
}

fn verify(
    root: &Path,
    artifact: &Artifact,
    remaining: &mut u64,
    checkpoint: &Checkpoint<'_>,
) -> Result<(), MediaError> {
    if artifact.size > *remaining {
        return Err(MediaError::Limit {
            budget: "artifact_verification_bytes",
            limit: *remaining,
        });
    }
    *remaining -= artifact.size;
    if artifact.digest.algorithm != Algorithm::Sha256
        || artifact.digest.hex.len() != 64
        || artifact.path.is_empty()
    {
        return Err(MediaError::Destination(
            "invalid artifact integrity declaration".into(),
        ));
    }
    let relative = Path::new(&artifact.path);
    let mut path = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(MediaError::Destination("unsafe artifact path".into()));
        }
        path.push(component);
    }
    if !output_path::file(&path)? {
        return Err(MediaError::Destination("artifact file is missing".into()));
    }
    let (digest, size) = hashing::file(&path, artifact.digest.scope, artifact.size, checkpoint)?;
    if size != artifact.size || digest != artifact.digest {
        return Err(MediaError::Destination(format!(
            "artifact integrity mismatch: {}",
            artifact.path
        )));
    }
    Ok(())
}
