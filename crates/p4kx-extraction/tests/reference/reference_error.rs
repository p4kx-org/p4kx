// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Code {
    MissingManifest,
    Manifest,
    Bounds,
    Provenance,
    Selection,
    Identity,
    Path,
    Artifact,
    Incomplete,
    Coverage,
    Changed,
}

#[derive(Debug)]
pub(crate) struct ReferenceError {
    pub(crate) code: Code,
    detail: String,
}

impl ReferenceError {
    pub(crate) fn new(code: Code, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into().chars().take(512).collect(),
        }
    }
}

impl fmt::Display for ReferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.detail)
    }
}

impl std::error::Error for ReferenceError {}

pub(crate) type Result<T> = std::result::Result<T, ReferenceError>;
