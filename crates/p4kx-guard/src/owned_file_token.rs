// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use same_file::Handle;

use crate::{
    AliasUncertainty, ArchiveWriteProtection, OwnedOutputCleanupError, OwnedOutputError,
    UnknownAlias, WriteProtectionError,
};

/// Descriptor-bound proof for one file exclusively created by its retained output job.
#[derive(Debug)]
pub struct OwnedFileToken {
    protection: Option<ArchiveWriteProtection>,
    host: PathBuf,
    handle: Handle,
}

impl OwnedFileToken {
    pub(crate) fn capture(
        path: &Path,
        host: PathBuf,
        file: File,
        protection: Option<&ArchiveWriteProtection>,
    ) -> Result<Self, OwnedOutputError> {
        let handle =
            Handle::from_file(file).map_err(|source| OwnedOutputError::RetainedCreation {
                path: path.to_path_buf(),
                source,
            })?;
        Ok(Self {
            protection: protection.cloned(),
            host,
            handle,
        })
    }

    pub(crate) fn writer(&self) -> std::io::Result<File> {
        self.handle.as_file().try_clone()
    }

    /// Checks content writes against the retained job and the exclusively created descriptor.
    ///
    /// # Errors
    /// Returns protection refusal when the job, object or authenticated spelling changed.
    pub fn ensure_writable(
        &self,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), WriteProtectionError> {
        let host = crate::write_target::host_target(path, cancel)?;
        self.matches(path)?;
        if host != self.host {
            return Err(changed(path));
        }
        self.protection.as_ref().map_or_else(
            || crate::registry::check_known(&host).map_err(Into::into),
            |protection| protection.ensure_owned_host(path, &host, self),
        )
    }

    /// Rebinds the same descriptor after a successful, separately authorised rename.
    ///
    /// # Errors
    /// Returns refusal if the new path does not identify this job's created file.
    pub fn rebind_after_rename(
        &mut self,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), WriteProtectionError> {
        let host = crate::namespace_target::checked(path, self.protection.as_ref(), cancel)?;
        self.matches(path)?;
        match &self.protection {
            Some(protection) => protection.ensure_owned_host(path, &host, self)?,
            None => crate::registry::check_known(&host)?,
        }
        self.host = host;
        Ok(())
    }

    /// Removes only the matching created filename without changing hardlink target data.
    ///
    /// # Errors
    /// Returns namespace, identity or unlink failure and keeps an unauthenticated object.
    pub fn remove(&self, path: &Path, cancel: &AtomicBool) -> Result<(), OwnedOutputCleanupError> {
        let host = crate::namespace_target::checked(path, self.protection.as_ref(), cancel)?;
        if host != self.host {
            return Err(changed(path).into());
        }
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(OwnedOutputCleanupError::Io {
                    path: path.to_path_buf(),
                    source,
                });
            }
            Ok(_) => {}
        }
        self.matches(path)?;
        fs::remove_file(path).map_err(|source| OwnedOutputCleanupError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    pub(crate) fn belongs_to(&self, protection: &ArchiveWriteProtection) -> bool {
        self.protection
            .as_ref()
            .is_some_and(|own| own.same_job(protection))
    }

    fn matches(&self, path: &Path) -> Result<(), WriteProtectionError> {
        let metadata = fs::symlink_metadata(path).map_err(|error| identity_error(path, error))?;
        if !metadata.is_file() || is_redirected_metadata(&metadata) {
            return Err(changed(path));
        }
        let current = Handle::from_path(path).map_err(|error| identity_error(path, error))?;
        if current != self.handle {
            return Err(changed(path));
        }
        Ok(())
    }
}

/// Returns whether unfollowed metadata identifies a symlink or Windows reparse point.
#[cfg(windows)]
#[must_use]
pub fn is_redirected_metadata(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    metadata.file_type().is_symlink() || content_reparse_point(metadata.file_attributes())
}

/// Returns whether unfollowed metadata identifies a symlink.
#[cfg(not(windows))]
#[must_use]
pub fn is_redirected_metadata(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(any(windows, test))]
const fn content_reparse_point(attributes: u32) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

pub(crate) fn changed(path: &Path) -> WriteProtectionError {
    WriteProtectionError::TargetIdentity {
        target: path.to_path_buf(),
        cause: UnknownAlias::new(AliasUncertainty::OwnedOutputIdentity),
    }
}

pub(crate) fn identity_error(path: &Path, error: std::io::Error) -> WriteProtectionError {
    WriteProtectionError::TargetIdentity {
        target: path.to_path_buf(),
        cause: UnknownAlias::with_source(AliasUncertainty::OwnedOutputIdentity, error),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn content_reparse_points_do_not_require_the_directory_attribute() {
        const DIRECTORY: u32 = 0x10;
        const REPARSE_POINT: u32 = 0x400;
        assert!(super::content_reparse_point(REPARSE_POINT));
        assert!(super::content_reparse_point(REPARSE_POINT | DIRECTORY));
    }

    #[test]
    fn ordinary_file_attributes_do_not_report_a_content_redirect() {
        for attributes in [0, 0x01, 0x10, 0x20, 0x80] {
            assert!(!super::content_reparse_point(attributes));
        }
    }
}
