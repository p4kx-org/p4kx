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
    /// Folder of settings.toml; defaults to the platform config directory: %APPDATA%\P4KX on
    /// Windows, the XDG config folder P4KX (~/.config/P4KX) on Linux.
    #[arg(long, global = true, env = "P4KX_CONFIG_DIR", value_name = "FOLDER")]
    pub(crate) config_dir: Option<PathBuf>,
    /// The RSI Launcher's application-data folder; defaults to %APPDATA%\rsilauncher on Windows
    /// and to the rsilauncher folder in the config directory on Linux. The launchers inside Wine
    /// prefixes are read as well.
    #[arg(long, global = true, env = "P4KX_LAUNCHER_DIR", value_name = "FOLDER")]
    pub(crate) launcher_dir: Option<PathBuf>,
    /// The default library folder; defaults to C:\Program Files\Roberts Space Industries\StarCitizen
    /// on Windows. On Linux the libraries in the Wine prefix named by WINEPREFIX and in
    /// ~/Games/star-citizen are searched without it.
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
