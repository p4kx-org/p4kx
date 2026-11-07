// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForgeError, DataForgeLoadError, LocalizationError, LocalizationLoadError,
};
use thiserror::Error;

/// A Basetool catalog cannot be built.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CatalogError {
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
    IncompleteExport {
        /// The machine-readable source and projection health that prevented publication.
        health: Box<crate::ExportHealth>,
    },
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
