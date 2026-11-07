// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use crate::namespace_root::{NamespaceRoot, RootPresence};
use crate::{
    ArchiveWriteProtection, OwnedOutputCleanupError, OwnedOutputError, WriteProtectionError,
};

/// Bound ownership of an output directory namespace without granting content writes.
#[derive(Debug)]
pub struct OwnedDirectoryPermit {
    root: NamespaceRoot,
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
    crate::namespace_root::validate_path(path)?;
    let host = crate::write_target::host_target(path, cancel)?;
    crate::namespace_target::check_directory(path, &host, protection)?;
    fs::create_dir(path).map_err(|error| OwnedOutputError::io(path, error))?;
    let root = NamespaceRoot::capture(path, host, protection).map_err(|error| {
        OwnedOutputError::RetainedCreation {
            path: path.to_path_buf(),
            source: error.into_io_error(),
        }
    })?;
    let permit = OwnedDirectoryPermit { root };
    if let Err(primary) = permit.root.revalidate(cancel, RootPresence::Present) {
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
    let root = NamespaceRoot::bind_directory(path, protection, cancel)?;
    validate(path).map_err(|error| OwnedOutputError::io(path, error))?;
    root.revalidate(cancel, RootPresence::Present)?;
    Ok(OwnedDirectoryPermit { root })
}

impl OwnedDirectoryPermit {
    /// Returns the current bound directory spelling.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.root.path()
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
        self.root.check_entry(path, cancel)
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
        self.root.rebind_after_rename(path, cancel)
    }

    /// Removes this owned namespace without following links or modifying hardlink target data.
    ///
    /// # Errors
    /// Returns the first source, root-identity or filesystem failure and retains remaining nodes.
    pub fn cleanup(&self, cancel: &AtomicBool) -> Result<(), OwnedOutputCleanupError> {
        crate::owned_output_cleanup::cleanup_with(
            self.path(),
            &|| self.root.revalidate(cancel, RootPresence::AbsentAllowed),
            &|path| self.root.check_entry(path, cancel),
        )
    }
}
