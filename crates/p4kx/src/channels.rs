// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use p4kx_channels::{SearchPaths, detect};
use p4kx_settings::{Settings, default_settings_file};
use serde::Serialize;
use thiserror::Error;

use crate::cli::{ChannelsAction, ChannelsArgs};
use crate::json::{self, BuildJson};

const SETTINGS_FILE: &str = "settings.toml";
const UNKNOWN: &str = "unknown";

/// A `channels add` or `channels remove` argument that names nothing usable.
#[derive(Debug, Error)]
pub(crate) enum InvalidPath {
    /// The path to add does not exist.
    #[error("{0} does not exist")]
    Missing(PathBuf),
    /// The path to remove was never added.
    #[error("{0} is not a manually added folder or Archive")]
    NotAdded(PathBuf),
}

#[derive(Serialize)]
struct ChannelsJson<'a> {
    channels: Vec<ChannelJson<'a>>,
}

#[derive(Serialize)]
struct ChannelJson<'a> {
    name: &'a str,
    build: Option<BuildJson<'a>>,
    source: String,
    archive: String,
}

#[derive(Serialize)]
struct ChangeJson {
    action: Action,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<Kind>,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Added,
    AlreadyAdded,
    Removed,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Archive,
    Folder,
}

/// Lists the detected Channels, or adds or removes a manual entry in the settings.
pub(crate) fn run(args: &ChannelsArgs, as_json: bool) -> Result<()> {
    let settings_file = settings_file(args)?;
    match &args.action {
        None => list(args, &settings_file, as_json),
        Some(ChannelsAction::Add { path }) => add(path, &settings_file, as_json),
        Some(ChannelsAction::Remove { path }) => remove(path, &settings_file, as_json),
    }
}

fn settings_file(args: &ChannelsArgs) -> Result<PathBuf> {
    args.config_dir.as_ref().map_or_else(
        || default_settings_file().context("this platform has no config directory"),
        |dir| Ok(dir.join(SETTINGS_FILE)),
    )
}

fn list(args: &ChannelsArgs, settings_file: &Path, as_json: bool) -> Result<()> {
    let settings = Settings::load(settings_file)?;
    let standard = SearchPaths::standard();
    let paths = SearchPaths {
        default_library: args.default_library.clone().or(standard.default_library),
        launcher: args.launcher_dir.clone().or(standard.launcher),
        wine_prefixes: standard.wine_prefixes,
        manual_folders: settings.channels.folders,
        manual_archives: settings.channels.archives,
    };
    let channels = detect(&paths);
    if as_json {
        json::print(&ChannelsJson {
            channels: channels
                .iter()
                .map(|channel| ChannelJson {
                    name: channel.name(),
                    build: channel.build().map(BuildJson::from),
                    source: channel.source().to_string(),
                    archive: json::path(channel.archive()),
                })
                .collect(),
        })?;
        return Ok(());
    }
    let mut out = BufWriter::new(io::stdout().lock());
    write_row(&mut out, ["CHANNEL", "VERSION", "SOURCE", "ARCHIVE"])?;
    for channel in &channels {
        write_row(
            &mut out,
            [
                channel.name(),
                channel.build().map_or(UNKNOWN, |build| build.version()),
                &channel.source().to_string(),
                &channel.archive().display().to_string(),
            ],
        )?;
    }
    out.flush()?;
    Ok(())
}

fn add(path: &Path, settings_file: &Path, as_json: bool) -> Result<()> {
    if !path.exists() {
        return Err(InvalidPath::Missing(path.to_path_buf()).into());
    }
    let path =
        std::path::absolute(path).with_context(|| format!("cannot resolve {}", path.display()))?;
    let mut settings = Settings::load(settings_file)?;
    let is_archive = path.is_file();
    let added = if is_archive {
        settings.channels.add_archive(path.clone())
    } else {
        settings.channels.add_folder(path.clone())
    };
    if added {
        settings.save(settings_file)?;
    }
    if as_json {
        json::print(&ChangeJson {
            action: if added {
                Action::Added
            } else {
                Action::AlreadyAdded
            },
            kind: Some(if is_archive {
                Kind::Archive
            } else {
                Kind::Folder
            }),
            path: json::path(&path),
        })?;
    } else {
        let verb = if added { "added" } else { "already added" };
        let kind = if is_archive { "Archive" } else { "folder" };
        writeln!(io::stdout().lock(), "{verb} {kind} {}", path.display())?;
    }
    Ok(())
}

fn remove(path: &Path, settings_file: &Path, as_json: bool) -> Result<()> {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut settings = Settings::load(settings_file)?;
    if !settings.channels.remove(&absolute) {
        return Err(InvalidPath::NotAdded(absolute).into());
    }
    settings.save(settings_file)?;
    if as_json {
        json::print(&ChangeJson {
            action: Action::Removed,
            kind: None,
            path: json::path(&absolute),
        })?;
    } else {
        writeln!(io::stdout().lock(), "removed {}", absolute.display())?;
    }
    Ok(())
}

fn write_row(
    out: &mut impl Write,
    [channel, version, source, archive]: [&str; 4],
) -> io::Result<()> {
    writeln!(out, "{channel:<14} {version:<16} {source:<9} {archive}")
}
