// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsString;
use std::fmt;

use crate::installer::notification::{Failure, Report};

/// A private installer failure retaining its typed original cause.
pub struct Error {
    kind: ErrorKind,
}

enum ErrorKind {
    UnsupportedPlatform,
    #[cfg(windows)]
    Arguments(crate::installer::fault::Fault),
    #[cfg(windows)]
    Operation(
        crate::installer::driver::DriverError<crate::installer::registry_adapter::AdapterFault>,
    ),
}

impl Error {
    /// Returns an error for a platform without the Windows MSI adapter.
    #[must_use]
    pub const fn unsupported_platform() -> Self {
        Self {
            kind: ErrorKind::UnsupportedPlatform,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::UnsupportedPlatform => {
                formatter.write_str("the MSI helper requires Windows")
            }
            #[cfg(windows)]
            ErrorKind::Arguments(error) => fmt::Display::fmt(error, formatter),
            #[cfg(windows)]
            ErrorKind::Operation(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::UnsupportedPlatform => None,
            #[cfg(windows)]
            ErrorKind::Arguments(error) => Some(error),
            #[cfg(windows)]
            ErrorKind::Operation(error) => Some(error),
        }
    }
}

/// Executes one private MSI phase with the fixed environment notifier.
///
/// # Errors
/// Returns malformed arguments, random-source or ownership failures, original Registry errors or incomplete notification.
#[cfg(windows)]
pub fn execute(
    arguments: impl IntoIterator<Item = OsString>,
    notify: impl FnMut() -> Result<Report, Failure>,
) -> Result<(), Error> {
    use std::os::windows::ffi::OsStrExt as _;
    let words = arguments.into_iter().map(|argument| {
        argument
            .encode_wide()
            .take(crate::installer::arguments::MAX_ARGUMENT_WORDS + 1)
            .collect()
    });
    let command = crate::installer::arguments::parse(words).map_err(|error| Error {
        kind: ErrorKind::Arguments(error),
    })?;
    crate::installer::driver::execute(
        &mut crate::installer::registry_adapter::Registry,
        command,
        crate::installer::invocation_nonce::InvocationNonce::generate,
        notify,
    )
    .map_err(|error| Error {
        kind: ErrorKind::Operation(error),
    })
}

/// Rejects installer execution on platforms without the Windows adapter.
///
/// # Errors
/// Always returns an unsupported-platform error without reading installer state.
#[cfg(not(windows))]
pub fn execute(
    _arguments: impl IntoIterator<Item = OsString>,
    _notify: impl FnMut() -> Result<Report, Failure>,
) -> Result<(), Error> {
    Err(Error::unsupported_platform())
}
