// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::LoadoutFileError;
use p4kx_extraction::{
    DataForgeError, DataForgeLoadError, LocalizationError, LocalizationLoadError,
};
use thiserror::Error;

use crate::CatalogRefusal;

/// A Basetool catalog cannot be built.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CatalogError {
    /// Shared field-origin evidence was interrupted during mutation.
    #[error(transparent)]
    SourceEvidence(#[from] p4kx_extraction::SourceEvidenceError),
    /// Retaining the actual source-use set exceeded available storage.
    #[error("cannot allocate catalog source-use evidence")]
    SourceUseAllocation(#[source] std::collections::TryReserveError),
    /// Storage for the original JSON traversal or source arena could not be reserved.
    #[error("cannot reserve catalog JSON source storage")]
    JsonSourceAllocation(#[source] std::collections::TryReserveError),
    /// Storage for a decoded Archive Entry could not be reserved.
    #[error("cannot reserve catalog storage for an Archive Entry of {size} decoded bytes")]
    EntryPayloadAllocation {
        /// Decoded byte length stated by the Entry.
        size: u64,
        /// Why storage for the next decoded bytes could not be reserved.
        #[source]
        source: std::collections::TryReserveError,
    },
    /// The original loadout file admission could not be retained.
    #[error(transparent)]
    LoadoutSourceEvidence(LoadoutFileError),
    /// An Archive-backed DataForge belongs to another observed input or container.
    #[error("the DataForge origin does not match the supplied Archive input")]
    SourceOriginMismatch,
    /// An unpublished catalog could not be removed after its original failure.
    #[error("catalog output cleanup failed: {cleanup}")]
    CleanupFailed {
        /// The original catalog failure.
        #[source]
        primary: Box<Self>,
        /// The separate verified cleanup failure.
        cleanup: Box<p4kx_extraction::OwnedOutputCleanupError>,
    },
    /// Production accounting could not establish a reliable export-health contract.
    #[error(transparent)]
    Accounting(#[from] crate::ExportAccountingError),
    /// Normal publication was refused before any output was created.
    #[error("catalog export is incomplete; explicit diagnostic partial export is required")]
    IncompleteExport(CatalogRefusal),
    /// The DataForge Entry cannot be found or read.
    #[error(transparent)]
    Load(DataForgeLoadError),
    /// The DataForge has an unsupported version or is corrupt.
    #[error(transparent)]
    DataForge(DataForgeError),
    /// A `global.ini` the Archive holds cannot be read.
    #[error(transparent)]
    LoadTexts(LocalizationLoadError),
    /// A `global.ini` is corrupt or exceeds a limit.
    #[error("cannot read the localisation file {path}")]
    Texts {
        /// The Entry path.
        path: String,
        /// Why it cannot be read.
        #[source]
        source: LocalizationError,
    },
    /// The ship store slug data compiled into the tool is invalid.
    #[error("the ship store slug data is invalid: {0}")]
    StoreSlugs(String),
    /// A rendered mission text exceeds the platform string length.
    #[error("rendered mission text exceeds the platform string length")]
    MissionTextLength,
    /// Memory for a mission text or its render plan could not be reserved.
    #[error("cannot reserve memory for mission text")]
    MissionTextAllocation(#[source] std::collections::TryReserveError),
    /// An internal mission text plan cannot be resolved.
    #[error("cannot resolve the mission text render plan")]
    MissionTextPlan,
    /// The catalog cannot be written as JSON.
    #[error("cannot write the catalog as JSON")]
    Json(#[source] serde_json::Error),
    /// The catalog cannot be written to its output.
    #[error("cannot write the catalog")]
    Write(#[source] std::io::Error),
    /// The build was cancelled.
    #[error("the catalog build was cancelled")]
    Cancelled,
}

#[cfg(test)]
mod source_evidence_tests {
    #[test]
    fn source_evidence_failure_remains_a_fatal_typed_catalog_error() {
        let error =
            super::CatalogError::from(p4kx_extraction::SourceEvidenceError::PoisonedEvidence);
        assert!(
            matches!(
                error,
                super::CatalogError::SourceEvidence(
                    p4kx_extraction::SourceEvidenceError::PoisonedEvidence
                )
            ),
            "source evidence must not become a field warning or substitute origin"
        );
    }
}
