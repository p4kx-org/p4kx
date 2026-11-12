// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::fs::{self, Metadata, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use p4kx_guard::AliasUncertainty;
use same_file::Handle;

use crate::{
    ArchiveWriteProtection, CancelToken, OwnedDirectoryPermit, OwnedFileToken, OwnedOutputError,
    WriteProtectionError, check_protected_output, create_output_file,
};

use super::lock_retirement;

pub(super) const LOCK_NAME: &str = ".p4kx-origin.lock";

const LOCK_RETRY: Duration = Duration::from_millis(10);
static RECEIPT_PUBLISHERS: LazyLock<Mutex<HashSet<PathBuf>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Contention {
    Refuse,
    Wait,
}

pub(super) struct ReceiptLock {
    file: Handle,
    path: PathBuf,
    permit: OwnedDirectoryPermit,
    publishes_receipt: bool,
    ephemeral: Option<OwnedFileToken>,
}

impl ReceiptLock {
    pub(super) fn acquire(
        directory: &Path,
        protection: Option<&ArchiveWriteProtection>,
        cancel: &CancelToken,
    ) -> io::Result<Self> {
        Self::acquire_with(directory, protection, cancel, Contention::Refuse)
    }

    pub(super) fn acquire_with(
        directory: &Path,
        protection: Option<&ArchiveWriteProtection>,
        cancel: &CancelToken,
        contention: Contention,
    ) -> io::Result<Self> {
        let permit = crate::validated_output_directory(directory, None, cancel.flag(), |root| {
            regular_file(&root.join(LOCK_NAME)).map(|_| ())
        })
        .map_err(OwnedOutputError::into_io_error)?;
        let path = permit.path().join(LOCK_NAME);
        let candidate = lock_file(&path, &permit, protection, cancel, contention)?;
        let publishes_receipt = contention == Contention::Refuse;
        if publishes_receipt {
            receipt_publishers().insert(path.clone());
        }
        Ok(Self {
            file: candidate.file,
            path,
            permit,
            publishes_receipt,
            ephemeral: candidate.ephemeral,
        })
    }

    pub(super) fn retire(mut self, protection: Option<&ArchiveWriteProtection>) -> io::Result<()> {
        if let Some(ownership) = self.ephemeral.take() {
            self.check(&CancelToken::new())?;
            lock_retirement::rename_and_remove(&self.path, &self.permit, ownership, protection)?;
        }
        Ok(())
    }

    pub(super) fn check(&self, cancel: &CancelToken) -> io::Result<()> {
        self.permit
            .ensure_namespace_writable(&self.path, cancel.flag())
            .map_err(protection_error)?;
        validate_lock(&self.file, &self.path)
    }
}

impl Drop for ReceiptLock {
    fn drop(&mut self) {
        if self.publishes_receipt {
            receipt_publishers().remove(&self.path);
        }
    }
}

fn receipt_publishers() -> MutexGuard<'static, HashSet<PathBuf>> {
    RECEIPT_PUBLISHERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

struct LockCandidate {
    file: Handle,
    ephemeral: Option<OwnedFileToken>,
}

fn lock_file(
    path: &Path,
    permit: &OwnedDirectoryPermit,
    protection: Option<&ArchiveWriteProtection>,
    cancel: &CancelToken,
    contention: Contention,
) -> io::Result<LockCandidate> {
    let mut pending: Option<LockCandidate> = None;
    loop {
        if cancel.is_cancelled() {
            let cause = io::Error::from(io::ErrorKind::Interrupted);
            return Err(if pending.is_some() {
                retained_lock(path, cause)
            } else {
                cause
            });
        }
        let admitted = permit
            .ensure_namespace_writable(path, cancel.flag())
            .map_err(protection_error);
        if let Err(cause) = admitted {
            return Err(if pending.is_some() {
                retained_lock(path, cause)
            } else {
                cause
            });
        }
        let candidate = if let Some(candidate) = pending.take() {
            candidate
        } else if let Some(candidate) = open_candidate(path, protection, cancel, contention)? {
            candidate
        } else {
            continue;
        };
        if !candidate.admit(path, protection, cancel)? {
            continue;
        }
        match candidate.file.as_file().try_lock().map_err(io::Error::from) {
            Ok(()) if candidate.still_named(path)? => return Ok(candidate),
            Ok(()) => {}
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock && contention == Contention::Wait =>
            {
                if candidate.ephemeral.is_some() {
                    pending = Some(candidate);
                } else {
                    drop(candidate);
                    if receipt_publishers().contains(path) {
                        return Err(error);
                    }
                }
                thread::sleep(LOCK_RETRY);
            }
            Err(error) => {
                return Err(if candidate.ephemeral.is_some() {
                    retained_lock(path, error)
                } else {
                    error
                });
            }
        }
    }
}

impl LockCandidate {
    fn admit(
        &self,
        path: &Path,
        protection: Option<&ArchiveWriteProtection>,
        cancel: &CancelToken,
    ) -> io::Result<bool> {
        if !self.still_named(path)? {
            return Ok(false);
        }
        let Err(error) = check_protected_output(path, protection, Some(cancel)) else {
            return Ok(true);
        };
        if lock_target_disappeared(path, &error) && !self.still_named(path)? {
            return Ok(false);
        }
        let error = protection_error(error);
        Err(if self.ephemeral.is_some() {
            retained_lock(path, error)
        } else {
            error
        })
    }
    fn still_named(&self, path: &Path) -> io::Result<bool> {
        match same_lock(&self.file, path) {
            Ok(false) if self.ephemeral.is_some() => Err(invalid(
                "a newly created ordinary origin lock changed identity",
            )),
            Err(cause) if self.ephemeral.is_some() => Err(retained_lock(path, cause)),
            result => result,
        }
    }
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
struct RetainedLock {
    path: PathBuf,
    #[source]
    source: io::Error,
}

fn open_candidate(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: &CancelToken,
    contention: Contention,
) -> io::Result<Option<LockCandidate>> {
    let opened = if regular_file(path)?.is_some() {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map(|file| (file, None))
    } else if contention == Contention::Wait {
        match create_output_file(path, protection, cancel.flag()) {
            Ok(created) => {
                let (file, ownership) = created.into_parts();
                Ok((file, Some(ownership)))
            }
            Err(OwnedOutputError::Protection(error)) if lock_target_disappeared(path, &error) => {
                return Ok(None);
            }
            Err(error) => Err(error.into_io_error()),
        }
    } else {
        if let Err(error) = check_protected_output(path, protection, Some(cancel)) {
            if lock_target_disappeared(path, &error) {
                return Ok(None);
            }
            return Err(protection_error(error));
        }
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .map(|file| (file, None))
    };
    match opened {
        Ok((file, ephemeral)) => {
            let file = Handle::from_file(file).map_err(|error| {
                if ephemeral.is_some() {
                    retained_lock(path, error)
                } else {
                    error
                }
            })?;
            Ok(Some(LockCandidate { file, ephemeral }))
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::AlreadyExists | io::ErrorKind::NotFound
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn lock_target_disappeared(path: &Path, error: &WriteProtectionError) -> bool {
    use std::error::Error as _;

    matches!(
        error,
        WriteProtectionError::TargetIdentity { target, cause }
            if target == path
                && cause.reason() == AliasUncertainty::HostFileIdentity
                && cause
                    .source()
                    .and_then(|error| error.downcast_ref::<io::Error>())
                    .is_some_and(|error| error.kind() == io::ErrorKind::NotFound)
    )
}

fn validate_lock(file: &Handle, path: &Path) -> io::Result<()> {
    if !same_lock(file, path)? {
        return Err(invalid("the origin publication lock changed identity"));
    }
    Ok(())
}

fn same_lock(file: &Handle, path: &Path) -> io::Result<bool> {
    let metadata = file.as_file().metadata()?;
    let Some(current) = regular_file(path)? else {
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
        Ok(metadata) if metadata.is_file() && !redirected(&metadata) => Ok(Some(metadata)),
        Ok(_) => Err(invalid(
            "the origin target must be a regular, non-redirected file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn redirected(metadata: &Metadata) -> bool {
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
