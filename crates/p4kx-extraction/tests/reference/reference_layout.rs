// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::reference_digest;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::{
    Digest, MAX_ARTIFACTS, MAX_LOGICAL_SOURCES, Manifest, SCHEMA_VERSION, Tool,
};

pub(crate) fn expected_digest(manifest: &Manifest) -> Result<Digest> {
    let artifacts: Vec<_> = manifest
        .artifacts
        .iter()
        .map(|artifact| (&artifact.path, &artifact.role))
        .collect();
    reference_digest::value(&(
        manifest.schema_version,
        &manifest.producer,
        &manifest.source,
        &manifest.selection,
        &manifest.expected,
        artifacts,
    ))
}

pub(crate) fn shape(manifest: &Manifest, tool: Tool, finalized: bool) -> Result<()> {
    if manifest.schema_version != SCHEMA_VERSION || manifest.producer.tool != tool {
        return Err(ReferenceError::new(
            Code::Manifest,
            "unsupported schema or wrong producer adapter",
        ));
    }
    if manifest.expected.len() > MAX_LOGICAL_SOURCES || manifest.artifacts.len() > MAX_ARTIFACTS {
        return Err(ReferenceError::new(
            Code::Bounds,
            "manifest inventory exceeds its bound",
        ));
    }
    super::reference_provenance::producer(manifest)?;
    super::reference_provenance::dataset(manifest)?;
    super::reference_provenance::selection(manifest)?;
    super::reference_mappings::validate(manifest, tool)?;
    reference_digest::validate(&manifest.preparation.expected_digest)?;
    if expected_digest(manifest)? != manifest.preparation.expected_digest {
        return Err(ReferenceError::new(
            Code::Provenance,
            "pre-conversion source/selection/mapping changed",
        ));
    }
    if finalized
        && (!manifest.preparation.finalized
            || !manifest.completion.owner_asserts_success
            || manifest.completion.exit_code != Some(0)
            || !manifest.completion.errors.is_empty())
    {
        return Err(ReferenceError::new(
            Code::Incomplete,
            "producer completion is absent or reports errors",
        ));
    }
    Ok(())
}
