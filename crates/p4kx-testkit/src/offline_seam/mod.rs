// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded local reference files with containment, alias detection and cancellation.

mod paths;
mod reader;
mod tree;

pub use paths::{contained_file, direct_root, is_linked, relative_path};
pub use reader::{Fingerprint, LoadedFile, fingerprint, load};
pub use tree::inventory;

use std::fmt;

/// A local reference operation's failure category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Filesystem access failed.
    Io,
    /// A path, link or physical alias is invalid.
    Path,
    /// A declared resource bound was exceeded.
    Bounds,
    /// The caller requested cancellation.
    Cancelled,
}

/// A bounded local reference failure.
#[derive(Debug)]
pub struct Error {
    /// The failure category.
    pub kind: ErrorKind,
    detail: String,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, detail: impl fmt::Display) -> Self {
        Self {
            kind,
            detail: detail.to_string().chars().take(512).collect(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for Error {}

/// Bounds for a local reference inventory.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum UTF-8 bytes in one relative path.
    pub path_bytes: usize,
    /// Maximum visited files and directories.
    pub entries: usize,
}

pub(crate) fn check(cancelled: &impl Fn() -> bool) -> Result<(), Error> {
    if cancelled() {
        Err(Error::new(
            ErrorKind::Cancelled,
            "local reference operation cancelled",
        ))
    } else {
        Ok(())
    }
}
