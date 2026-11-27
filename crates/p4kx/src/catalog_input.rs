// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use p4kx_export::{CatalogError, Compression};
use p4kx_extraction::ProvenanceMode;
use thiserror::Error;

/// An export time that no system time can hold.
#[derive(Debug, Error)]
#[error("the export time {0} lies beyond the supported range")]
pub(crate) struct InvalidExportTime(u64);

impl InvalidExportTime {
    /// Returns the error for the export time `seconds`.
    pub(crate) const fn of(seconds: u64) -> Self {
        Self(seconds)
    }
}

/// Converts the explicit Unix timestamp or captures the current time.
///
/// # Errors
/// Returns an error if the explicit timestamp exceeds the system time range.
pub(crate) fn export_time(seconds: Option<u64>) -> Result<SystemTime, InvalidExportTime> {
    seconds.map_or_else(
        || Ok(SystemTime::now()),
        |seconds| {
            UNIX_EPOCH
                .checked_add(Duration::from_secs(seconds))
                .ok_or_else(|| InvalidExportTime::of(seconds))
        },
    )
}

/// `p4kx catalog` was given no Archive or no output file.
#[derive(Debug, Error)]
#[error("p4kx catalog needs an Archive and --out")]
pub(crate) struct MissingCatalogArgs;

/// The output file of `p4kx catalog` ends in `.gz`, which would be written as plain JSON.
#[derive(Debug, Error)]
#[error(
    "{0} would be written as plain JSON, because gzip is not supported; use a name ending in .json.zst for a compressed catalog or .json for a plain one"
)]
pub(crate) struct GzipNotSupported(String);

/// An explicit zstd level was given for a plain output file.
#[derive(Debug, Error)]
#[error("--zstd-level requires an --out filename ending in .zst")]
struct ZstdLevelNeedsCompressedOutput;

/// A provenance option outside the fixed Basetool profile.
#[derive(Debug, Error)]
#[error("the Basetool catalog uses the fixed markers-only provenance profile")]
pub(crate) struct InvalidBasetoolProvenance;

/// Checks the fixed provenance profile of the Basetool catalog.
pub(crate) const fn require_basetool_markers(
    provenance: ProvenanceMode,
) -> Result<(), InvalidBasetoolProvenance> {
    match provenance {
        ProvenanceMode::MarkersOnly => Ok(()),
        _ => Err(InvalidBasetoolProvenance),
    }
}

/// Maps shared catalog failures to the CLI input and Archive error classes.
pub(crate) fn into_anyhow(error: CatalogError) -> anyhow::Error {
    match error {
        CatalogError::Load(error) => error.into(),
        CatalogError::LoadTexts(error) => error.into(),
        CatalogError::DataForge(error) => error.into(),
        other => other.into(),
    }
}

/// Returns whether `error` is invalid catalog command-line input.
pub(crate) fn is_invalid_input(error: &anyhow::Error) -> bool {
    error.downcast_ref::<InvalidExportTime>().is_some()
        || error.downcast_ref::<MissingCatalogArgs>().is_some()
        || error.downcast_ref::<GzipNotSupported>().is_some()
        || error.downcast_ref::<InvalidBasetoolProvenance>().is_some()
        || error
            .downcast_ref::<ZstdLevelNeedsCompressedOutput>()
            .is_some()
}

/// Selects a compression based on the output extension and optional zstd override.
pub(crate) fn compression(out: &Path, zstd_level: Option<i32>) -> anyhow::Result<Compression> {
    refuse_gzip(out)?;
    match (Compression::for_path(out), zstd_level) {
        (compression, None) => Ok(compression),
        (Compression::Zstd { .. }, Some(level)) => Ok(Compression::Zstd { level }),
        (Compression::None, Some(_)) => Err(ZstdLevelNeedsCompressedOutput.into()),
    }
}

/// Fails for an output file ending in `.gz`, ignoring case.
fn refuse_gzip(out: &Path) -> Result<(), GzipNotSupported> {
    if out
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("gz"))
    {
        Err(GzipNotSupported(out.display().to_string()))
    } else {
        Ok(())
    }
}
