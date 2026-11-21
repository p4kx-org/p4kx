// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::{
    ArchiveWriteProtection, OwnedDirectoryPermit, OwnedFileToken, OwnedOutputError,
    create_output_file,
};

use super::lock::{LOCK_NAME, protection_error};

static RETIREMENT: AtomicU64 = AtomicU64::new(0);
static CLEANUP: AtomicBool = AtomicBool::new(false);

pub(super) fn rename_and_remove(
    lock: &Path,
    permit: &OwnedDirectoryPermit,
    mut ownership: OwnedFileToken,
    protection: Option<&ArchiveWriteProtection>,
) -> io::Result<()> {
    let (retired, placeholder) = reserve_sibling(lock, protection)?;
    let rename = || {
        permit
            .ensure_namespace_writable(lock, &CLEANUP)
            .map_err(protection_error)?;
        permit
            .ensure_namespace_writable(&retired, &CLEANUP)
            .map_err(protection_error)?;
        ownership
            .ensure_writable(lock, &CLEANUP)
            .map_err(protection_error)?;
        placeholder
            .ensure_writable(&retired, &CLEANUP)
            .map_err(protection_error)?;
        fs::rename(lock, &retired)
    };
    // Detach the old generation before deleting it so waiters never open a deleting name.
    if let Err(source) = rename() {
        return Err(match placeholder.remove(&retired, &CLEANUP) {
            Ok(()) => source,
            Err(cleanup) => OwnedOutputError::Cleanup {
                primary: Box::new(OwnedOutputError::Io {
                    path: lock.to_path_buf(),
                    source,
                }),
                cleanup,
            }
            .into_io_error(),
        });
    }
    ownership
        .rebind_after_rename(&retired, &CLEANUP)
        .map_err(protection_error)?;
    ownership
        .remove(&retired, &CLEANUP)
        .map_err(io::Error::other)
}

fn reserve_sibling(
    lock: &Path,
    protection: Option<&ArchiveWriteProtection>,
) -> io::Result<(PathBuf, OwnedFileToken)> {
    loop {
        let id = RETIREMENT.fetch_add(1, Ordering::Relaxed);
        let retired = lock.with_file_name(format!("{LOCK_NAME}.{}.{id}.part", std::process::id()));
        match create_output_file(&retired, protection, &CLEANUP) {
            Ok(created) => {
                let (file, token) = created.into_parts();
                drop(file);
                return Ok((retired, token));
            }
            Err(OwnedOutputError::Io { source, .. })
                if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into_io_error()),
        }
    }
}
