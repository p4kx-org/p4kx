// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use crate::{
    CancelToken, ConversionError, ExtractionError, OwnedFileToken, OwnedOutputCleanupError,
    QueryExportError,
};

pub(crate) fn conversion<T>(
    result: Result<T, ConversionError>,
    token: &OwnedFileToken,
    path: &Path,
    cancel: Option<&CancelToken>,
) -> Result<T, ConversionError> {
    cleaned(result, token, path, cancel, |primary, cleanup| {
        ConversionError::CleanupFailed { primary, cleanup }
    })
}

pub(crate) fn extraction<T>(
    result: Result<T, ExtractionError>,
    token: &OwnedFileToken,
    path: &Path,
    cancel: Option<&CancelToken>,
) -> Result<T, ExtractionError> {
    cleaned(result, token, path, cancel, |primary, cleanup| {
        ExtractionError::CleanupFailed { primary, cleanup }
    })
}

pub(crate) fn query<T>(
    result: Result<T, QueryExportError>,
    token: &OwnedFileToken,
    path: &Path,
    cancel: Option<&CancelToken>,
) -> Result<T, QueryExportError> {
    cleaned(result, token, path, cancel, |primary, cleanup| {
        QueryExportError::CleanupFailed { primary, cleanup }
    })
}

fn cleaned<T, E: std::error::Error + Send + Sync + 'static>(
    result: Result<T, E>,
    token: &OwnedFileToken,
    path: &Path,
    cancel: Option<&CancelToken>,
    failure: impl FnOnce(
        Option<Box<dyn std::error::Error + Send + Sync>>,
        Box<OwnedOutputCleanupError>,
    ) -> E,
) -> Result<T, E> {
    match token.remove(path, crate::output_file::cancel_flag(cancel)) {
        Ok(()) => result,
        Err(error) => Err(failure(
            result
                .err()
                .map(|primary| Box::new(primary) as Box<dyn std::error::Error + Send + Sync>),
            Box::new(error),
        )),
    }
}
