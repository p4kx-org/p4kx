// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::archive_error_in;

use crate::model::describe;
use crate::{EntryIndex, ModelExportFailure};

/// Returns why reading or writing an Assembly failed with `error`.
pub(crate) fn failure_of(
    index: &EntryIndex,
    error: &(dyn std::error::Error + 'static),
) -> ModelExportFailure {
    if index.watch().check().is_err() {
        return ModelExportFailure::ArchiveChanged;
    }
    if let Some(archive) = archive_error_in(error) {
        if archive.is_key_required() {
            return ModelExportFailure::KeyRequired;
        }
        if archive.is_wrong_key() {
            return ModelExportFailure::KeyRejected;
        }
    }
    if let Some(refused) = crate::installation::refusal_in(error) {
        return ModelExportFailure::Refused(refused.clone());
    }
    ModelExportFailure::Failed(describe(error))
}
