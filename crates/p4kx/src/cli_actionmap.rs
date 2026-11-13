// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use p4kx_export::{ActionMapFormat, Language};

use crate::dcb_export::FlatConflict;
use crate::exit::EXIT_CODES_HELP;

/// Arguments of `p4kx actionmap`.
#[derive(Debug, Args)]
pub(crate) struct ActionMapArgs {
    /// What to do with the action maps.
    #[command(subcommand)]
    pub(crate) action: ActionMapAction,
}

/// The actions of `p4kx actionmap`.
#[derive(Debug, Subcommand)]
pub(crate) enum ActionMapAction {
    /// Writes every action of the default profile with its localised label and its default
    /// bindings per device as JSON or CSV.
    #[command(after_help = EXIT_CODES_HELP)]
    Export(ActionMapExportArgs),
}

/// Arguments of `p4kx actionmap export`.
#[derive(Debug, Args)]
pub(crate) struct ActionMapExportArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// The file format.
    #[arg(long, value_enum, default_value_t = ActionMapOutput::Json)]
    pub(crate) format: ActionMapOutput,
    /// The language of the labels: a code such as `en`, `de`, `es-419` or `zh-Hans`, or the
    /// folder name of the Archive such as `german_(germany)`. A label whose key has no text in it
    /// stays empty.
    #[arg(
        long,
        alias = "lang",
        value_name = "LANGUAGE",
        default_value = "en",
        value_parser = clap::value_parser!(Language)
    )]
    pub(crate) language: Language,
    /// With --format csv: writes the exact texts, without the leading ' that keeps a spreadsheet
    /// from running a text as a formula; the byte order mark stays.
    #[arg(long)]
    pub(crate) csv_raw: bool,
    /// The file to write; an existing file is replaced. The export goes to stdout without it.
    #[arg(long, value_name = "FILE")]
    pub(crate) out: Option<PathBuf>,
    /// Writes the raw stdout stream's field-origin companion to stderr.
    #[arg(long, conflicts_with = "out")]
    pub(crate) origin_stderr: bool,
    /// Saves the raw stdout stream's field-origin companion to this file.
    #[arg(long, value_name = "FILE", conflicts_with = "out")]
    pub(crate) origin_out: Option<PathBuf>,
}

/// The values of `p4kx actionmap export --format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ActionMapOutput {
    /// One JSON object with the action maps, their actions and every binding.
    Json,
    /// CSV with a UTF-8 byte order mark and CRLF line ends, one row per action.
    Csv,
}

impl ActionMapOutput {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Csv => "csv",
        }
    }
}

impl ActionMapExportArgs {
    /// Returns the library format, or the conflict of `--csv-raw` with another format.
    pub(crate) const fn library_format(&self) -> Result<ActionMapFormat, FlatConflict> {
        match (self.format, self.csv_raw) {
            (ActionMapOutput::Csv, true) => Ok(ActionMapFormat::CsvRaw),
            (ActionMapOutput::Json, true) => Err(FlatConflict("--csv-raw needs --format csv")),
            (ActionMapOutput::Csv, false) => Ok(ActionMapFormat::Csv),
            (ActionMapOutput::Json, false) => Ok(ActionMapFormat::Json),
        }
    }
}
