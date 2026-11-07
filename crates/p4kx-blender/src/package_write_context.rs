// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_guard::{ArchiveWriteProtection, CreatedOutputFile, OwnedDirectoryPermit, OwnedFileToken};

use crate::{PackageError, PackageResult};

/// Source identity and cancellation shared by every output of a package job.
#[derive(Clone, Copy)]
pub struct PackageWriteContext<'a> {
    scope: Scope<'a>,
}

#[derive(Clone, Copy)]
enum Scope<'a> {
    Archive {
        protection: &'a ArchiveWriteProtection,
        cancel: &'a AtomicBool,
    },
    Standalone {
        cancelled: &'a (dyn Fn() -> bool + Sync),
        observed: &'a AtomicBool,
    },
}

impl<'a> PackageWriteContext<'a> {
    /// Carries the actual source Archive's protection and the job's cancellation flag.
    #[must_use]
    pub const fn new(protection: &'a ArchiveWriteProtection, cancel: &'a AtomicBool) -> Self {
        Self {
            scope: Scope::Archive { protection, cancel },
        }
    }

    pub(crate) const fn standalone(
        cancelled: &'a (dyn Fn() -> bool + Sync),
        observed: &'a AtomicBool,
    ) -> Self {
        Self {
            scope: Scope::Standalone {
                cancelled,
                observed,
            },
        }
    }

    pub(crate) fn check(&self) -> PackageResult<()> {
        let cancelled = match self.scope {
            Scope::Archive { cancel, .. } => cancel.load(Ordering::Relaxed),
            Scope::Standalone {
                cancelled,
                observed,
            } => {
                if cancelled() {
                    observed.store(true, Ordering::Relaxed);
                }
                observed.load(Ordering::Relaxed)
            }
        };
        if cancelled {
            Err(PackageError::Cancelled)
        } else {
            Ok(())
        }
    }

    pub(crate) fn writable(&self, path: &Path) -> PackageResult<()> {
        self.check()?;
        self.cleanup_writable(path)
    }

    pub(crate) fn cleanup_writable(&self, path: &Path) -> PackageResult<()> {
        match self.scope {
            Scope::Archive { protection, cancel } => protection.ensure_writable(path, cancel)?,
            Scope::Standalone { .. } => p4kx_guard::ensure_writable(path)?,
        }
        Ok(())
    }

    pub(crate) const fn cancel_flag(&self) -> &AtomicBool {
        match self.scope {
            Scope::Archive { cancel, .. } => cancel,
            Scope::Standalone { observed, .. } => observed,
        }
    }

    pub(crate) const fn protection(&self) -> Option<&ArchiveWriteProtection> {
        match self.scope {
            Scope::Archive { protection, .. } => Some(protection),
            Scope::Standalone { .. } => None,
        }
    }

    pub(crate) fn create_file(&self, path: &Path) -> PackageResult<CreatedOutputFile> {
        self.check()?;
        Ok(p4kx_guard::create_output_file(
            path,
            self.protection(),
            self.cancel_flag(),
        )?)
    }

    pub(crate) fn create_directory(&self, path: &Path) -> PackageResult<OwnedDirectoryPermit> {
        self.check()?;
        Ok(p4kx_guard::create_output_directory(
            path,
            self.protection(),
            self.cancel_flag(),
        )?)
    }

    pub(crate) fn cleanup_file(&self, token: &OwnedFileToken, path: &Path) -> PackageResult<()> {
        token.remove(path, self.cancel_flag())?;
        Ok(())
    }
}
