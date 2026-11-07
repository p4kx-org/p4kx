// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};

use crate::portal_alias::{AliasUncertainty, UnknownAlias};
use crate::portal_mount::{self, PortalMount};
use crate::portal_path;

/// An authenticated declared mountpoint paired with its visible kernel mount identity.
#[derive(Debug)]
pub(crate) struct MountBinding {
    declared: Vec<u8>,
    mount: PortalMount,
}

impl MountBinding {
    /// Resolves the original existing mountpoint twice against fresh bounded inventories.
    #[cfg(target_os = "linux")]
    pub(crate) fn capture(
        raw: &[u8],
        cancel: &AtomicBool,
        mut inventory: impl FnMut() -> Result<Vec<u8>, UnknownAlias>,
    ) -> Result<Self, UnknownAlias> {
        use crate::portal_filesystem;

        check_cancel(cancel)?;
        let declared = portal_path::bytestring_path(raw)?;
        let path = portal_filesystem::path_buf(declared);
        let before = observe(&path, &inventory()?, cancel)?;
        let after = observe(&path, &inventory()?, cancel)?;
        if before != after {
            return Err(UnknownAlias::new(AliasUncertainty::UntrustedMount));
        }
        check_cancel(cancel)?;
        Ok(Self {
            declared: declared.to_vec(),
            mount: before,
        })
    }

    pub(crate) fn check_declared(&self, raw: &[u8]) -> Result<(), UnknownAlias> {
        if portal_path::bytestring_path(raw)? != self.declared.as_slice() {
            return Err(UnknownAlias::new(AliasUncertainty::UntrustedMount));
        }
        Ok(())
    }

    pub(crate) fn check_mount(&self, mount: &PortalMount) -> Result<(), UnknownAlias> {
        if &self.mount != mount {
            return Err(UnknownAlias::new(AliasUncertainty::UntrustedMount));
        }
        Ok(())
    }

    /// Constructs explicit synthetic namespace evidence without filesystem authority.
    #[cfg(test)]
    pub(crate) fn synthetic(
        declared: &[u8],
        mounts: &[u8],
        visible: &[u8],
    ) -> Result<Self, UnknownAlias> {
        Ok(Self {
            declared: declared.to_vec(),
            mount: mount_at(mounts, visible)?,
        })
    }
}

#[cfg(target_os = "linux")]
fn observe(path: &Path, mounts: &[u8], cancel: &AtomicBool) -> Result<PortalMount, UnknownAlias> {
    use std::fs;
    use std::os::unix::ffi::OsStrExt as _;

    check_cancel(cancel)?;
    let visible = fs::canonicalize(path)
        .map_err(|error| UnknownAlias::with_source(AliasUncertainty::Filesystem, error))?;
    let bytes = visible.as_os_str().as_bytes();
    if bytes.len() > portal_path::MAX_PATH_BYTES {
        return Err(UnknownAlias::new(AliasUncertainty::Limit));
    }
    let metadata = fs::symlink_metadata(&visible)
        .map_err(|error| UnknownAlias::with_source(AliasUncertainty::Filesystem, error))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(UnknownAlias::new(AliasUncertainty::UntrustedMount));
    }
    let mount = mount_at(mounts, bytes)?;
    check_cancel(cancel)?;
    Ok(mount)
}

fn mount_at(mounts: &[u8], visible: &[u8]) -> Result<PortalMount, UnknownAlias> {
    let mount = portal_mount::covering_mount(mounts, visible)?
        .ok_or_else(|| UnknownAlias::new(AliasUncertainty::UntrustedMount))?;
    if !mount.is_documents() || mount.target != visible {
        return Err(UnknownAlias::new(AliasUncertainty::UntrustedMount));
    }
    Ok(mount)
}

#[cfg(target_os = "linux")]
fn check_cancel(cancel: &AtomicBool) -> Result<(), UnknownAlias> {
    if cancel.load(Ordering::Acquire) {
        return Err(UnknownAlias::new(AliasUncertainty::Cancelled));
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
