// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;

use same_file::Handle;

use crate::{ArchiveWriteProtection, OwnedOutputError, WriteProtectionError};

#[derive(Debug)]
pub(crate) struct NamespaceRoot {
    path: PathBuf,
    host: PathBuf,
    handle: Handle,
    protection: Option<ArchiveWriteProtection>,
}

#[derive(Clone, Copy)]
pub(crate) enum RootPresence {
    Present,
    AbsentAllowed,
}

impl NamespaceRoot {
    pub(crate) fn bind_directory(
        path: &Path,
        protection: Option<&ArchiveWriteProtection>,
        cancel: &AtomicBool,
    ) -> Result<Self, OwnedOutputError> {
        Self::bind_with(path, protection, cancel, &crate::write_target::host_target)
    }

    fn bind_with(
        path: &Path,
        protection: Option<&ArchiveWriteProtection>,
        cancel: &AtomicBool,
        lookup: &impl Fn(&Path, &AtomicBool) -> Result<PathBuf, WriteProtectionError>,
    ) -> Result<Self, OwnedOutputError> {
        validate_path(path)?;
        let host = lookup(path, cancel)?;
        crate::namespace_target::check_directory(path, &host, protection)?;
        Self::capture(path, host, protection)
    }

    pub(crate) fn capture(
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
            path: absolute_entry(path)?,
            host,
            handle,
            protection: protection.cloned(),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn revalidate(
        &self,
        cancel: &AtomicBool,
        presence: RootPresence,
    ) -> Result<(), WriteProtectionError> {
        self.revalidate_with(cancel, presence, &crate::write_target::cleanup_host_target)
    }

    fn revalidate_with(
        &self,
        cancel: &AtomicBool,
        presence: RootPresence,
        lookup: &impl Fn(&Path, &AtomicBool) -> Result<PathBuf, WriteProtectionError>,
    ) -> Result<(), WriteProtectionError> {
        let host = lookup(&self.path, cancel)?;
        crate::namespace_target::check_directory(&self.path, &host, self.protection.as_ref())?;
        if host != self.host {
            return Err(crate::owned_file_token::changed(&self.path));
        }
        if matches!(presence, RootPresence::AbsentAllowed)
            && matches!(fs::symlink_metadata(&self.path), Err(error) if error.kind() == io::ErrorKind::NotFound)
        {
            return Ok(());
        }
        self.matches(&self.path)
    }

    pub(crate) fn check_entry(
        &self,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), WriteProtectionError> {
        self.check_entry_with(path, cancel, &crate::write_target::cleanup_host_target)
    }

    fn check_entry_with(
        &self,
        path: &Path,
        cancel: &AtomicBool,
        lookup: &impl Fn(&Path, &AtomicBool) -> Result<PathBuf, WriteProtectionError>,
    ) -> Result<(), WriteProtectionError> {
        let absolute = absolute_entry(path)?;
        if !absolute.starts_with(&self.path) {
            return Err(crate::owned_file_token::changed(path));
        }
        self.revalidate_with(cancel, RootPresence::Present, lookup)?;
        if absolute == self.path {
            return Ok(());
        }
        let host = crate::namespace_target::host_entry_with(&absolute, cancel, lookup)?;
        if !host.starts_with(&self.host) {
            return Err(crate::owned_file_token::changed(path));
        }
        crate::namespace_target::check_host(path, &host, self.protection.as_ref())?;
        self.revalidate_with(cancel, RootPresence::Present, lookup)
    }

    pub(crate) fn rebind_after_rename(
        &mut self,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<(), WriteProtectionError> {
        self.matches(path)?;
        let host = crate::write_target::cleanup_host_target(path, cancel)?;
        crate::namespace_target::check_directory(path, &host, self.protection.as_ref())?;
        self.matches(path)?;
        self.path = absolute_entry(path)?;
        self.host = host;
        Ok(())
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

fn absolute_entry(path: &Path) -> Result<PathBuf, WriteProtectionError> {
    validate_path(path)?;
    std::path::absolute(path).map_err(|error| crate::owned_file_token::identity_error(path, error))
}

pub(crate) fn validate_path(path: &Path) -> Result<(), WriteProtectionError> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(crate::owned_file_token::changed(path));
    }
    Ok(())
}

#[cfg(test)]
#[path = "namespace_root_tests.rs"]
mod tests;
