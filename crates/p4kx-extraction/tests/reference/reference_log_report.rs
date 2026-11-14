// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::reference_manifest::{Fingerprint, PatchScope, Producer, Tool};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Profile {
    Unp4kB492,
    UnforgeB492,
    Starbreaker450,
    Starbreaker7d3,
    Starbreaker305,
}

impl Profile {
    pub(crate) fn of(producer: &Producer) -> Option<Self> {
        if producer.patch_scope != PatchScope::Unmodified {
            return None;
        }
        match (producer.tool, producer.revision.as_str()) {
            (Tool::Unp4k, super::unp4k_allow::PRODUCER_REVISION) => Some(Self::Unp4kB492),
            (Tool::Unforge, super::unp4k_allow::PRODUCER_REVISION) => Some(Self::UnforgeB492),
            (Tool::Starbreaker, "450fa284d4b1cbed6832675297dfb02579c0abd3") => {
                Some(Self::Starbreaker450)
            }
            (Tool::Starbreaker, "7d3f01e76307354bbc56a64a70eb5167b1189b64") => {
                Some(Self::Starbreaker7d3)
            }
            (Tool::Starbreaker, "305215bca75350013c221bdfbcb2f8180fe09dbe") => {
                Some(Self::Starbreaker305)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Finding {
    pub(crate) stream: Stream,
    pub(crate) line: u64,
    #[serde(deserialize_with = "super::reference_limits::text")]
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Capability {
    PinnedSourceProfile { profile: Profile },
    ReviewRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Report {
    #[schemars(range(min = 1, max = 1))]
    pub(crate) schema_version: u32,
    pub(crate) capability: Capability,
    pub(crate) stdout: Fingerprint,
    pub(crate) stderr: Fingerprint,
    #[serde(deserialize_with = "super::reference_limits::findings")]
    pub(crate) errors: Vec<Finding>,
    #[serde(deserialize_with = "super::reference_limits::findings")]
    pub(crate) review_required: Vec<Finding>,
}

impl Report {
    pub(crate) const fn complete(&self) -> bool {
        matches!(self.capability, Capability::PinnedSourceProfile { .. })
            && self.errors.is_empty()
            && self.review_required.is_empty()
    }
}
