// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::cli::Conflict;
use crate::exit::EXIT_CODES_HELP;

/// Arguments of `p4kx socpak`.
#[derive(Debug, Args)]
pub(crate) struct SocpakArgs {
    /// What to do with the container.
    #[command(subcommand)]
    pub(crate) action: SocpakAction,
}

/// The actions of `p4kx socpak`.
#[derive(Debug, Subcommand)]
pub(crate) enum SocpakAction {
    /// Lists every Entry of a socpak container: compression method, encryption flag, compressed
    /// size, size and path.
    #[command(after_help = EXIT_CODES_HELP)]
    List(SocpakSource),
    /// Extracts the Entries of a socpak container into a target folder, without extracting the
    /// container itself first.
    #[command(after_help = EXIT_CODES_HELP)]
    Extract {
        /// The container.
        #[command(flatten)]
        source: SocpakSource,
        /// Extracts only the Entries the Filter matches, with the rules of `p4kx search`.
        #[arg(long, value_name = "FILTER")]
        filter: Option<String>,
        /// Leaves out every Entry this Filter matches, with the rules of `p4kx extract --exclude`;
        /// repeatable, also without --filter.
        #[arg(long, value_name = "FILTER")]
        exclude: Vec<String>,
        /// Target folder; created if missing, files already in it are handled by --conflict.
        /// Defaults to a new folder `SC-EXPORT-<Channel>-<Build version>-<timestamp>` in the
        /// current directory.
        #[arg(long, value_name = "FOLDER")]
        out: Option<PathBuf>,
        /// What to do with a file that already exists in the target.
        #[arg(long, value_enum, default_value_t = Conflict::Skip)]
        conflict: Conflict,
        /// Converts CryXml Entries to UTF-8 XML and DDS textures with fixed settings, those of
        /// `p4kx extract --convert --textures merge,png --mip 0`: one merged DDS and a PNG of the
        /// largest mip per texture. Every other Entry is written unchanged.
        #[arg(long)]
        convert: bool,
        /// Checks each Entry against the CRC-32 of the container directory while it is written,
        /// as `p4kx extract --verify` does.
        #[arg(long)]
        verify: bool,
    },
    /// Checks the data of the Entries of a socpak container against the CRC-32 of its directory
    /// and reports every mismatch, as `p4kx verify` does; nothing is written.
    #[command(after_help = EXIT_CODES_HELP)]
    Verify {
        /// The container.
        #[command(flatten)]
        source: SocpakSource,
        /// Checks only the Entries the Filter matches, with the rules of `p4kx search`.
        #[arg(long, value_name = "FILTER")]
        filter: Option<String>,
        /// Leaves out every Entry this Filter matches, with the rules of `p4kx verify --exclude`;
        /// repeatable.
        #[arg(long, value_name = "FILTER")]
        exclude: Vec<String>,
    },
}

/// Where a socpak container is.
#[derive(Debug, Args)]
pub(crate) struct SocpakSource {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// Path of the container Entry in the Archive, with `\` or `/` as separator, ignoring case;
    /// each further path names a container inside the one before.
    #[arg(required = true, value_name = "CONTAINER")]
    pub(crate) containers: Vec<String>,
}
