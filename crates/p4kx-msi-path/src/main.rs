// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Maintains transactional per-user MSI PATH ownership.

#[cfg(windows)]
use p4kx_msi_path::notification;

#[cfg(any(windows, test))]
mod notification_recipient;
#[cfg(any(windows, test))]
mod notification_status;
#[cfg(windows)]
mod notification_windows;

#[cfg(windows)]
fn main() {
    let status = run();
    std::process::exit(status);
}

#[cfg(windows)]
fn run() -> i32 {
    use std::process::Termination as _;

    let mut observed = None;
    let result = p4kx_msi_path::execute(std::env::args_os().skip(1), || {
        let notification = notification_windows::environment_changed();
        observed = notification_status::of(&notification);
        notification
    });
    let status = notification_status::completed(result.is_ok(), observed);
    let _reported = result.report();
    status
}

#[cfg(not(windows))]
const fn main() -> Result<(), p4kx_msi_path::Error> {
    Err(p4kx_msi_path::Error::unsupported_platform())
}
