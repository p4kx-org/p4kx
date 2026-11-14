// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io;

use crate::{ConversionError, ExtractionError, OwnedOutputError, QueryExportError};

const MAX_ERROR_DEPTH: usize = 64;

/// Finds a typed factory failure without discarding its original I/O wrapper chain.
#[must_use]
pub fn owned_output_failure(error: &io::Error) -> Option<&OwnedOutputError> {
    let mut current: &(dyn Error + 'static) = error;
    for _ in 0..MAX_ERROR_DEPTH {
        if let Some(owned) = current.downcast_ref::<OwnedOutputError>() {
            return Some(owned);
        }
        current = current
            .downcast_ref::<io::Error>()
            .and_then(|error| error.get_ref().map(|inner| inner as &(dyn Error + 'static)))
            .or_else(|| current.source())?;
    }
    None
}

impl From<OwnedOutputError> for ConversionError {
    fn from(error: OwnedOutputError) -> Self {
        match error {
            OwnedOutputError::Protection(error) => error.into(),
            OwnedOutputError::Io { path, source } => Self::Write { path, source },
            other => Self::OwnedOutput(Box::new(other)),
        }
    }
}

impl From<OwnedOutputError> for ExtractionError {
    fn from(error: OwnedOutputError) -> Self {
        match error {
            OwnedOutputError::Protection(error) => Self::Installation(error),
            OwnedOutputError::Io { path, source } => Self::Write { path, source },
            other => Self::OwnedOutput(Box::new(other)),
        }
    }
}

impl From<OwnedOutputError> for QueryExportError {
    fn from(error: OwnedOutputError) -> Self {
        match error {
            OwnedOutputError::Protection(error) => error.into(),
            OwnedOutputError::Io { path, source } => Self::Write { path, source },
            other => Self::OwnedOutput(Box::new(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use crate::OwnedOutputError;

    #[test]
    fn a_factory_wrapper_preserves_the_terminal_capture_failure() {
        let error = OwnedOutputError::RetainedCreation {
            path: "owned.part".into(),
            source: io::Error::from(io::ErrorKind::AlreadyExists),
        }
        .into_io_error();
        let wrapped = io::Error::other(error);
        let failure = super::owned_output_failure(&wrapped).expect("typed capture failure");
        assert!(!failure.is_cancelled());
        let OwnedOutputError::RetainedCreation { source, .. } = failure else {
            panic!("retained creation is preserved");
        };
        assert_eq!(source.kind(), io::ErrorKind::AlreadyExists);
    }
}
