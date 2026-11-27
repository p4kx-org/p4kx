// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::Metadata;
use std::io;
use std::path::{Path, PathBuf};

use super::{RetainedLock, regular_file};

#[derive(Debug, thiserror::Error)]
#[error("the origin publication lock {path} must be a regular, non-redirected file")]
struct OriginLockCollision {
    path: PathBuf,
}

pub(crate) fn origin_lock_collision(error: &io::Error) -> Option<&Path> {
    for cause in crate::owned_output_failure::io_error_chain(error) {
        if cause.is::<RetainedLock>() {
            return None;
        }
        if let Some(collision) = cause.downcast_ref::<OriginLockCollision>() {
            return Some(collision.path.as_path());
        }
    }
    None
}

pub(super) fn lock_metadata(path: &Path) -> io::Result<Option<Metadata>> {
    regular_file(path).map_err(|error| {
        if error.kind() == io::ErrorKind::InvalidInput && error.raw_os_error().is_none() {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                OriginLockCollision {
                    path: path.to_path_buf(),
                },
            )
        } else {
            error
        }
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;

    use crate::file_receipt::retained_lock_failure;

    use super::{lock_metadata, origin_lock_collision};

    #[test]
    fn an_owned_retained_lock_does_not_collapse_to_a_plain_collision() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let lock = directory.path().join(".p4kx-origin.lock");
        fs::create_dir(&lock)?;
        let collision = lock_metadata(&lock).expect_err("a foreign lock directory");
        assert_eq!(
            origin_lock_collision(&collision),
            Some(lock.as_path()),
            "a plain collision exposes its path"
        );

        let retained = super::super::retained_lock(&lock, collision);
        assert_eq!(
            origin_lock_collision(&retained),
            None,
            "the retained state takes precedence over its nested collision"
        );
        assert!(
            retained_lock_failure(&retained),
            "the original retained lock cause is still visible"
        );
        Ok(())
    }
}
