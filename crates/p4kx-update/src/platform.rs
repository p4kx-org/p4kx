// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An operating system supported by a native P4KX release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperatingSystem {
    /// Windows with the MSVC ABI.
    Windows,
    /// Linux with the GNU ABI.
    Linux,
}

/// The architecture of the running application binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    /// AMD64, named `x86_64` by Rust.
    Amd64,
    /// ARM64, named aarch64 by Rust.
    Arm64,
}

/// A native application package kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageKind {
    /// A Windows Installer package.
    Msi,
    /// A portable Windows ZIP.
    PortableZip,
    /// A Linux tar.gz archive.
    TarGz,
    /// A Linux `AppImage`.
    AppImage,
    /// A Debian package.
    Deb,
    /// A Flatpak bundle.
    Flatpak,
}

impl PackageKind {
    /// Returns whether this package kind applies to an operating system.
    #[must_use]
    pub const fn supports(self, os: OperatingSystem) -> bool {
        matches!(
            (self, os),
            (Self::Msi | Self::PortableZip, OperatingSystem::Windows)
                | (
                    Self::TarGz | Self::AppImage | Self::Deb | Self::Flatpak,
                    OperatingSystem::Linux
                )
        )
    }
}

/// An operating system and the compiled binary architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    /// The binary's operating system.
    pub os: OperatingSystem,
    /// The binary's architecture, independent of host emulation.
    pub arch: Architecture,
}

impl Platform {
    /// Returns the target selected when this binary was compiled.
    #[must_use]
    pub const fn current() -> Option<Self> {
        let os = if cfg!(target_os = "windows") {
            OperatingSystem::Windows
        } else if cfg!(target_os = "linux") {
            OperatingSystem::Linux
        } else {
            return None;
        };
        let arch = if cfg!(target_arch = "x86_64") {
            Architecture::Amd64
        } else if cfg!(target_arch = "aarch64") {
            Architecture::Arm64
        } else {
            return None;
        };
        Some(Self { os, arch })
    }
}

/// A row in the authoritative native target contract.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetDefinition {
    /// The complete Rust target triple.
    pub target: String,
    /// The supported operating system.
    pub os: OperatingSystem,
    /// The native binary architecture.
    pub arch: Architecture,
    /// The native GitHub-hosted runner label.
    pub runner: String,
    /// The GitHub runner OS spelling.
    pub runner_os: String,
    /// The GitHub runner architecture spelling.
    pub runner_arch: String,
    /// The `WiX` architecture, when applicable.
    pub msi_arch: Option<String>,
    /// The Debian architecture, when applicable.
    pub deb_arch: Option<String>,
    /// The `AppImage`/Flatpak architecture, when applicable.
    pub unix_arch: Option<String>,
    /// The required package formats.
    pub packages: Vec<PackageKind>,
    /// The PE COFF machine value, when applicable.
    pub pe_machine: Option<u16>,
    /// The ELF `e_machine` value, when applicable.
    pub elf_machine: Option<u16>,
}

impl TargetDefinition {
    /// Returns the binary platform represented by this row.
    #[must_use]
    pub const fn platform(&self) -> Platform {
        Platform {
            os: self.os,
            arch: self.arch,
        }
    }
}

/// An invalid or inconsistent repository target definition.
#[derive(Debug, Error)]
pub enum PlatformError {
    /// The target contract is not valid JSON.
    #[error("invalid platform contract JSON")]
    Json(#[from] serde_json::Error),
    /// The requested asset version is not a semantic version.
    #[error("invalid asset version")]
    Version(#[from] semver::Error),
    /// The target contract does not describe the required native matrix.
    #[error("invalid platform contract: {0}")]
    Invalid(&'static str),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    version: u32,
    pub(crate) packages: BTreeMap<PackageKind, String>,
    pub(crate) targets: Vec<TargetDefinition>,
}

impl Manifest {
    pub(crate) fn asset_name(
        &self,
        version: &str,
        platform: Platform,
        package: PackageKind,
    ) -> Option<String> {
        let row = self.targets.iter().find(|row| row.platform() == platform)?;
        if !row.packages.contains(&package) {
            return None;
        }
        let suffix = self.packages.get(&package)?;
        Some(format!("P4KX-{version}-{}{suffix}", row.target))
    }
}

/// Returns the validated native target contract embedded in this build.
///
/// # Errors
///
/// Returns a platform error for malformed or inconsistent embedded configuration.
pub fn supported_targets() -> Result<Vec<TargetDefinition>, PlatformError> {
    Ok(manifest()?.targets)
}

/// Returns the canonical filename for a package supported by the binary platform.
///
/// # Errors
///
/// Returns a platform error for malformed or inconsistent embedded configuration.
pub fn asset_filename(
    version: &str,
    platform: Platform,
    package: PackageKind,
) -> Result<Option<String>, PlatformError> {
    semver::Version::parse(version)?;
    Ok(manifest()?.asset_name(version, platform, package))
}

pub(crate) fn manifest() -> Result<Manifest, PlatformError> {
    parse_manifest(include_str!("../../../.config/platforms.json"))
}

pub(crate) fn parse_manifest(text: &str) -> Result<Manifest, PlatformError> {
    let manifest: Manifest = serde_json::from_str(text)?;
    if manifest.version != 1 || manifest.targets.len() != 4 {
        return Err(PlatformError::Invalid(
            "expected contract version 1 and four native targets",
        ));
    }
    if manifest.packages.len() != 6 {
        return Err(PlatformError::Invalid("six package suffixes are required"));
    }
    let mut suffixes = BTreeSet::new();
    for suffix in manifest.packages.values() {
        if suffix.len() < 2
            || !suffix.starts_with(['.', '-'])
            || !suffix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b".-".contains(&byte))
            || !suffixes.insert(suffix.to_ascii_lowercase())
        {
            return Err(PlatformError::Invalid(
                "unsafe or colliding package filename suffix",
            ));
        }
    }
    for (index, row) in manifest.targets.iter().enumerate() {
        if manifest
            .targets
            .iter()
            .take(index)
            .any(|other| other.platform() == row.platform())
        {
            return Err(PlatformError::Invalid("duplicate OS/architecture row"));
        }
        validate_target(row)?;
        validate_architecture_names(row)?;
    }
    Ok(manifest)
}

fn validate_target(row: &TargetDefinition) -> Result<(), PlatformError> {
    let rust_arch = match row.arch {
        Architecture::Amd64 => "x86_64",
        Architecture::Arm64 => "aarch64",
    };
    let abi = match row.os {
        OperatingSystem::Windows => "pc-windows-msvc",
        OperatingSystem::Linux => "unknown-linux-gnu",
    };
    if row.target != format!("{rust_arch}-{abi}") {
        return Err(PlatformError::Invalid(
            "target does not match OS/architecture",
        ));
    }
    let package_count = match row.os {
        OperatingSystem::Windows => 2,
        OperatingSystem::Linux => 4,
    };
    if row.packages.len() != package_count
        || row.packages.iter().enumerate().any(|(index, package)| {
            !package.supports(row.os)
                || row
                    .packages
                    .iter()
                    .take(index)
                    .any(|other| other == package)
        })
    {
        return Err(PlatformError::Invalid(
            "required package formats are missing or duplicated",
        ));
    }
    if row.runner.is_empty() {
        return Err(PlatformError::Invalid("native runner label is empty"));
    }
    Ok(())
}

fn validate_architecture_names(row: &TargetDefinition) -> Result<(), PlatformError> {
    let (runner_arch, msi, deb, unix, pe, elf) = match row.arch {
        Architecture::Amd64 => ("X64", "x64", "amd64", "x86_64", 0x8664, 62),
        Architecture::Arm64 => ("ARM64", "arm64", "arm64", "aarch64", 0xaa64, 183),
    };
    let valid = row.runner_arch == runner_arch
        && match row.os {
            OperatingSystem::Windows => {
                row.runner_os == "Windows"
                    && row.msi_arch.as_deref() == Some(msi)
                    && row.deb_arch.is_none()
                    && row.unix_arch.is_none()
                    && row.pe_machine == Some(pe)
                    && row.elf_machine.is_none()
            }
            OperatingSystem::Linux => {
                row.runner_os == "Linux"
                    && row.msi_arch.is_none()
                    && row.deb_arch.as_deref() == Some(deb)
                    && row.unix_arch.as_deref() == Some(unix)
                    && row.pe_machine.is_none()
                    && row.elf_machine == Some(elf)
            }
        };
    if !valid {
        return Err(PlatformError::Invalid(
            "runner/package/machine architecture mismatch",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "platform_tests.rs"]
mod tests;
