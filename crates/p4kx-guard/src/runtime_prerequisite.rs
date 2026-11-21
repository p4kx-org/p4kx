// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::UnknownAlias;

/// The authenticated startup environment used for frontend presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimeEnvironment {
    /// No Flatpak sandbox marker exists.
    Native,
    /// A valid Flatpak marker and Documents API prerequisite were checked.
    Flatpak,
}

/// A missing prerequisite for operational Flatpak commands.
#[derive(Debug, thiserror::Error)]
#[error(
    "[p4kx.runtime.documents-api5] Flatpak requires Documents portal API 5 or later with authenticated host-path lookup: {cause}"
)]
pub struct RuntimePrerequisiteError {
    #[source]
    cause: UnknownAlias,
}

impl RuntimePrerequisiteError {
    /// Returns the prerequisite's typed evidence failure.
    #[must_use]
    pub const fn cause(&self) -> &UnknownAlias {
        &self.cause
    }
}

/// Checks operational prerequisites when running in an actual Flatpak sandbox.
///
/// # Errors
/// Returns uncertainty for invalid sandbox evidence or unavailable Documents API 5.
#[cfg(target_os = "linux")]
pub fn check_runtime_prerequisites() -> Result<(), RuntimePrerequisiteError> {
    check_runtime_environment().map(|_| ())
}

/// Checks operational prerequisites outside the Flatpak platform.
///
/// # Errors
/// This platform has no additional Documents-service prerequisite.
#[cfg(not(target_os = "linux"))]
pub const fn check_runtime_prerequisites() -> Result<(), RuntimePrerequisiteError> {
    Ok(())
}

/// Checks startup prerequisites and returns their presentation environment.
///
/// # Errors
/// Returns uncertainty for invalid sandbox evidence or unavailable Documents API 5.
#[cfg(target_os = "linux")]
pub fn check_runtime_environment() -> Result<RuntimeEnvironment, RuntimePrerequisiteError> {
    check_flatpak().map_err(|cause| RuntimePrerequisiteError { cause })
}

/// Returns the native startup environment on platforms without Flatpak.
///
/// # Errors
/// This platform has no additional Documents-service prerequisite.
#[cfg(not(target_os = "linux"))]
pub const fn check_runtime_environment() -> Result<RuntimeEnvironment, RuntimePrerequisiteError> {
    Ok(RuntimeEnvironment::Native)
}

#[cfg(target_os = "linux")]
fn check_flatpak() -> Result<RuntimeEnvironment, UnknownAlias> {
    use std::fs::File;
    use std::io::{self, Read as _};
    use std::sync::atomic::AtomicBool;

    use crate::AliasUncertainty;

    const MAX_FLATPAK_INFO_BYTES: u64 = 64 * 1024;
    let file = match File::open("/.flatpak-info") {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RuntimeEnvironment::Native);
        }
        Err(error) => {
            return Err(UnknownAlias::with_source(
                AliasUncertainty::Filesystem,
                error,
            ));
        }
    };
    let mut bytes = Vec::new();
    file.take(MAX_FLATPAK_INFO_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| UnknownAlias::with_source(AliasUncertainty::Filesystem, error))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_FLATPAK_INFO_BYTES {
        return Err(UnknownAlias::new(AliasUncertainty::Limit));
    }
    validate_flatpak_info(&bytes)?;
    crate::portal_dbus::DocumentsClient {
        cancel: &AtomicBool::new(false),
    }
    .probe()?;
    Ok(RuntimeEnvironment::Flatpak)
}

#[cfg(any(target_os = "linux", test))]
fn validate_flatpak_info(bytes: &[u8]) -> Result<(), UnknownAlias> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| UnknownAlias::new(crate::AliasUncertainty::MalformedReply))?;
    let mut application = false;
    let mut name = None;
    let mut instance = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            application = line == "[Application]";
            instance |= line == "[Instance]";
        } else if application
            && let Some(value) = line.strip_prefix("name=")
            && (name.replace(value).is_some() || value.is_empty())
        {
            return Err(UnknownAlias::new(crate::AliasUncertainty::MalformedReply));
        }
    }
    if name.is_none() || !instance {
        return Err(UnknownAlias::new(crate::AliasUncertainty::MalformedReply));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(all(test, not(target_os = "linux")))]
mod environment_tests {
    use super::{RuntimeEnvironment, check_runtime_environment, check_runtime_prerequisites};

    #[test]
    fn the_native_startup_fact_and_existing_unit_contract_agree() {
        assert_eq!(
            check_runtime_environment().expect("no platform prerequisite"),
            RuntimeEnvironment::Native
        );
        assert!(
            check_runtime_prerequisites().is_ok(),
            "existing API retained"
        );
    }
}
