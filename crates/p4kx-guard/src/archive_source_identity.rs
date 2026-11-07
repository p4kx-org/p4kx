// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use same_file::Handle;

use crate::{AliasUncertainty, UnknownAlias, WriteProtectionError};

#[derive(Debug, Clone)]
pub(crate) struct ArchiveSourceIdentity {
    archive: PathBuf,
    host: PathBuf,
    namespace: PathBuf,
    handle: Option<Arc<Handle>>,
    host_handle: Option<Arc<Handle>>,
    host_unavailable: Option<UnknownAlias>,
}

impl ArchiveSourceIdentity {
    pub(crate) fn new(archive: &Path, host: &Path) -> Result<Self, WriteProtectionError> {
        Self::capture(archive, host, SourceView::Ordinary)
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn documents(archive: &Path, host: &Path) -> Result<Self, WriteProtectionError> {
        Self::capture(archive, host, SourceView::Documents)
    }

    fn capture(
        archive: &Path,
        host: &Path,
        view: SourceView,
    ) -> Result<Self, WriteProtectionError> {
        let failure = |error| WriteProtectionError::SourceIdentity {
            archive: archive.to_path_buf(),
            cause: UnknownAlias::with_source(AliasUncertainty::Filesystem, error),
        };
        let absolute = std::path::absolute(host).map_err(failure)?;
        let (host, namespace) = match view {
            SourceView::Ordinary => (
                canonical_or_absent(&absolute).map_err(failure)?,
                original_namespace(archive).map_err(failure)?,
            ),
            #[cfg(any(target_os = "linux", test))]
            SourceView::Documents => (absolute.clone(), absolute),
        };
        let handle = match Handle::from_path(archive) {
            Ok(handle) => Some(Arc::new(handle)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(failure(error)),
        };
        let host_identity = match view {
            SourceView::Ordinary => handle
                .clone()
                .ok_or_else(|| UnknownAlias::new(AliasUncertainty::HostFileIdentity)),
            #[cfg(any(target_os = "linux", test))]
            SourceView::Documents => document_host_identity(handle.as_ref(), &host),
        };
        let (host_handle, host_unavailable) = match host_identity {
            Ok(handle) => (Some(handle), None),
            Err(error) => (None, Some(error)),
        };
        Ok(Self {
            archive: archive.to_path_buf(),
            host,
            namespace,
            handle,
            host_handle,
            host_unavailable,
        })
    }

    pub(crate) fn check(&self, target: &Path, host: &Path) -> Result<(), WriteProtectionError> {
        self.check_content(target, host, false)
    }

    pub(crate) fn check_owned(
        &self,
        target: &Path,
        host: &Path,
    ) -> Result<(), WriteProtectionError> {
        self.check_content(target, host, true)
    }

    pub(crate) fn host(&self) -> &Path {
        &self.host
    }

    pub(crate) fn check_namespace(
        &self,
        target: &Path,
        host: &Path,
    ) -> Result<(), WriteProtectionError> {
        if host == self.host || host == self.namespace {
            return Err(self.refusal(target));
        }
        Ok(())
    }

    pub(crate) fn check_root(
        &self,
        target: &Path,
        host: &Path,
    ) -> Result<(), WriteProtectionError> {
        if self.host.starts_with(host) || self.namespace.starts_with(host) {
            return Err(self.refusal(target));
        }
        Ok(())
    }

    fn check_content(
        &self,
        target: &Path,
        host: &Path,
        owned: bool,
    ) -> Result<(), WriteProtectionError> {
        let absolute =
            std::path::absolute(host).map_err(|error| WriteProtectionError::TargetIdentity {
                target: target.to_path_buf(),
                cause: UnknownAlias::with_source(AliasUncertainty::Filesystem, error),
            })?;
        if absolute == self.host {
            return Err(self.refusal(target));
        }
        let metadata = match fs::metadata(target) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(target_error(target, error)),
        };
        if metadata.is_dir() {
            return Ok(());
        }
        if !metadata.is_file() {
            return Err(unavailable_identity(target));
        }
        let visible = Handle::from_path(target).map_err(|error| target_error(target, error))?;
        if self
            .handle
            .as_ref()
            .is_some_and(|source| **source == visible)
        {
            return Err(self.refusal(target));
        }
        if owned {
            return Ok(());
        }
        let native = Handle::from_path(host).map_err(|error| target_error(target, error))?;
        let Some(source) = &self.host_handle else {
            return Err(WriteProtectionError::TargetIdentity {
                target: target.to_path_buf(),
                cause: self
                    .host_unavailable
                    .clone()
                    .unwrap_or_else(|| UnknownAlias::new(AliasUncertainty::HostFileIdentity)),
            });
        };
        if visible != native {
            return Err(unavailable_identity(target));
        }
        if **source == native {
            return Err(self.refusal(target));
        }
        Ok(())
    }

    fn refusal(&self, target: &Path) -> WriteProtectionError {
        WriteProtectionError::SourceOverwrite {
            archive: self.archive.clone(),
            target: target.to_path_buf(),
        }
    }
}

fn canonical_or_absent(path: &Path) -> io::Result<PathBuf> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(error),
    }
}

fn original_namespace(path: &Path) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let (Some(parent), Some(name)) = (absolute.parent(), absolute.file_name()) else {
        return Ok(absolute);
    };
    Ok(canonical_or_absent(parent)?.join(name))
}

#[cfg(any(target_os = "linux", test))]
fn document_host_identity(
    visible: Option<&Arc<Handle>>,
    host: &Path,
) -> Result<Arc<Handle>, UnknownAlias> {
    let visible = visible.ok_or_else(|| UnknownAlias::new(AliasUncertainty::HostFileIdentity))?;
    #[cfg(target_os = "linux")]
    if crate::portal_native::ordinary_target_for_cleanup(host)?.is_none() {
        return Err(UnknownAlias::new(AliasUncertainty::HostFileIdentity));
    }
    let native = Handle::from_path(host)
        .map_err(|error| UnknownAlias::with_source(AliasUncertainty::HostFileIdentity, error))?;
    if native != **visible {
        return Err(UnknownAlias::new(AliasUncertainty::HostFileIdentity));
    }
    Ok(Arc::new(native))
}

#[derive(Clone, Copy)]
enum SourceView {
    Ordinary,
    #[cfg(any(target_os = "linux", test))]
    Documents,
}

fn unavailable_identity(target: &Path) -> WriteProtectionError {
    WriteProtectionError::TargetIdentity {
        target: target.to_path_buf(),
        cause: UnknownAlias::new(AliasUncertainty::HostFileIdentity),
    }
}

fn target_error(target: &Path, error: io::Error) -> WriteProtectionError {
    WriteProtectionError::TargetIdentity {
        target: target.to_path_buf(),
        cause: UnknownAlias::with_source(AliasUncertainty::HostFileIdentity, error),
    }
}

#[cfg(test)]
mod tests;
