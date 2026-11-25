// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{self, Metadata};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use p4kx_guard::is_redirected_metadata;
use same_file::Handle;

use crate::{
    ArchiveWriteProtection, CancelToken, OwnedDirectoryPermit, OwnedFileToken, OwnedOutputError,
    WriteProtectionError, check_protected_output, create_output_file,
};

use super::lock_retirement;

#[path = "file_receipt_lock_collision.rs"]
mod collision;

use collision::lock_metadata;
pub(crate) use collision::origin_lock_collision;

pub(super) const LOCK_NAME: &str = ".p4kx-origin.lock";
pub(super) const LEASE_NAME: &str = ".p4kx-origin.lease";

pub(crate) fn is_origin_lock_name(name: &OsStr) -> bool {
    name.eq_ignore_ascii_case(LEASE_NAME) || name.eq_ignore_ascii_case(LOCK_NAME)
}

const LOCK_RETRY: Duration = Duration::from_millis(10);
static LOCAL_LEASES: LazyLock<Mutex<HashMap<PathBuf, (Contention, thread::ThreadId)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Contention {
    Refuse,
    Wait,
}

pub(super) struct ReceiptLock<'source> {
    file: Handle,
    permit: OwnedDirectoryPermit,
    ownership: Option<OwnedFileToken>,
    local: LocalLease,
    protection: Option<&'source ArchiveWriteProtection>,
}

impl<'source> ReceiptLock<'source> {
    pub(super) fn acquire(
        directory: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
        cancel: &CancelToken,
    ) -> io::Result<Self> {
        Self::acquire_with(directory, protection, cancel, Contention::Refuse)
    }

    pub(super) fn acquire_with(
        directory: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
        cancel: &CancelToken,
        contention: Contention,
    ) -> io::Result<Self> {
        let permit = crate::validated_output_directory(directory, None, cancel.flag(), |root| {
            let marker = root.join(LOCK_NAME);
            if let Some(metadata) = lock_metadata(&marker)? {
                if metadata.len() != 0 {
                    return Err(invalid("the legacy origin lock marker must be empty"));
                }
                check_protected_output(&marker, protection, Some(cancel))
                    .map_err(protection_error)?;
            }
            lock_metadata(&root.join(LEASE_NAME)).map(|_| ())
        })
        .map_err(OwnedOutputError::into_io_error)?;
        let local = LocalLease::reserve(permit.path().join(LEASE_NAME), cancel, contention)?;
        permit
            .ensure_namespace_writable(&local.path, cancel.flag())
            .map_err(protection_error)?;
        let created =
            create_output_file(&local.path, protection, cancel.flag()).map_err(|error| {
                let cause = error.into_io_error();
                if cause.kind() == io::ErrorKind::AlreadyExists {
                    io::Error::new(
                        io::ErrorKind::WouldBlock,
                        format!(
                            "the origin publication lease already exists at {}",
                            local.path.display()
                        ),
                    )
                } else {
                    cause
                }
            })?;
        let (file, ownership) = created.into_parts();
        let file = Handle::from_file(file).map_err(|error| retained_lock(&local.path, error))?;
        let result = Self {
            file,
            permit,
            ownership: Some(ownership),
            local,
            protection,
        };
        match result.check(cancel) {
            Ok(()) => Ok(result),
            Err(error) => result.finish_io(Err(error), false),
        }
    }

    pub(super) fn retire(mut self) -> io::Result<()> {
        self.retire_owned()
    }

    fn retire_owned(&mut self) -> io::Result<()> {
        if let Some(ownership) = self.ownership.take() {
            self.check(&CancelToken::new())?;
            lock_retirement::rename_and_remove(
                &self.local.path,
                &self.permit,
                ownership,
                self.protection,
            )?;
        }
        Ok(())
    }

    pub(super) fn finish_io<T>(self, result: io::Result<T>, committed: bool) -> io::Result<T> {
        use super::ordinary::LockFinishFailure;

        let Err(cleanup) = self.retire() else {
            return result;
        };
        Err(io::Error::other(match (result, committed) {
            (Ok(_), true) => LockFinishFailure::Committed { cleanup },
            (Ok(_), false) => LockFinishFailure::Unplaced { cleanup },
            (Err(primary), true) => LockFinishFailure::Receipt { primary, cleanup },
            (Err(primary), false) => LockFinishFailure::Uncommitted { primary, cleanup },
        }))
    }

    pub(super) fn check(&self, cancel: &CancelToken) -> io::Result<()> {
        self.permit
            .ensure_namespace_writable(&self.local.path, cancel.flag())
            .map_err(protection_error)?;
        validate_lock(&self.file, &self.local.path)
    }
}

impl Drop for ReceiptLock<'_> {
    fn drop(&mut self) {
        if let Err(error) = self.retire_owned() {
            let _ = writeln!(
                io::stderr(),
                "could not retire the abandoned origin publication lease: {error}"
            );
        }
    }
}

struct LocalLease {
    path: PathBuf,
}

impl LocalLease {
    fn reserve(path: PathBuf, cancel: &CancelToken, contention: Contention) -> io::Result<Self> {
        let thread_id = thread::current().id();
        loop {
            if cancel.is_cancelled() {
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            {
                let mut leases = local_leases();
                match leases.get(&path) {
                    None => {
                        leases.insert(path.clone(), (contention, thread_id));
                        drop(leases);
                        return Ok(Self { path });
                    }
                    Some(&(Contention::Wait, owner))
                        if contention == Contention::Wait && owner != thread_id => {}
                    Some(_) => return Err(io::Error::from(io::ErrorKind::WouldBlock)),
                }
            }
            thread::sleep(LOCK_RETRY);
        }
    }
}

impl Drop for LocalLease {
    fn drop(&mut self) {
        local_leases().remove(&self.path);
    }
}

fn local_leases() -> MutexGuard<'static, HashMap<PathBuf, (Contention, thread::ThreadId)>> {
    LOCAL_LEASES.lock().unwrap_or_else(PoisonError::into_inner)
}
fn retained_lock(path: &Path, source: io::Error) -> io::Error {
    io::Error::new(
        source.kind(),
        RetainedLock {
            path: path.to_path_buf(),
            source,
        },
    )
}

#[derive(Debug, thiserror::Error)]
#[error("temporary ordinary publication lock at {} was retained before commit", path.display())]
pub(super) struct RetainedLock {
    path: PathBuf,
    #[source]
    source: io::Error,
}

fn validate_lock(file: &Handle, path: &Path) -> io::Result<()> {
    if !same_lock(file, path)? {
        return Err(invalid("the origin publication lock changed identity"));
    }
    Ok(())
}

fn same_lock(file: &Handle, path: &Path) -> io::Result<bool> {
    let metadata = file.as_file().metadata()?;
    let Some(current) = lock_metadata(path)? else {
        return Ok(false);
    };
    if !metadata.is_file() || metadata.len() != 0 || current.len() != 0 {
        return Err(invalid(
            "the origin publication lock must be an empty regular file",
        ));
    }
    match Handle::from_path(path) {
        Ok(current) => Ok(current == *file),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn regular_file(path: &Path) -> io::Result<Option<Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !is_redirected_metadata(&metadata) => {
            Ok(Some(metadata))
        }
        Ok(_) => Err(invalid(
            "the origin target must be a regular, non-redirected file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn invalid(detail: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, detail)
}

pub(super) fn protection_error(error: WriteProtectionError) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, error)
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io;

    use same_file::Handle;

    use super::{LOCK_NAME, validate_lock};

    #[test]
    fn an_empty_regular_lock_must_still_identify_the_opened_file() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let lock = dir.path().join(LOCK_NAME);
        let opened = Handle::from_file(File::create_new(&lock)?)?;
        validate_lock(&opened, &lock)?;
        let previous = dir.path().join("previous.lock");
        fs::rename(&lock, &previous)?;
        File::create_new(&lock)?;
        assert_eq!(
            validate_lock(&opened, &lock)
                .expect_err("a different empty regular file cannot authenticate this lock")
                .kind(),
            io::ErrorKind::InvalidInput
        );
        validate_lock(&opened, &previous)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "file_receipt_lock_order_tests.rs"]
mod order_tests;
