// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use p4kx_export::{Language, TableFormat};

use crate::exit::EXIT_CODES_HELP;

/// Arguments of `p4kx loc`.
#[derive(Debug, Args)]
pub(crate) struct LocArgs {
    /// What to do with the localisation texts.
    #[command(subcommand)]
    pub(crate) action: LocAction,
}

/// The actions of `p4kx loc`.
#[derive(Debug, Subcommand)]
pub(crate) enum LocAction {
    /// Lists the languages the Archive holds a global.ini for.
    #[command(after_help = EXIT_CODES_HELP)]
    Languages {
        /// Path of the Data.p4k Archive.
        archive: PathBuf,
    },
    /// Prints the text of a localisation key; exits with 2 when the key has no text.
    #[command(after_help = EXIT_CODES_HELP)]
    Get(LocGetArgs),
    /// Writes every localisation key with its texts per language as JSON or CSV.
    #[command(after_help = EXIT_CODES_HELP)]
    Export(LocExportArgs),
}

/// Arguments of `p4kx loc get`.
#[derive(Debug, Args)]
pub(crate) struct LocGetArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// The localisation key, with or without the leading `@`, ignoring case.
    pub(crate) key: String,
    /// The language to resolve in: a code such as `en`, `de`, `es-419` or `zh-Hans`, or the
    /// folder name of the Archive such as `german_(germany)`.
    #[arg(long, value_name = "LANGUAGE", default_value = "en", value_parser = clap::value_parser!(Language))]
    pub(crate) lang: Language,
    /// The language to resolve in when --lang has no text for the key; there is none unless
    /// given, and `--fallback en` is the usual choice.
    #[arg(long, value_name = "LANGUAGE", value_parser = clap::value_parser!(Language))]
    pub(crate) fallback: Option<Language>,
}

/// Arguments of `p4kx loc export`.
#[derive(Debug, Args)]
pub(crate) struct LocExportArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// The languages to write, one column each; every language the Archive holds when not given.
    /// Keys without a text in any of them are left out.
    #[arg(
        long,
        value_name = "LANGUAGE",
        value_delimiter = ',',
        num_args = 1..,
        value_parser = clap::value_parser!(Language)
    )]
    pub(crate) lang: Vec<Language>,
    /// The file format.
    #[arg(long, value_enum, default_value_t = TableOutput::Json)]
    pub(crate) format: TableOutput,
    /// With --format csv: writes the exact keys and texts, without the leading ' that keeps a
    /// spreadsheet from running a text as a formula; the byte order mark stays.
    #[arg(long)]
    pub(crate) csv_raw: bool,
    /// The file to write; an existing file is replaced. The table goes to stdout without it.
    #[arg(long, value_name = "FILE")]
    pub(crate) out: Option<PathBuf>,
    /// Writes the raw stdout table's field-origin companion to stderr.
    #[arg(long, conflicts_with = "out")]
    pub(crate) origin_stderr: bool,
    /// Saves the raw stdout table's field-origin companion to this file.
    #[arg(long, value_name = "FILE", conflicts_with = "out")]
    pub(crate) origin_out: Option<PathBuf>,
}

/// The values of `p4kx loc export --format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum TableOutput {
    /// One JSON object with the languages and one entry per key.
    Json,
    /// CSV with a UTF-8 byte order mark and CRLF line ends, one row per key.
    Csv,
}

impl From<TableOutput> for TableFormat {
    fn from(output: TableOutput) -> Self {
        match output {
            TableOutput::Json => Self::Json,
            TableOutput::Csv => Self::Csv,
        }
    }
}
