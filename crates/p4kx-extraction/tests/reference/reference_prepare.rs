// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use p4kx_extraction::Archive;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::reference_artifacts;
use super::reference_digest;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_layout;
use super::reference_load;
use super::reference_manifest::{
    Artifact, ArtifactRole, Completion, Expected, MANIFEST_NAME, MAX_MANIFEST_BYTES, Manifest,
    PatchScope, Preparation, Producer, SCHEMA_VERSION, Selection, Tool,
};
use super::reference_paths;
use super::reference_source;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProducerPlan {
    pub(crate) tool: Tool,
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) version: String,
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) revision: String,
    pub(crate) patch_scope: PatchScope,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Plan {
    #[schemars(range(min = SCHEMA_VERSION, max = SCHEMA_VERSION))]
    pub(crate) schema_version: u32,
    pub(crate) producer: ProducerPlan,
    pub(crate) selection: Selection,
    #[serde(deserialize_with = "super::reference_limits::expected")]
    pub(crate) expected: Vec<Expected>,
    #[serde(deserialize_with = "super::reference_limits::artifacts")]
    pub(crate) artifacts: Vec<Artifact>,
}

pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let file =
        File::open(path).map_err(|error| ReferenceError::new(Code::Manifest, error.to_string()))?;
    if file
        .metadata()
        .map_err(|error| ReferenceError::new(Code::Manifest, error.to_string()))?
        .len()
        > MAX_MANIFEST_BYTES
    {
        return Err(ReferenceError::new(
            Code::Bounds,
            "preparation input exceeds its bound",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ReferenceError::new(Code::Manifest, error.to_string()))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ReferenceError::new(
            Code::Bounds,
            "preparation input grew beyond its bound",
        ));
    }
    super::reference_limits::json_tokens(&bytes)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| ReferenceError::new(Code::Manifest, error.to_string()))
}

fn preconversion(root: &Path, manifest: &Manifest) -> Result<()> {
    if root.join(MANIFEST_NAME).exists() {
        return Err(ReferenceError::new(
            Code::Incomplete,
            "root already has a preparation manifest",
        ));
    }
    let files = reference_paths::inventory(root)?;
    for (key, path) in files {
        let artifact = manifest
            .artifacts
            .iter()
            .find(|artifact| reference_paths::key(&artifact.path).is_ok_and(|name| name == key))
            .ok_or_else(|| {
                ReferenceError::new(Code::Coverage, "unexpected file before conversion")
            })?;
        match artifact.role {
            ArtifactRole::Payload | ArtifactRole::ProducerErrorLog => {
                return Err(ReferenceError::new(
                    Code::Incomplete,
                    "payload already exists before selection was frozen",
                ));
            }
            ArtifactRole::ProducerStdout | ArtifactRole::ProducerStderr => {
                if File::open(path)
                    .and_then(|file| file.metadata())
                    .map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))?
                    .len()
                    != 0
                {
                    return Err(ReferenceError::new(
                        Code::Incomplete,
                        "producer log already has content before conversion",
                    ));
                }
            }
            ArtifactRole::SourceCopy { .. } => {}
        }
    }
    Ok(())
}

fn save(root: &Path, manifest: &Manifest) -> Result<()> {
    let path = root.join(MANIFEST_NAME);
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| ReferenceError::new(Code::Manifest, error.to_string()))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ReferenceError::new(
            Code::Bounds,
            "prepared manifest exceeds its bound",
        ));
    }
    p4kx_extraction::write_bytes_replacing(&path, &bytes)
        .map_err(|error| ReferenceError::new(Code::Artifact, error.to_string()))
}

pub(crate) fn begin(archive: &Archive, root: &Path, plan: Plan, binary: &Path) -> Result<()> {
    let root = reference_paths::root(root)?;
    if plan.schema_version != SCHEMA_VERSION
        || plan
            .artifacts
            .iter()
            .any(|artifact| artifact.fingerprint.is_some())
    {
        return Err(ReferenceError::new(
            Code::Manifest,
            "pre-conversion plan has wrong version or post-conversion fingerprints",
        ));
    }
    let source = reference_source::measure(archive, plan.producer.tool)?;
    let producer = Producer {
        tool: plan.producer.tool,
        version: plan.producer.version,
        revision: plan.producer.revision,
        binary: reference_digest::file(binary, u64::MAX)?,
        patch_scope: plan.producer.patch_scope,
    };
    let mut manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        producer,
        source: source.dataset.clone(),
        selection: plan.selection,
        expected: plan.expected,
        artifacts: plan.artifacts,
        preparation: Preparation {
            expected_digest: reference_digest::value(&())?,
            finalized: false,
        },
        completion: Completion {
            owner_asserts_success: false,
            exit_code: None,
            errors: Vec::new(),
        },
    };
    manifest.preparation.expected_digest = reference_layout::expected_digest(&manifest)?;
    reference_layout::shape(&manifest, manifest.producer.tool, false)?;
    source.validate(&manifest)?;
    preconversion(&root, &manifest)?;
    source.verify_source_copies(archive, &root, &manifest)?;
    save(&root, &manifest)
}

pub(crate) fn finalize(
    archive: &Archive,
    root: &Path,
    binary: &Path,
    completion: Completion,
) -> Result<()> {
    let root = reference_paths::root(root)?;
    let mut manifest = reference_load::read(&root)?.manifest;
    if manifest.preparation.finalized {
        return Err(ReferenceError::new(
            Code::Incomplete,
            "reference manifest is already finalized",
        ));
    }
    reference_layout::shape(&manifest, manifest.producer.tool, false)?;
    let source = reference_source::measure(archive, manifest.producer.tool)?;
    source.validate(&manifest)?;
    if reference_digest::file(binary, u64::MAX)? != manifest.producer.binary {
        return Err(ReferenceError::new(
            Code::Provenance,
            "producer binary changed since preparation",
        ));
    }
    super::reference_logs::capture(&root, &manifest)?;
    reference_artifacts::capture(&root, &mut manifest)?;
    source.verify_source_copies(archive, &root, &manifest)?;
    manifest.completion = completion;
    manifest.preparation.finalized = true;
    reference_layout::shape(&manifest, manifest.producer.tool, true)?;
    save(&root, &manifest)
}

fn variable(name: &str) -> Result<PathBuf> {
    std::env::var_os(name).map(PathBuf::from).ok_or_else(|| {
        ReferenceError::new(
            Code::Manifest,
            format!("{name} is required for preparation"),
        )
    })
}

#[test]
#[ignore = "manual reference preparation: set P4KX_REF_PREPARE_STAGE and private input paths"]
fn prepare_reference_manifest() -> super::TestResult {
    let Some(stage) = std::env::var_os("P4KX_REF_PREPARE_STAGE") else {
        super::report("skipped: manual reference preparation is not requested");
        return Ok(());
    };
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    p4kx_extraction::protect_archive_folder(archive.path());
    let root = variable("P4KX_REF_PREPARE_ROOT")?;
    let binary = variable("P4KX_REF_PRODUCER_BINARY")?;
    match stage.to_str() {
        Some("begin") => {
            let plan = read_json(&variable("P4KX_REF_PREPARE_PLAN")?)?;
            begin(&archive, &root, plan, &binary)?;
        }
        Some("finalize") => {
            let receipt = read_json(&variable("P4KX_REF_PREPARE_RECEIPT")?)?;
            finalize(&archive, &root, &binary, receipt)?;
        }
        _ => {
            return Err(ReferenceError::new(
                Code::Manifest,
                "preparation stage must be begin or finalize",
            )
            .into());
        }
    }
    super::report("manual source/artifact preparation completed; no foreign process was launched");
    Ok(())
}
