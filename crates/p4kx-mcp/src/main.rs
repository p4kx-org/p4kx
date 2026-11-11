// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx-mcp`: the read-only MCP server of P4KX, spoken to over stdio.

mod archives;
mod audio;
mod audio_convert;
mod audio_excerpt;
mod audio_objects;
mod audio_params;
mod audio_shapes;
mod audio_tools;
mod audio_wem;
mod audio_wem_shapes;
mod audio_window;
mod budget;
mod budget_text;
mod container;
mod content;
mod content_origin;
mod content_text_origin;
mod content_tools;
mod definitions;
mod discovery_notes;
mod entries;
mod error;
mod forge_file;
mod forge_params;
mod forge_shapes;
mod forge_state;
mod gate;
mod image;
mod image_origin;
mod image_tools;
#[cfg(test)]
mod key_fixture;
mod key_report;
mod key_stamp;
mod keys;
mod loadout;
mod loadout_cache;
mod loadout_error;
mod loadout_shape;
mod loadout_shapes;
mod loadout_tools;
mod locate;
mod material;
mod material_shape;
mod material_shapes;
mod material_tools;
mod model_params;
mod model_shapes;
mod model_tools;
mod models;
mod noted;
mod notice;
mod ordered;
mod page;
mod params;
mod picture;
mod query;
mod query_tools;
mod record_tools;
mod records;
mod server;
mod shapes;

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use p4kx_settings::{KeyStore, KeyText, default_settings_file};
use rmcp::ServiceExt;

use crate::keys::KeyChoice;
use crate::locate::Locations;
use crate::server::Server;

const SETTINGS_FILE: &str = "settings.toml";
const KEY_FILE: &str = "archive.key";

/// Read-only MCP server over stdio for the Data.p4k Archives of Star Citizen.
#[derive(Debug, Parser)]
#[command(
    name = "p4kx-mcp",
    version = notice::VERSION,
    before_help = notice::BEFORE_HELP
)]
struct Args {
    /// Checks GitHub for a newer release and prints its link instead of starting a session.
    #[arg(long, exclusive = true)]
    check_update: bool,
    /// The Archive tools use when a call names none: a Channel name such as `LIVE` or the
    /// absolute path of a `.p4k` file.
    #[arg(long)]
    archive: Option<String>,
    /// The folder holding `settings.toml` with the manual Channel entries.
    #[arg(long, env = "P4KX_CONFIG_DIR")]
    config_dir: Option<PathBuf>,
    /// The RSI Launcher's application-data folder.
    #[arg(long, env = "P4KX_LAUNCHER_DIR")]
    launcher_dir: Option<PathBuf>,
    /// The default Library folder.
    #[arg(long, env = "P4KX_DEFAULT_LIBRARY")]
    default_library: Option<PathBuf>,
    /// The Archive key as 32 hexadecimal digits, needed to read encrypted Entries. Without it the
    /// `P4KX_ARCHIVE_KEY` environment variable of the server configuration is used, then the key
    /// stored with `p4kx key set` or in the GUI. Prefer the variable or the stored key: an option
    /// shows up in process lists.
    #[arg(long = "key", value_name = "HEX")]
    archive_key: Option<KeyText>,
    /// The file holding the stored Archive key; `archive.key` in the config folder or in the
    /// profile of the current user when omitted.
    #[arg(long, env = "P4KX_KEY_FILE", value_name = "FILE")]
    key_file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.check_update {
        return write_update_link();
    }
    p4kx_extraction::check_runtime_prerequisites()?;
    let settings_file = args
        .config_dir
        .as_ref()
        .map(|dir| dir.join(SETTINGS_FILE))
        .or_else(default_settings_file);
    let key_file = args
        .key_file
        .or_else(|| args.config_dir.as_ref().map(|dir| dir.join(KEY_FILE)));
    let store = key_file.map_or_else(KeyStore::in_profile, |file| Some(KeyStore::new(file)));
    let keys = KeyChoice::new(args.archive_key, store);
    keys.check().context("the Archive key is not valid")?;
    let locations = Locations::new(args.default_library, args.launcher_dir, settings_file);
    let default_archive = args
        .archive
        .as_deref()
        .map(|archive| locations.resolve(archive, locate::Accepts::Archive))
        .transpose()
        .map_err(|error| anyhow::anyhow!(noted::take().error_message(&error)))?;
    for note in noted::take().discovery {
        writeln!(io::stderr().lock(), "{note}")?;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("cannot start the runtime")?;
    runtime.block_on(async {
        let service = Server::new(locations, default_archive, keys)
            .serve(rmcp::transport::stdio())
            .await
            .context("cannot start the MCP session")?;
        service.waiting().await.context("the MCP session failed")?;
        Ok(())
    })
}

fn write_update_link() -> Result<()> {
    let release = p4kx_update::ReleaseCheck::github().newer_than(env!("CARGO_PKG_VERSION"))?;
    if let Some(release) = release {
        writeln!(io::stdout().lock(), "{}", release.link())?;
    }
    Ok(())
}
