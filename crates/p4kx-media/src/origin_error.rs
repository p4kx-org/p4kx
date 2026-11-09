// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldPathError, SourceEvidenceError};

use crate::MediaError;

pub(crate) fn evidence(error: SourceEvidenceError) -> MediaError {
    MediaError::Contract(format!("media source evidence rejected: {error}"))
}

pub(crate) fn address(error: &FieldPathError) -> MediaError {
    MediaError::Contract(format!("media origin address rejected: {error}"))
}

pub(crate) fn counter() -> MediaError {
    MediaError::Contract("actual media origin count exceeds its wire width".into())
}

pub(crate) fn foreign() -> MediaError {
    MediaError::Contract("media origins belong to another original source inventory".into())
}
