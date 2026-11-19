// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{Args, Subcommand};

/// Arguments of `p4kx catalog`.
#[derive(Debug, Args)]
pub(crate) struct CatalogArgs {
    /// Prints the schema instead of writing a catalog.
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
    /// Selects only the Records whose record path matches, with the rules of `p4kx search`; every
    /// kind is then marked partial with this Filter.
    #[arg(long, value_name = "FILTER")]
    pub(crate) filter: Option<String>,
    /// Writes an explicitly marked diagnostic catalog even when selected source data could not
    /// be read or projected; an incomplete result still exits with status 1.
    #[arg(long)]
    pub(crate) diagnostic_partial: bool,
    /// Prints each export phase on its own stderr line, including when redirected.
    #[arg(long)]
    pub(crate) progress_phases: bool,
    /// The export time the catalog states, in seconds since 1970-01-01 UTC; defaults to the
    /// current time.
    #[arg(long, env = "SOURCE_DATE_EPOCH", value_name = "SECONDS")]
    pub(crate) exported_at: Option<u64>,
}

/// The subcommands of `p4kx catalog`.
#[derive(Debug, Subcommand)]
pub(crate) enum CatalogCommand {
    /// Prints the JSON Schema (draft 2020-12) of the P4KX catalog.
    Schema,
}
