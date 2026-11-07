// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use crate::{PackageKind, Platform, supported_targets};

const MAX_IDENTITY_BYTES: u64 = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Distribution {
    pub(crate) platform: Platform,
    pub(crate) package: PackageKind,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    target: String,
    package: PackageKind,
}

impl Distribution {
    pub(crate) fn current() -> Option<Self> {
        let executable = std::env::current_exe().ok()?;
        let identity = executable.parent()?.join("p4kx-distribution.json");
        Self::read(&identity, Platform::current()?)
    }

    pub(crate) fn read(path: &Path, platform: Platform) -> Option<Self> {
        let mut bytes = Vec::new();
        File::open(path)
            .ok()?
            .take(MAX_IDENTITY_BYTES + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        Self::parse(&bytes, platform)
    }

    pub(crate) fn parse(bytes: &[u8], platform: Platform) -> Option<Self> {
        if bytes.len() as u64 > MAX_IDENTITY_BYTES {
            return None;
        }
        let identity: Identity = serde_json::from_slice(bytes).ok()?;
        let row = supported_targets()
            .ok()?
            .into_iter()
            .find(|row| row.platform() == platform)?;
        (identity.target == row.target && identity.package.supports(platform.os)).then_some(Self {
            platform,
            package: identity.package,
        })
    }
}

#[cfg(test)]
#[path = "distribution_tests.rs"]
mod tests;
