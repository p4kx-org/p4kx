// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Captures and restores fixed raw Registry evidence only on acknowledged hosted runners.

#![forbid(unsafe_code)]

#[cfg(windows)]
#[path = "msi_registry_fixture/codec.rs"]
mod codec;
#[cfg(windows)]
#[path = "msi_registry_fixture/io.rs"]
mod file_io;
#[cfg(windows)]
#[path = "msi_registry_fixture/run.rs"]
mod run;

#[cfg(windows)]
fn main() -> std::io::Result<()> {
    run::execute(std::env::args_os().skip(1))
}

#[cfg(not(windows))]
fn main() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "native Registry evidence requires Windows",
    ))
}
