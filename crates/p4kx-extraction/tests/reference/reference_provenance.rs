// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::reference_digest;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::{MAX_PATH_BYTES, Manifest, PatchScope};
use super::reference_paths;

fn required(text: &str, maximum: usize) -> Result<()> {
    if text.trim().is_empty() || text.len() > maximum {
        return Err(ReferenceError::new(
            Code::Provenance,
            "missing or oversized provenance text",
        ));
    }
    Ok(())
}

pub(crate) fn producer(manifest: &Manifest) -> Result<()> {
    required(&manifest.producer.version, 256)?;
    let revision = &manifest.producer.revision;
    if !matches!(revision.len(), 40 | 64) || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ReferenceError::new(
            Code::Provenance,
            "missing immutable producer revision",
        ));
    }
    reference_digest::validate(&manifest.producer.binary.digest)?;
    if manifest.producer.binary.bytes == 0 {
        return Err(ReferenceError::new(
            Code::Provenance,
            "producer binary identity is empty",
        ));
    }
    if let PatchScope::Local {
        description,
        digest,
    } = &manifest.producer.patch_scope
    {
        required(description, MAX_PATH_BYTES)?;
        reference_digest::validate(digest)?;
    }
    Ok(())
}

pub(crate) fn dataset(manifest: &Manifest) -> Result<()> {
    let build = &manifest.source.build;
    for text in [
        &build.channel,
        &build.branch,
        &build.version,
        &build.change_number,
        &build.build_id,
    ] {
        required(text, 256)?;
    }
    reference_digest::validate(&manifest.source.archive.digest)?;
    reference_digest::validate(&manifest.source.inventory)?;
    if manifest.source.archive.bytes == 0 {
        return Err(ReferenceError::new(
            Code::Provenance,
            "source Archive fingerprint is empty",
        ));
    }
    if let Some(dcb) = &manifest.source.dcb {
        reference_paths::relative(&dcb.entry)?;
        reference_digest::validate(&dcb.fingerprint.digest)?;
        if dcb.fingerprint.bytes == 0 {
            return Err(ReferenceError::new(
                Code::Provenance,
                "source DCB fingerprint is empty",
            ));
        }
    }
    Ok(())
}

pub(crate) fn selection(manifest: &Manifest) -> Result<()> {
    required(&manifest.selection.declaration, MAX_PATH_BYTES)?;
    if manifest.expected.is_empty()
        && manifest
            .selection
            .empty_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty())
    {
        return Err(ReferenceError::new(
            Code::Selection,
            "empty selection requires an explicit reason",
        ));
    }
    if manifest
        .selection
        .empty_reason
        .as_ref()
        .is_some_and(|reason| reason.len() > MAX_PATH_BYTES)
        || manifest.completion.errors.len() > 1000
        || manifest
            .completion
            .errors
            .iter()
            .any(|error| error.len() > MAX_PATH_BYTES)
    {
        return Err(ReferenceError::new(
            Code::Bounds,
            "selection or completion details exceed their bound",
        ));
    }
    Ok(())
}
