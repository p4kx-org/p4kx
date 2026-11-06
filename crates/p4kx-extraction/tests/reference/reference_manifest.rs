// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub(crate) const MANIFEST_NAME: &str = ".p4kx-reference.json";
pub(crate) const SCHEMA_VERSION: u32 = 1;
pub(crate) const MAX_MANIFEST_BYTES: u64 = 64 << 20;
pub(crate) const MAX_LOGICAL_SOURCES: usize = 1_000_000;
pub(crate) const MAX_ARTIFACTS: usize = 2_000_000;
pub(crate) const MAX_PATH_BYTES: usize = 4096;
pub(crate) const MAX_ARTIFACT_BYTES: u64 = 8 << 30;
pub(crate) const MAX_STRUCTURE_BYTES: u64 = 64 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Tool {
    Unp4k,
    Unforge,
    Starbreaker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Algorithm {
    Sha256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Digest {
    pub(crate) algorithm: Algorithm,
    #[schemars(regex(pattern = r"^[0-9a-f]{64}$"))]
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Fingerprint {
    pub(crate) bytes: u64,
    pub(crate) digest: Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum PatchScope {
    Unmodified,
    Local {
        #[serde(deserialize_with = "super::reference_limits::text")]
        description: String,
        digest: Digest,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Producer {
    pub(crate) tool: Tool,
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) version: String,
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) revision: String,
    pub(crate) binary: Fingerprint,
    pub(crate) patch_scope: PatchScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BuildIdentity {
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) channel: String,
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) branch: String,
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) version: String,
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) change_number: String,
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) build_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DcbIdentity {
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) entry: String,
    pub(crate) fingerprint: Fingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SourceDataset {
    pub(crate) archive: Fingerprint,
    pub(crate) build: BuildIdentity,
    pub(crate) inventory: Digest,
    pub(crate) dcb: Option<DcbIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SelectionMode {
    All,
    MainOnly,
    Filtered,
    Sampled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Selection {
    pub(crate) mode: SelectionMode,
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) declaration: String,
    #[serde(default, deserialize_with = "super::reference_limits::optional_text")]
    pub(crate) empty_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Identity {
    Entry {
        #[serde(deserialize_with = "super::reference_limits::text")]
        path: String,
    },
    Record {
        #[serde(deserialize_with = "super::reference_limits::text")]
        guid: String,
        #[serde(deserialize_with = "super::reference_limits::text")]
        dcb: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Location {
    #[schemars(length(min = 1, max = MAX_PATH_BYTES))]
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) path: String,
    #[serde(default, deserialize_with = "super::reference_limits::optional_text")]
    pub(crate) subresource: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Expected {
    pub(crate) identity: Identity,
    #[schemars(length(min = 1, max = MAX_ARTIFACTS))]
    #[serde(deserialize_with = "super::reference_limits::targets")]
    pub(crate) targets: Vec<Location>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum ArtifactRole {
    Payload,
    SourceCopy { identity: Identity },
    ProducerStdout,
    ProducerStderr,
    ProducerErrorLog,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Artifact {
    #[schemars(length(min = 1, max = MAX_PATH_BYTES))]
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) path: String,
    pub(crate) role: ArtifactRole,
    pub(crate) fingerprint: Option<Fingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Preparation {
    pub(crate) expected_digest: Digest,
    pub(crate) finalized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Completion {
    pub(crate) owner_asserts_success: bool,
    pub(crate) exit_code: Option<i32>,
    #[serde(deserialize_with = "super::reference_limits::details")]
    pub(crate) errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Manifest {
    #[schemars(range(min = SCHEMA_VERSION, max = SCHEMA_VERSION))]
    pub(crate) schema_version: u32,
    pub(crate) producer: Producer,
    pub(crate) source: SourceDataset,
    pub(crate) selection: Selection,
    #[schemars(length(max = MAX_LOGICAL_SOURCES))]
    #[serde(deserialize_with = "super::reference_limits::expected")]
    pub(crate) expected: Vec<Expected>,
    #[schemars(length(max = MAX_ARTIFACTS))]
    #[serde(deserialize_with = "super::reference_limits::artifacts")]
    pub(crate) artifacts: Vec<Artifact>,
    pub(crate) preparation: Preparation,
    pub(crate) completion: Completion,
}
