// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Input,
    Bounds,
    Cancelled,
    Identity,
    Mapping,
}

#[derive(Debug)]
pub(crate) struct Error {
    pub(crate) kind: Kind,
    detail: String,
}

impl Error {
    pub(crate) fn new(kind: Kind, detail: impl fmt::Display) -> Self {
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

pub(crate) fn check(cancelled: &impl Fn() -> bool) -> super::Result<()> {
    if cancelled() {
        Err(Error::new(Kind::Cancelled, "verification cancelled").into())
    } else {
        Ok(())
    }
}
