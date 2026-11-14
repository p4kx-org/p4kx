// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

use crate::cli_bnk_objects::BnkObjectsArgs;
use crate::exit::EXIT_CODES_HELP;

/// Arguments of `p4kx bnk`.
#[derive(Debug, Args)]
pub(crate) struct BnkArgs {
    /// What to do with the soundbank.
    #[command(subcommand)]
    pub(crate) action: BnkAction,
}

/// The actions of `p4kx bnk`.
#[derive(Debug, Subcommand)]
pub(crate) enum BnkAction {
    /// Prints the ID, codec and duration of every WEM a soundbank embeds.
    #[command(after_help = EXIT_CODES_HELP)]
    List(BnkSource),
    /// Prints the objects of the HIRC section in bank order: type, ID, parent and children, and
    /// for music objects the duration and markers of a segment, the clips of a track and the
    /// items of a playlist with their loop counts. An object of a type or bank version this tool
    /// does not decode, or whose body is malformed, is listed as such and counted in the
    /// summary. Prints the structure only; it does not resolve triggers or render playback.
    #[command(after_help = EXIT_CODES_HELP)]
    Objects(BnkObjectsArgs),
    /// Writes every WEM a soundbank embeds into a folder as `<ID>.wem`, or converted as `<ID>.wav`
    /// or `<ID>.ogg`.
    #[command(after_help = EXIT_CODES_HELP)]
    Extract {
        /// The soundbank.
        #[command(flatten)]
        source: BnkSource,
        /// The target folder; created if missing, existing files of the same name are replaced.
        #[arg(long, value_name = "FOLDER")]
        out: PathBuf,
        /// The format of the files; a WEM whose codec cannot be converted fails on its own.
        #[arg(long, value_enum, default_value_t = BnkFormat::Wem)]
        format: BnkFormat,
    },
}

/// Where a soundbank is.
#[derive(Debug, Args)]
pub(crate) struct BnkSource {
    /// A Data.p4k Archive when ENTRY is given; otherwise the soundbank file.
    #[arg(value_name = "ARCHIVE_OR_FILE")]
    pub(crate) path: PathBuf,
    /// Path of the soundbank Entry in the Archive, with `\` or `/` as separator, ignoring case.
    pub(crate) entry: Option<String>,
}

/// The values of `p4kx bnk extract --format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum BnkFormat {
    /// The WEM bytes unchanged.
    Wem,
    /// A 16-bit PCM WAV.
    Wav,
    /// An Ogg Vorbis or Ogg Opus file rebuilt from a Wwise Vorbis or Opus WEM without re-encoding.
    Ogg,
}
