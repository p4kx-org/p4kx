// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::{Deserialize, Deserializer};

use crate::{BuildProvenance, Digest, DirectoryIssue, RootSource};

/// Wire presence stays separate from original in-process Build authority.
#[derive(Deserialize)]
struct RootWire {
    path: String,
    digest: Digest,
    size: u64,
    #[serde(default)]
    build: BuildSlot,
    installation_flags: Option<u8>,
    directory_issue_count: usize,
    directory_issues: Vec<DirectoryIssue>,
    directory_issue_samples_complete: bool,
    enumeration_complete: bool,
}

#[derive(Default)]
struct BuildSlot {
    value: Option<BuildProvenance>,
    present: bool,
}

impl<'de> Deserialize<'de> for BuildSlot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            value: Option::deserialize(deserializer)?,
            present: true,
        })
    }
}

impl<'de> Deserialize<'de> for RootSource {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = RootWire::deserialize(deserializer)?;
        Ok(Self {
            path: wire.path,
            digest: wire.digest,
            size: wire.size,
            build: wire.build.value,
            build_present: wire.build.present,
            build_origin: None,
            installation_flags: wire.installation_flags,
            directory_issue_count: wire.directory_issue_count,
            directory_issues: wire.directory_issues,
            directory_issue_samples_complete: wire.directory_issue_samples_complete,
            enumeration_complete: wire.enumeration_complete,
        })
    }
}
