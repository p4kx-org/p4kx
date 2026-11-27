// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;
use std::time::SystemTime;

use p4kx_channels::PublicPatches;
use p4kx_extraction::{Archive, CancelToken};
use serde::Serialize;
use thiserror::Error;

use super::ReducedError;
use super::bounded_output::Sizes;
use super::{document, plan};
use crate::catalog_rows::spool::RowSpoolError;
use crate::{CatalogError, CatalogFile, CatalogOptions, StarmapTraversalBudget};

/// Source and execution options for the reduced Basetool Catalog.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct BasetoolCatalogOptions<'a> {
    /// Export timestamp recorded in the source metadata.
    pub exported_at: SystemTime,
    /// Optional cancellation signal for reading and writing.
    pub cancel: Option<&'a CancelToken>,
    /// Public patch labels for the observed archive build.
    pub public_patches: Option<&'a PublicPatches>,
    /// Optional limits on original system-container traversal.
    pub starmap_budget: StarmapTraversalBudget,
}

impl BasetoolCatalogOptions<'_> {
    /// Creates options for an unrestricted read with the given export time.
    #[must_use]
    pub const fn new(exported_at: SystemTime) -> Self {
        Self {
            exported_at,
            cancel: None,
            public_patches: None,
            starmap_budget: StarmapTraversalBudget::new(),
        }
    }
}

/// The measured wire sizes of a prepared reduced Basetool Catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct BasetoolCatalogSummary {
    /// The reduced format identifier.
    pub format: &'static str,
    /// The independent reduced wire version.
    pub format_version: u32,
    /// Bytes in the JSON payload, including its final newline.
    pub decoded_bytes: u64,
    /// Bytes in the prepared Zstandard frame.
    pub compressed_bytes: u64,
}

/// A failure to prepare the selected Basetool Catalog.
#[derive(Debug, Error)]
#[error(transparent)]
pub struct BasetoolCatalogError(ReducedError);

impl BasetoolCatalogError {
    /// Returns whether cancellation stopped preparation before publication.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        matches!(
            &self.0,
            ReducedError::Catalog(CatalogError::Cancelled)
                | ReducedError::Spool(RowSpoolError::Cancelled)
        )
    }

    /// Extracts an underlying shared catalog failure.
    ///
    /// # Errors
    /// Returns this error if it holds a reduced projection or output failure.
    pub fn into_catalog_error(self) -> Result<CatalogError, Self> {
        match self.0 {
            ReducedError::Catalog(error) => Ok(error),
            ReducedError::Spool(RowSpoolError::Cancelled) => Ok(CatalogError::Cancelled),
            other => Err(Self(other)),
        }
    }
}

/// Stages a compressed Basetool Catalog with the fixed selected scope and locales.
///
/// Supply an existing writable scratch directory outside the game installation.
/// The callback runs after selection and decoded-size checks pass. Commit the returned
/// file with an Archive watch created before calling this function.
///
/// # Errors
///
/// Returns source, cancellation, size or write errors and any failed output cleanup.
pub fn write_basetool_catalog_file<'output>(
    archive: &Archive,
    options: &BasetoolCatalogOptions<'_>,
    scratch: &Path,
    create: impl FnOnce() -> io::Result<CatalogFile<'output>>,
) -> Result<(BasetoolCatalogSummary, CatalogFile<'output>), BasetoolCatalogError> {
    let mut catalog_options = CatalogOptions::new(options.exported_at);
    catalog_options.cancel = options.cancel;
    catalog_options.public_patches = options.public_patches;
    catalog_options.starmap_budget = options.starmap_budget;
    let (
        file,
        Sizes {
            decoded,
            compressed,
        },
    ) = document::prepare(archive, scratch, &catalog_options, create)
        .map_err(BasetoolCatalogError)?;
    Ok((
        BasetoolCatalogSummary {
            format: plan::FORMAT,
            format_version: plan::FORMAT_VERSION,
            decoded_bytes: decoded,
            compressed_bytes: compressed,
        },
        file,
    ))
}
