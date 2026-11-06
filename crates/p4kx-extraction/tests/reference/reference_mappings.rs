// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use super::reference_digest;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_identity::{self, LogicalKey};
use super::reference_manifest::{
    Artifact, ArtifactRole, Expected, Location, MANIFEST_NAME, MAX_ARTIFACT_BYTES, MAX_ARTIFACTS,
    Manifest, Tool,
};
use super::reference_paths;

fn artifacts(manifest: &Manifest) -> Result<BTreeMap<String, &Artifact>> {
    let mut artifacts = BTreeMap::new();
    for artifact in &manifest.artifacts {
        let key = reference_paths::key(&artifact.path)?;
        if key == MANIFEST_NAME {
            return Err(ReferenceError::new(
                Code::Path,
                "root manifest is not a payload artifact",
            ));
        }
        if artifacts.insert(key, artifact).is_some() {
            return Err(ReferenceError::new(
                Code::Path,
                "duplicate/case-colliding artifact paths",
            ));
        }
        if let Some(fingerprint) = &artifact.fingerprint {
            reference_digest::validate(&fingerprint.digest)?;
            if fingerprint.bytes > MAX_ARTIFACT_BYTES {
                return Err(ReferenceError::new(
                    Code::Bounds,
                    "artifact exceeds its size bound",
                ));
            }
        } else if manifest.preparation.finalized {
            return Err(ReferenceError::new(
                Code::Incomplete,
                "finalized artifact lacks a fingerprint",
            ));
        }
    }
    Ok(artifacts)
}

struct Mappings<'a> {
    tool: Tool,
    artifacts: BTreeMap<String, &'a Artifact>,
    sources: BTreeSet<LogicalKey>,
    locations: BTreeSet<(String, Option<String>)>,
    payloads: BTreeSet<String>,
}

impl Mappings<'_> {
    fn source(&mut self, expected: &Expected) -> Result<()> {
        let identity = reference_identity::key(&expected.identity)?;
        if !matches!(
            (&identity, self.tool),
            (LogicalKey::Entry(_), Tool::Unp4k | Tool::Unforge)
                | (LogicalKey::Record { .. }, Tool::Starbreaker)
        ) {
            return Err(ReferenceError::new(
                Code::Identity,
                "source identity does not match adapter",
            ));
        }
        if !self.sources.insert(identity) || expected.targets.is_empty() {
            return Err(ReferenceError::new(
                Code::Identity,
                "duplicate identity or source without a target",
            ));
        }
        if self.tool != Tool::Starbreaker && expected.targets.len() != 1 {
            return Err(ReferenceError::new(
                Code::Identity,
                "Entry comparisons require one output per source",
            ));
        }
        for target in &expected.targets {
            self.target(target)?;
        }
        Ok(())
    }

    fn target(&mut self, target: &Location) -> Result<()> {
        if self.locations.len() >= MAX_ARTIFACTS {
            return Err(ReferenceError::new(
                Code::Bounds,
                "too many source-to-artifact mappings",
            ));
        }
        let path = reference_paths::key(&target.path)?;
        let artifact = self.artifacts.get(&path).ok_or_else(|| {
            ReferenceError::new(Code::Coverage, "mapping names an undeclared artifact")
        })?;
        if artifact.role != ArtifactRole::Payload {
            return Err(ReferenceError::new(
                Code::Identity,
                "mapping names a non-payload artifact",
            ));
        }
        if let Some(pointer) = &target.subresource {
            if self.tool != Tool::Starbreaker {
                return Err(ReferenceError::new(
                    Code::Identity,
                    "Entry source cannot use JSON subresources",
                ));
            }
            reference_paths::pointer(pointer)?;
        }
        if !self
            .locations
            .insert((path.clone(), target.subresource.clone()))
        {
            return Err(ReferenceError::new(
                Code::Identity,
                "sources collide at an output location",
            ));
        }
        self.payloads.insert(path);
        Ok(())
    }

    fn finish(&self) -> Result<()> {
        for (path, artifact) in &self.artifacts {
            match &artifact.role {
                ArtifactRole::Payload if !self.payloads.contains(path) => {
                    return Err(ReferenceError::new(
                        Code::Coverage,
                        "payload artifact has no logical source",
                    ));
                }
                ArtifactRole::SourceCopy { identity }
                    if !self.sources.contains(&reference_identity::key(identity)?) =>
                {
                    return Err(ReferenceError::new(
                        Code::Identity,
                        "source copy has no selected source",
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub(crate) fn validate(manifest: &Manifest, tool: Tool) -> Result<()> {
    let mut mappings = Mappings {
        tool,
        artifacts: artifacts(manifest)?,
        sources: BTreeSet::new(),
        locations: BTreeSet::new(),
        payloads: BTreeSet::new(),
    };
    for expected in &manifest.expected {
        mappings.source(expected)?;
    }
    mappings.finish()?;
    log_roles(manifest)
}

fn log_roles(manifest: &Manifest) -> Result<()> {
    for role in [
        ArtifactRole::ProducerStdout,
        ArtifactRole::ProducerStderr,
        ArtifactRole::ProducerErrorLog,
    ] {
        if manifest
            .artifacts
            .iter()
            .filter(|artifact| artifact.role == role)
            .count()
            != 1
        {
            return Err(ReferenceError::new(
                Code::Incomplete,
                "each captured/derived producer-log role is required once",
            ));
        }
    }
    Ok(())
}
