// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::PackageError;

const ATTEMPTS: u32 = 6;
const FIRST_DELAY: Duration = Duration::from_millis(20);
#[cfg(windows)]
const ERROR_ACCESS_DENIED: i32 = 5;
#[cfg(windows)]
const ERROR_SHARING_VIOLATION: i32 = 32;

/// Returns whether `error` may be a handle another program holds briefly: access denied or a
/// sharing violation.
#[cfg(windows)]
pub(crate) fn is_busy(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::PermissionDenied
        || matches!(
            error.raw_os_error(),
            Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION)
        )
}

/// Returns whether `error` may be a handle another program holds briefly; never off Windows,
/// where an open handle does not block a rename.
#[cfg(not(windows))]
pub(crate) const fn is_busy(_error: &io::Error) -> bool {
    false
}

/// Checks destination absence before each bounded rename attempt.
pub(crate) fn rename_to_absent(from: &Path, to: &Path) -> io::Result<()> {
    retry(ATTEMPTS, FIRST_DELAY, || {
        match std::fs::symlink_metadata(to) {
            Ok(_) => return Err(io::Error::from(io::ErrorKind::AlreadyExists)),
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(source),
        }
        std::fs::rename(from, to)
    })
}

pub(crate) fn moving_error(path: &Path, source: io::Error) -> PackageError {
    if is_busy(&source) {
        PackageError::InUse {
            path: path.to_path_buf(),
            source,
        }
    } else {
        PackageError::write(path, source)
    }
}

fn retry(
    attempts: u32,
    first_delay: Duration,
    mut operation: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let mut delay = first_delay;
    let mut left = attempts;
    loop {
        left = left.saturating_sub(1);
        match operation() {
            Err(error) if left > 0 && is_busy(&error) => {
                thread::sleep(delay);
                delay = delay.saturating_mul(2);
            }
            other => return other,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::time::Duration;

    use super::{is_busy, retry};

    fn denied() -> io::Error {
        io::Error::from(io::ErrorKind::PermissionDenied)
    }

    #[cfg(windows)]
    #[test]
    fn a_busy_folder_is_tried_again_until_it_is_free() {
        let mut calls = 0;
        let result = retry(5, Duration::ZERO, || {
            calls += 1;
            if calls < 3 { Err(denied()) } else { Ok(()) }
        });

        assert!(result.is_ok());
        assert_eq!(calls, 3);
    }

    #[cfg(windows)]
    #[test]
    fn the_attempts_are_bounded_and_the_last_error_is_returned() {
        let mut calls = 0;
        let result = retry(4, Duration::ZERO, || {
            calls += 1;
            Err(denied())
        });

        assert_eq!(calls, 4);
        assert!(result.is_err_and(|error| is_busy(&error)));
    }

    #[test]
    fn another_error_is_not_tried_again() {
        let mut calls = 0;
        let result = retry(5, Duration::ZERO, || {
            calls += 1;
            Err(io::Error::from(io::ErrorKind::NotFound))
        });

        assert_eq!(calls, 1);
        assert!(result.is_err());
    }

    #[cfg(windows)]
    #[test]
    fn the_windows_codes_for_access_denied_and_a_sharing_violation_are_busy() {
        assert!(is_busy(&io::Error::from_raw_os_error(5)));
        assert!(is_busy(&io::Error::from_raw_os_error(32)));
        assert!(!is_busy(&io::Error::from(io::ErrorKind::NotFound)));
    }

    #[cfg(not(windows))]
    #[test]
    fn off_windows_neither_an_io_error_nor_a_permission_error_is_busy() {
        assert!(!is_busy(&io::Error::from_raw_os_error(5)));
        assert!(!is_busy(&io::Error::from_raw_os_error(32)));
        assert!(!is_busy(&denied()));
    }
}
