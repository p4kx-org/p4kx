// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::Read;
use std::path::Path;

use super::reference_digest;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_log_report::{Capability, Profile, Report, Stream};
use super::reference_log_scan::{self, Observed};
use super::reference_manifest::{ArtifactRole, MAX_STRUCTURE_BYTES, Manifest};
use super::reference_paths;

fn path<'a>(manifest: &'a Manifest, role: &ArtifactRole) -> Result<&'a str> {
    let mut found = manifest
        .artifacts
        .iter()
        .filter(|artifact| artifact.role == *role);
    let first = found
        .next()
        .ok_or_else(|| ReferenceError::new(Code::Incomplete, "required producer log is absent"))?;
    if found.next().is_some() {
        return Err(ReferenceError::new(
            Code::Incomplete,
            "producer log role is duplicated",
        ));
    }
    Ok(&first.path)
}

fn bytes(root: &Path, path: &str) -> Result<Vec<u8>> {
    let file = File::open(reference_paths::resolve(root, path)?)
        .map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?;
    if file
        .metadata()
        .map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?
        .len()
        > MAX_STRUCTURE_BYTES
    {
        return Err(ReferenceError::new(
            Code::Bounds,
            "producer log exceeds its byte bound",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_STRUCTURE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?;
    if bytes.len() as u64 > MAX_STRUCTURE_BYTES {
        return Err(ReferenceError::new(
            Code::Bounds,
            "producer log grew beyond its byte bound",
        ));
    }
    Ok(bytes)
}

fn report(root: &Path, manifest: &Manifest) -> Result<Report> {
    let stdout = bytes(root, path(manifest, &ArtifactRole::ProducerStdout)?)?;
    let stderr = bytes(root, path(manifest, &ArtifactRole::ProducerStderr)?)?;
    let mut observed = Observed::default();
    let profile = Profile::of(&manifest.producer);
    if let Some(profile) = profile {
        reference_log_scan::scan(profile, Stream::Stdout, &stdout, &mut observed)?;
        reference_log_scan::scan(profile, Stream::Stderr, &stderr, &mut observed)?;
        let payloads = manifest
            .artifacts
            .iter()
            .filter(|artifact| artifact.role == ArtifactRole::Payload)
            .count();
        reference_log_scan::finish(profile, &manifest.expected, payloads, &mut observed)?;
    }
    Ok(Report {
        schema_version: 1,
        capability: profile.map_or(Capability::ReviewRequired, |profile| {
            Capability::PinnedSourceProfile { profile }
        }),
        stdout: reference_digest::reader(stdout.as_slice(), MAX_STRUCTURE_BYTES)?,
        stderr: reference_digest::reader(stderr.as_slice(), MAX_STRUCTURE_BYTES)?,
        errors: observed.errors,
        review_required: observed.review,
    })
}

pub(crate) fn capture(root: &Path, manifest: &Manifest) -> Result<()> {
    let report = report(root, manifest)?;
    let output = path(manifest, &ArtifactRole::ProducerErrorLog)?;
    let output = reference_paths::for_write(root, output)?;
    p4kx_extraction::write_bytes_replacing(
        &output,
        &serde_json::to_vec_pretty(&report)
            .map_err(|error| ReferenceError::new(Code::Manifest, error.to_string()))?,
    )
    .map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?;
    if !report.complete() {
        return Err(ReferenceError::new(
            Code::Incomplete,
            "producer errors or review_required log capability",
        ));
    }
    Ok(())
}

pub(crate) fn validate(root: &Path, manifest: &Manifest) -> Result<()> {
    let measured = report(root, manifest)?;
    let encoded = bytes(root, path(manifest, &ArtifactRole::ProducerErrorLog)?)?;
    super::reference_limits::json_tokens(&encoded)?;
    let recorded: Report = serde_json::from_slice(&encoded)
        .map_err(|error| ReferenceError::new(Code::Incomplete, error.to_string()))?;
    if recorded != measured || !measured.complete() {
        return Err(ReferenceError::new(
            Code::Incomplete,
            "error log differs from captured streams or needs review",
        ));
    }
    Ok(())
}
