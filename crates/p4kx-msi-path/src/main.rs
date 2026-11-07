// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Maintains transactional per-user MSI PATH ownership.

#[cfg(windows)]
use p4kx_msi_path::notification;
#[cfg(windows)]
mod notification_windows;

#[cfg(windows)]
fn main() -> Result<(), p4kx_msi_path::Error> {
    p4kx_msi_path::execute(
        std::env::args_os().skip(1),
        notification_windows::environment_changed,
    )
}

#[cfg(not(windows))]
const fn main() -> Result<(), p4kx_msi_path::Error> {
    Err(p4kx_msi_path::Error::unsupported_platform())
}
