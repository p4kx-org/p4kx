// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::{ArchiveWriteProtection, WriteProtectionError};

pub(crate) fn checked(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: &AtomicBool,
) -> Result<PathBuf, WriteProtectionError> {
    let host = host_entry(path, cancel)?;
    crate::registry::check_known_namespace(&host)?;
    if let Some(protection) = protection {
        protection.check_namespace_host(path, &host)?;
    }
    Ok(host)
}

pub(crate) fn host_entry(
    path: &Path,
    cancel: &AtomicBool,
) -> Result<PathBuf, WriteProtectionError> {
    let absolute = std::path::absolute(path)
        .map_err(|error| crate::owned_file_token::identity_error(path, error))?;
    let (Some(parent), Some(name)) = (absolute.parent(), absolute.file_name()) else {
        return crate::write_target::cleanup_host_target(&absolute, cancel);
    };
    let mut host = crate::write_target::cleanup_host_target(parent, cancel)?;
    host.push(name);
    Ok(host)
}
