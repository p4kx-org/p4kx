// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use same_file::Handle;

use crate::{
    ArchiveWriteProtection, OwnedOutputCleanupError, OwnedOutputError, WriteProtectionError,
};

/// Bound ownership of an output directory namespace without granting content writes.
#[derive(Debug)]
pub struct OwnedDirectoryPermit {
    path: PathBuf,
    host: PathBuf,
    handle: Handle,
    protection: Option<ArchiveWriteProtection>,
}

/// Exclusively creates a new output directory and retains its authenticated root identity.
///
/// # Errors
/// Returns protection, exclusive creation or root-identity failure.
pub fn create_output_directory(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: &AtomicBool,
) -> Result<OwnedDirectoryPermit, OwnedOutputError> {
    crate::write_target::host_target(path, cancel)?;
    let host = check_root(path, protection, cancel)?;
    fs::create_dir(path).map_err(|error| OwnedOutputError::io(path, error))?;
    let permit = OwnedDirectoryPermit::capture(path, host, protection).map_err(|error| {
        OwnedOutputError::RetainedCreation {
            path: path.to_path_buf(),
            source: error.into_io_error(),
        }
    })?;
    if let Err(primary) = permit.check_root(cancel, false) {
        let primary = OwnedOutputError::Protection(primary);
        return Err(match permit.cleanup(cancel) {
            Ok(()) => primary,
            Err(cleanup) => OwnedOutputError::Cleanup {
                primary: Box::new(primary),
                cleanup,
            },
        });
    }
    Ok(permit)
}

/// Binds a replacement namespace only after domain validation against its stable directory.
///
/// # Errors
/// Returns validation, source-containment, protection or changed-root failure.
pub fn validated_output_directory(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: &AtomicBool,
    validate: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<OwnedDirectoryPermit, OwnedOutputError> {
    crate::write_target::host_target(path, cancel)?;
    let host = check_root(path, protection, cancel)?;
    let permit = OwnedDirectoryPermit::capture(path, host, protection)?;
    validate(path).map_err(|error| OwnedOutputError::io(path, error))?;
    permit.check_root(cancel, false)?;
    Ok(permit)
}

impl OwnedDirectoryPermit {
    fn capture(
        path: &Path,
        host: PathBuf,
        protection: Option<&ArchiveWriteProtection>,
    ) -> Result<Self, OwnedOutputError> {
        let metadata =
            fs::symlink_metadata(path).map_err(|error| OwnedOutputError::io(path, error))?;
        if !metadata.is_dir() || crate::owned_output_cleanup::redirected(&metadata) {
            return Err(crate::owned_file_token::changed(path).into());
        }
        let handle = Handle::from_path(path).map_err(|error| OwnedOutputError::io(path, error))?;
        Ok(Self {
            path: std::path::absolute(path).map_err(|error| OwnedOutputError::io(path, error))?,
            host,
            handle,
            protection: protection.cloned(),
        })
    }

    /// Returns the current bound directory spelling.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Checks namespace removal or rename without authorising file-content writes.
    ///
    /// # Errors
    /// Returns refusal outside this root or when its root, source or host mapping changed.
    pub fn ensure_namespace_writable(
        &self,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), WriteProtectionError> {
        self.check_root(cancel, false)?;
        let host = crate::namespace_target::checked(path, self.protection.as_ref(), cancel)?;
        if !host.starts_with(&self.host) {
            return Err(crate::owned_file_token::changed(path));
        }
        Ok(())
    }

    /// Rebinds the same directory after its successful, separately authorised rename.
    ///
    /// # Errors
    /// Returns refusal unless the destination preserves this root's identity and source bounds.
    pub fn rebind_after_rename(
        &mut self,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), WriteProtectionError> {
        self.matches(path)?;
        let host = check_root(path, self.protection.as_ref(), cancel)?;
        self.path = std::path::absolute(path)
            .map_err(|error| crate::owned_file_token::identity_error(path, error))?;
        self.host = host;
        Ok(())
    }

    /// Removes this owned namespace without following links or modifying hardlink target data.
    ///
    /// # Errors
    /// Returns the first source, root-identity or filesystem failure and retains remaining nodes.
    pub fn cleanup(&self, cancel: &AtomicBool) -> Result<(), OwnedOutputCleanupError> {
        self.check_root(cancel, true)?;
        crate::owned_output_cleanup::cleanup_checked(&self.path, &|path| {
            if self.path.starts_with(path) && path != self.path {
                crate::namespace_target::checked(path, self.protection.as_ref(), cancel)?;
                return Ok(());
            }
            self.check_root(cancel, true)?;
            let host = crate::namespace_target::checked(path, self.protection.as_ref(), cancel)?;
            if !host.starts_with(&self.host) {
                return Err(crate::owned_file_token::changed(path));
            }
            Ok(())
        })
    }

    fn check_root(
        &self,
        cancel: &AtomicBool,
        allow_missing: bool,
    ) -> Result<(), WriteProtectionError> {
        let host = crate::namespace_target::checked(&self.path, self.protection.as_ref(), cancel)?;
        if host != self.host {
            return Err(crate::owned_file_token::changed(&self.path));
        }
        if allow_missing
            && matches!(fs::symlink_metadata(&self.path), Err(error) if error.kind() == io::ErrorKind::NotFound)
        {
            return Ok(());
        }
        self.matches(&self.path)
    }

    fn matches(&self, path: &Path) -> Result<(), WriteProtectionError> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| crate::owned_file_token::identity_error(path, error))?;
        if !metadata.is_dir() || crate::owned_output_cleanup::redirected(&metadata) {
            return Err(crate::owned_file_token::changed(path));
        }
        let handle = Handle::from_path(path)
            .map_err(|error| crate::owned_file_token::identity_error(path, error))?;
        if handle != self.handle {
            return Err(crate::owned_file_token::changed(path));
        }
        Ok(())
    }
}

fn check_root(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: &AtomicBool,
) -> Result<PathBuf, WriteProtectionError> {
    let host = crate::namespace_target::checked(path, protection, cancel)?;
    if let Some(protection) = protection {
        protection.check_directory_root(path, &host)?;
    }
    Ok(host)
}
