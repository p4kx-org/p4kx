// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt;

use crate::MaterialError;

/// A material syntax/projection failure or the caller's original interruption.
#[derive(Debug)]
#[non_exhaustive]
pub enum MaterialReadError<Interrupted> {
    /// The existing XML/material reader could not interpret the input.
    Material(MaterialError),
    /// The checkpoint stopped this read without converting it to a source defect.
    Interrupted(Interrupted),
}

impl<Interrupted: fmt::Display> fmt::Display for MaterialReadError<Interrupted> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Material(error) => error.fmt(formatter),
            Self::Interrupted(error) => error.fmt(formatter),
        }
    }
}

impl<Interrupted: Error + 'static> Error for MaterialReadError<Interrupted> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Material(error) => Some(error),
            Self::Interrupted(error) => Some(error),
        }
    }
}

impl<Interrupted> From<MaterialError> for MaterialReadError<Interrupted> {
    fn from(error: MaterialError) -> Self {
        Self::Material(error)
    }
}
