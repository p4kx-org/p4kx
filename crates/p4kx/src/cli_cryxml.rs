// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::Args;

/// Arguments of `p4kx cryxml`.
#[derive(Debug, Args)]
pub(crate) struct CryXmlArgs {
    /// Path of the CryXml or text XML file.
    pub(crate) file: PathBuf,
    /// Writes the XML to this file instead of stdout; needed with --json.
    #[arg(long, value_name = "FILE")]
    pub(crate) out: Option<PathBuf>,
    /// Writes the raw stdout stream's field-origin companion to stderr.
    #[arg(long, conflicts_with = "out")]
    pub(crate) origin_stderr: bool,
    /// Saves the raw stdout stream's field-origin companion to this file.
    #[arg(long, value_name = "FILE", conflicts_with = "out")]
    pub(crate) origin_out: Option<PathBuf>,
}
