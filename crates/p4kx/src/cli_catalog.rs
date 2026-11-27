// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use p4kx_extraction::ProvenanceMode;

/// Arguments of `p4kx catalog`.
#[derive(Debug, Args)]
pub(crate) struct CatalogArgs {
    /// Selects a named catalog export or prints the catalog schema.
    #[command(subcommand)]
    pub(crate) command: Option<CatalogCommand>,
    /// Path of the Data.p4k Archive.
    #[arg(required = true)]
    pub(crate) archive: Option<PathBuf>,
    /// The catalog file to replace. A `.zst` name writes one zstd frame with the content size.
    /// A `.gz` name is refused. Other names write plain JSON.
    #[arg(long, value_name = "FILE", required = true)]
    pub(crate) out: Option<PathBuf>,
    /// Sets the compression level from 1 to 22 for a `.zst` output. Defaults to 19.
    #[arg(long, value_name = "LEVEL", value_parser = clap::value_parser!(i32).range(1..=22))]
    pub(crate) zstd_level: Option<i32>,
    /// Selects Records by record path using the Filter rules of `p4kx search`. Filtered kinds are
    /// marked partial. The global `tags` and `shopInventories` kinds remain complete.
    #[arg(long, value_name = "FILTER")]
    pub(crate) filter: Option<String>,
    /// Writes an explicitly marked diagnostic catalog even when selected source data could not
    /// be read or projected; an incomplete result still exits with status 1.
    #[arg(long)]
    pub(crate) diagnostic_partial: bool,
    /// Chooses marker-only provenance or complete proofs.
    #[arg(long, global = true, value_enum, value_name = "MODE", default_value_t = CatalogProvenanceArg::MarkersOnly)]
    pub(crate) provenance: CatalogProvenanceArg,
    /// Prints each export phase on its own stderr line, including when redirected.
    #[arg(long)]
    pub(crate) progress_phases: bool,
    /// The export time the catalog states, in seconds since 1970-01-01 UTC; defaults to the
    /// current time.
    #[arg(long, env = "SOURCE_DATE_EPOCH", value_name = "SECONDS")]
    pub(crate) exported_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CatalogProvenanceArg {
    Full,
    MarkersOnly,
}

impl From<CatalogProvenanceArg> for ProvenanceMode {
    fn from(mode: CatalogProvenanceArg) -> Self {
        match mode {
            CatalogProvenanceArg::Full => Self::Full,
            CatalogProvenanceArg::MarkersOnly => Self::MarkersOnly,
        }
    }
}

/// The subcommands of `p4kx catalog`.
#[derive(Debug, Subcommand)]
pub(crate) enum CatalogCommand {
    /// Writes the reduced Basetool catalog as compressed JSON.
    Basetool(crate::catalog_basetool::BasetoolArgs),
    /// Prints the JSON Schema (draft 2020-12) of the selected catalog profile.
    Schema(CatalogSchemaArgs),
}

#[derive(Debug, Args)]
pub(crate) struct CatalogSchemaArgs {
    /// Selects the standard or reduced Basetool wire format.
    #[arg(long, value_enum, default_value_t = CatalogSchemaProfile::Standard)]
    pub(crate) profile: CatalogSchemaProfile,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CatalogSchemaProfile {
    Standard,
    Basetool,
}
