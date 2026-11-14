// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{ConversionError, ExtractionError, QueryExportError, WriteProtectionError};

impl From<WriteProtectionError> for ConversionError {
    fn from(error: WriteProtectionError) -> Self {
        if error.is_cancelled() {
            Self::Cancelled
        } else {
            Self::Installation(error)
        }
    }
}

impl From<WriteProtectionError> for QueryExportError {
    fn from(error: WriteProtectionError) -> Self {
        if error.is_cancelled() {
            Self::Cancelled
        } else {
            Self::Installation(error)
        }
    }
}

pub(crate) fn extraction_cancelled(error: &ExtractionError) -> bool {
    match error {
        ExtractionError::Installation(cause) => cause.is_cancelled(),
        ExtractionError::Texture(cause) => conversion_cancelled(cause),
        ExtractionError::Sound(cause) => conversion_cancelled(cause),
        _ => false,
    }
}

pub(crate) fn retained_output(error: &ExtractionError) -> bool {
    match error {
        ExtractionError::OwnedOutput(_) | ExtractionError::CleanupFailed { .. } => true,
        ExtractionError::Texture(error) => retained_conversion_output(error),
        ExtractionError::Sound(error) => retained_conversion_output(error),
        ExtractionError::Write { source, .. } => {
            crate::owned_output_failure(source).is_some_and(|error| !error.is_cancelled())
        }
        _ => false,
    }
}

fn retained_conversion_output(error: &ConversionError) -> bool {
    match error {
        ConversionError::OwnedOutput(_) | ConversionError::CleanupFailed { .. } => true,
        ConversionError::Write { source, .. } => {
            crate::owned_output_failure(source).is_some_and(|error| !error.is_cancelled())
        }
        _ => false,
    }
}

const fn conversion_cancelled(error: &ConversionError) -> bool {
    match error {
        ConversionError::Cancelled => true,
        ConversionError::Installation(cause) => cause.is_cancelled(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::atomic::AtomicBool;

    use super::{ConversionError, QueryExportError};

    #[test]
    fn cancelled_guard_lookup_keeps_existing_conversion_and_query_cancellation_types() {
        let error =
            crate::ArchiveWriteProtection::new(Path::new("not-opened.p4k"), &AtomicBool::new(true))
                .expect_err("cancel before source observation");
        assert!(matches!(
            ConversionError::from(error.clone()),
            ConversionError::Cancelled
        ));
        assert!(matches!(
            QueryExportError::from(error),
            QueryExportError::Cancelled
        ));
    }
}
