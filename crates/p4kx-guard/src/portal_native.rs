// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Read as _};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::portal_alias::{AliasUncertainty, HostAlias, PortalAlias, UnknownAlias};
use crate::portal_dbus::DocumentsClient;
use crate::portal_filesystem::{self, GrantSnapshot};
use crate::portal_mount::{self, PortalMount};
use crate::portal_path;
use crate::portal_resolution::{self, DocumentsTransport, MappingOutcome};

/// Resolves Documents-portal identity without deciding whether writing is permitted.
///
/// Unknown identity requires a caller policy; a mapped file does not reveal its host siblings.
pub fn resolve_portal_alias(path: &Path, cancel: &AtomicBool) -> PortalAlias {
    match resolve(path, cancel) {
        Ok(Some(alias)) => PortalAlias::Resolved(alias),
        Ok(None) => PortalAlias::NotPortal,
        Err(error) => PortalAlias::Unknown(error),
    }
}

pub(crate) fn ordinary_target_for_cleanup(path: &Path) -> Result<Option<PathBuf>, UnknownAlias> {
    let expected = expected_mount()?;
    let absolute = portal_filesystem::absolute(path)?;
    let mounts = mount_inventory()?;
    if portal_target(&absolute, &mounts, &expected)?.is_some() {
        return Ok(None);
    }
    let resolved = portal_filesystem::existing_ancestor(&absolute)?;
    let before = candidate_mount(&resolved, &mounts, &expected)?;
    let fresh = mount_inventory()?;
    if portal_target(&absolute, &fresh, &expected)?.is_some()
        || portal_filesystem::existing_ancestor(&absolute)? != resolved
        || candidate_mount(&resolved, &fresh, &expected)? != before
        || portal_mount::covering_mount(&mounts, resolved.as_os_str().as_bytes())?
            != portal_mount::covering_mount(&fresh, resolved.as_os_str().as_bytes())?
    {
        return Err(UnknownAlias::new(AliasUncertainty::UntrustedMount));
    }
    Ok(Some(resolved))
}

fn resolve(path: &Path, cancel: &AtomicBool) -> Result<Option<HostAlias>, UnknownAlias> {
    check_cancel(cancel)?;
    let expected = expected_mount()?;
    let mut transport = DocumentsClient { cancel };
    resolve_observed(path, cancel, &expected, mount_inventory, &mut transport)
}

fn resolve_observed(
    path: &Path,
    cancel: &AtomicBool,
    expected: &Path,
    mut inventory: impl FnMut() -> Result<Vec<u8>, UnknownAlias>,
    transport: &mut impl DocumentsTransport,
) -> Result<Option<HostAlias>, UnknownAlias> {
    check_cancel(cancel)?;
    let mounts = inventory()?;
    let absolute = portal_filesystem::absolute(path)?;
    let Some((target, mount)) = portal_target(&absolute, &mounts, expected)? else {
        return Ok(None);
    };
    let document = portal_path::document_path(target.as_os_str().as_bytes(), &mount.target)?;
    let before = GrantSnapshot::capture(&mount.target, &document)?;
    let outcome = portal_resolution::resolve_with(
        target.as_os_str().as_bytes(),
        &mounts,
        expected.as_os_str().as_bytes(),
        before.grant,
        transport,
    )?;
    check_cancel(cancel)?;
    let fresh_mounts = inventory()?;
    let Some((fresh_target, fresh_mount)) = portal_target(&absolute, &fresh_mounts, expected)?
    else {
        return Err(UnknownAlias::new(AliasUncertainty::UntrustedMount));
    };
    let after = GrantSnapshot::capture(&mount.target, &document)?;
    if before != after || fresh_target != target || fresh_mount != mount {
        return Err(UnknownAlias::new(AliasUncertainty::UntrustedMount));
    }
    match outcome {
        MappingOutcome::NotPortal => Err(UnknownAlias::new(AliasUncertainty::UntrustedMount)),
        MappingOutcome::Resolved(mapping) => Ok(Some(HostAlias::new(
            portal_filesystem::path_buf(&mapping.host_path),
            mapping.document_id,
            mapping.grant,
        ))),
    }
}

fn portal_target(
    path: &Path,
    mounts: &[u8],
    expected: &Path,
) -> Result<Option<(PathBuf, PortalMount)>, UnknownAlias> {
    if let Some(mount) = candidate_mount(path, mounts, expected)? {
        return Ok(Some((path.to_path_buf(), mount)));
    }
    let resolved = portal_filesystem::existing_ancestor(path)?;
    Ok(candidate_mount(&resolved, mounts, expected)?.map(|mount| (resolved, mount)))
}

fn candidate_mount(
    path: &Path,
    mounts: &[u8],
    expected: &Path,
) -> Result<Option<PortalMount>, UnknownAlias> {
    let bytes = path.as_os_str().as_bytes();
    let mount = portal_mount::covering_mount(mounts, bytes)?
        .ok_or_else(|| UnknownAlias::new(AliasUncertainty::MountInventory))?;
    if mount.is_documents() {
        return Ok(Some(mount));
    }
    if mount.filesystem == b"fuse.portal"
        || bytes == expected.as_os_str().as_bytes()
        || portal_path::strip_mount(bytes, expected.as_os_str().as_bytes()).is_some()
    {
        return Err(UnknownAlias::new(AliasUncertainty::UntrustedMount));
    }
    Ok(None)
}

fn expected_mount() -> Result<PathBuf, UnknownAlias> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(
        || PathBuf::from(format!("/run/user/{}", rustix::process::geteuid().as_raw())),
        PathBuf::from,
    );
    if !runtime.is_absolute() {
        return Err(UnknownAlias::new(AliasUncertainty::UntrustedMount));
    }
    Ok(runtime.join("doc"))
}

pub(crate) fn mount_inventory() -> Result<Vec<u8>, UnknownAlias> {
    let file = File::open("/proc/self/mountinfo")
        .map_err(|error| UnknownAlias::with_source(AliasUncertainty::MountInventory, error))?;
    read_mounts(file)
}

fn read_mounts(reader: impl io::Read) -> Result<Vec<u8>, UnknownAlias> {
    let limit = u64::try_from(portal_mount::MAX_MOUNT_BYTES)
        .map_err(|error| UnknownAlias::with_source(AliasUncertainty::Limit, error))?;
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| UnknownAlias::with_source(AliasUncertainty::MountInventory, error))?;
    if bytes.len() > portal_mount::MAX_MOUNT_BYTES {
        return Err(UnknownAlias::new(AliasUncertainty::Limit));
    }
    Ok(bytes)
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), UnknownAlias> {
    if cancel.load(Ordering::Acquire) {
        return Err(UnknownAlias::new(AliasUncertainty::Cancelled));
    }
    Ok(())
}

#[cfg(test)]
mod route_tests;
#[cfg(test)]
mod tests;
