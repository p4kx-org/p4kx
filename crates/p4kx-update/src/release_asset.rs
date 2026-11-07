// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::{Architecture, OperatingSystem, PackageKind, Platform, PlatformError};

/// A compatible package link advertised by a Release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    /// The exact asset filename.
    pub name: String,
    /// The GitHub URL opened when the user follows the update hint.
    pub url: String,
    /// The selected native Rust target.
    pub target: String,
    /// The selected package format.
    pub package: PackageKind,
}

pub(crate) fn select_asset(
    version: &str,
    platform: Platform,
    package: PackageKind,
    assets: &Value,
) -> Result<Option<ReleaseAsset>, PlatformError> {
    let contract = crate::platform::manifest()?;
    let Some(row) = contract
        .targets
        .iter()
        .find(|row| row.platform() == platform)
    else {
        return Ok(None);
    };
    let Some(expected) = contract.asset_name(version, platform, package) else {
        return Ok(None);
    };
    let Some(assets) = assets.as_array() else {
        return Ok(None);
    };
    let name = if assets
        .iter()
        .any(|asset| asset.get("name").and_then(Value::as_str) == Some(&expected))
    {
        expected
    } else if platform
        == (Platform {
            os: OperatingSystem::Windows,
            arch: Architecture::Amd64,
        })
    {
        let Some(suffix) = contract.packages.get(&package) else {
            return Ok(None);
        };
        format!("P4KX-{version}-x86_64{suffix}")
    } else {
        return Ok(None);
    };
    let mut matching = assets
        .iter()
        .filter(|asset| asset.get("name").and_then(Value::as_str) == Some(&name));
    let Some(candidate) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Ok(None);
    }
    let Some(url) = candidate
        .get("browser_download_url")
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };
    let expected_url =
        format!("https://github.com/p4kx-org/p4kx/releases/download/v{version}/{name}");
    if url != expected_url {
        return Ok(None);
    }
    Ok(Some(ReleaseAsset {
        name,
        url: url.to_owned(),
        target: row.target.clone(),
        package,
    }))
}

#[cfg(test)]
#[path = "release_asset_tests.rs"]
mod tests;
