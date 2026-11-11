// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File, Metadata, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use crate::{ArchiveWriteProtection, CancelToken, OwnedDirectoryPermit, check_protected_output};

pub(super) const LOCK_NAME: &str = ".p4kx-origin.lock";

pub(super) struct ReceiptLock {
    file: File,
    path: PathBuf,
    permit: OwnedDirectoryPermit,
}

impl ReceiptLock {
    pub(super) fn acquire(
        directory: &Path,
        protection: Option<&ArchiveWriteProtection>,
        cancel: &CancelToken,
    ) -> io::Result<Self> {
        let permit = crate::validated_output_directory(directory, None, cancel.flag(), |root| {
            regular_file(&root.join(LOCK_NAME)).map(|_| ())
        })
        .map_err(crate::OwnedOutputError::into_io_error)?;
        let path = permit.path().join(LOCK_NAME);
        permit
            .ensure_namespace_writable(&path, cancel.flag())
            .map_err(protection_error)?;
        check_protected_output(&path, protection, Some(cancel)).map_err(protection_error)?;
        let file = open_lock(&path)?;
        validate_lock(&file, &path)?;
        file.try_lock().map_err(io::Error::from)?;
        let lock = Self { file, path, permit };
        lock.check(cancel)?;
        Ok(lock)
    }

    pub(super) fn check(&self, cancel: &CancelToken) -> io::Result<()> {
        self.permit
            .ensure_namespace_writable(&self.path, cancel.flag())
            .map_err(protection_error)?;
        validate_lock(&self.file, &self.path)
    }
}

fn open_lock(path: &Path) -> io::Result<File> {
    match regular_file(path)? {
        Some(_) => OpenOptions::new().read(true).write(true).open(path),
        None => match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(file) => Ok(file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                regular_file(path)?;
                OpenOptions::new().read(true).write(true).open(path)
            }
            Err(error) => Err(error),
        },
    }
}

fn validate_lock(file: &File, path: &Path) -> io::Result<()> {
    let metadata = file.metadata()?;
    let Some(current) = regular_file(path)? else {
        return Err(invalid("the origin publication lock no longer exists"));
    };
    if !metadata.is_file() || metadata.len() != 0 || current.len() != 0 {
        return Err(invalid(
            "the origin publication lock must be an empty regular file",
        ));
    }
    Ok(())
}

pub(super) fn regular_file(path: &Path) -> io::Result<Option<Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !redirected(&metadata) => Ok(Some(metadata)),
        Ok(_) => Err(invalid(
            "the origin target must be a regular, non-redirected file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn redirected(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        const REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub(super) fn invalid(detail: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, detail)
}

pub(super) fn protection_error(error: crate::WriteProtectionError) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, error)
}
