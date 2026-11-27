// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::reference_digest;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::{
    Artifact, Fingerprint, MAX_ARTIFACT_BYTES, MAX_STRUCTURE_BYTES, Manifest,
};
use super::reference_paths;

pub(crate) fn fingerprint(artifact: &Artifact) -> Result<&Fingerprint> {
    artifact.fingerprint.as_ref().ok_or_else(|| {
        ReferenceError::new(Code::Incomplete, "artifact has no completed fingerprint")
    })
}

pub(crate) fn verify(root: &Path, manifest: &Manifest) -> Result<()> {
    let files = reference_paths::inventory(root)?;
    let expected = manifest
        .artifacts
        .iter()
        .map(|artifact| reference_paths::key(&artifact.path))
        .collect::<Result<BTreeSet<_>>>()?;
    if files.keys().cloned().collect::<BTreeSet<_>>() != expected {
        return Err(ReferenceError::new(
            Code::Coverage,
            "missing or extra artifacts in configured root",
        ));
    }
    for artifact in &manifest.artifacts {
        let path = reference_paths::resolve(root, &artifact.path)?;
        let expected = fingerprint(artifact)?;
        if reference_digest::file(&path, MAX_ARTIFACT_BYTES)? != *expected {
            return Err(ReferenceError::new(
                Code::Artifact,
                "artifact bytes differ from recorded fingerprint",
            ));
        }
    }
    super::reference_logs::validate(root, manifest)
}

pub(crate) fn read(root: &Path, artifact: &Artifact) -> Result<Vec<u8>> {
    let expected = fingerprint(artifact)?;
    if expected.bytes > MAX_STRUCTURE_BYTES {
        return Err(ReferenceError::new(
            Code::Bounds,
            "structured comparison artifact exceeds its byte bound",
        ));
    }
    let path = reference_paths::resolve(root, &artifact.path)?;
    let file =
        File::open(path).map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?;
    let mut bytes = Vec::new();
    file.take(MAX_STRUCTURE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?;
    if reference_digest::reader(bytes.as_slice(), MAX_STRUCTURE_BYTES)? != *expected {
        return Err(ReferenceError::new(
            Code::Changed,
            "comparison artifact changed after validation",
        ));
    }
    Ok(bytes)
}

pub(crate) fn capture(root: &Path, manifest: &mut Manifest) -> Result<()> {
    let files = reference_paths::inventory(root)?;
    let expected = manifest
        .artifacts
        .iter()
        .map(|artifact| reference_paths::key(&artifact.path))
        .collect::<Result<BTreeSet<_>>>()?;
    if files.keys().cloned().collect::<BTreeSet<_>>() != expected {
        return Err(ReferenceError::new(
            Code::Coverage,
            "produced artifact inventory differs from pre-conversion plan",
        ));
    }
    for artifact in &mut manifest.artifacts {
        let path = reference_paths::resolve(root, &artifact.path)?;
        artifact.fingerprint = Some(reference_digest::file(&path, MAX_ARTIFACT_BYTES)?);
    }
    super::reference_logs::validate(root, manifest)
}
