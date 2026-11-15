// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{Args, Subcommand};

/// Arguments of `p4kx channels`.
#[derive(Debug, Args)]
pub(crate) struct ChannelsArgs {
    /// Adds or removes a manual entry; without one, the Channels are listed.
    #[command(subcommand)]
    pub(crate) action: Option<ChannelsAction>,
    /// Folder containing settings.toml. Defaults to the active P4KX profile, normally
    /// `$XDG_CONFIG_HOME/P4KX` (`~/.config/P4KX`) on Linux.
    #[arg(long, global = true, env = "P4KX_CONFIG_DIR", value_name = "FOLDER")]
    pub(crate) config_dir: Option<PathBuf>,
    /// Overrides the native RSI Launcher application-data folder. Discovered Wine launcher
    /// logs are still read.
    #[arg(long, global = true, env = "P4KX_LAUNCHER_DIR", value_name = "FOLDER")]
    pub(crate) launcher_dir: Option<PathBuf>,
    /// Overrides the native Star Citizen library folder. Linux still searches Wine, Proton,
    /// Steam, Bottles and immediate ~/Games prefixes for Lutris or custom installations.
    /// macOS still searches configured Wine and `CrossOver` locations.
    #[arg(
        long,
        global = true,
        env = "P4KX_DEFAULT_LIBRARY",
        value_name = "FOLDER"
    )]
    pub(crate) default_library: Option<PathBuf>,
}

/// The actions of `p4kx channels`.
#[derive(Debug, Subcommand)]
pub(crate) enum ChannelsAction {
    /// Adds a Channel folder, a library folder or an Archive to the settings.
    Add {
        /// Path of the folder or Archive.
        path: PathBuf,
    },
    /// Removes a manually added folder or Archive from the settings.
    Remove {
        /// Path of the folder or Archive.
        path: PathBuf,
    },
}
