// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use p4kx_channels::{SearchPaths, detect};
use p4kx_comparison::LineDiffError;
use p4kx_extraction::{
    Archive, ArchiveError, ArchiveKey, ArchiveKeyError, Entry, KeyCheck, OpenOptions,
    archive_error_in,
};
use p4kx_settings::{
    ChosenKey, KEY_VARIABLE, KeyChoiceError, KeySource, KeyStore, KeyText, Settings, choose_key,
    default_settings_file,
};
use serde::Serialize;
use thiserror::Error;

use crate::cli::{Cli, Command, KeyAction, KeyArgs};
use crate::json;

static KEY: OnceLock<Option<ArchiveKey>> = OnceLock::new();
static RECOVER: AtomicBool = AtomicBool::new(false);

/// The key options of the command line.
pub(crate) struct Given {
    /// The `--key` option.
    pub(crate) key: Option<KeyText>,
    /// The `--key-file` option.
    pub(crate) file: Option<PathBuf>,
}

/// A key given on the command line or in the environment that is not 32 hexadecimal digits.
#[derive(Debug, Error)]
pub(crate) enum InvalidKey {
    /// The `--key` option or the environment variable.
    #[error(transparent)]
    Choice(#[from] KeyChoiceError),
    /// The key given to `key set`.
    #[error("the Archive key to store is not valid")]
    Text(#[source] ArchiveKeyError),
}

#[derive(Serialize)]
struct StoredJson {
    action: &'static str,
    file: String,
    checked_against: Option<String>,
}

#[derive(Serialize)]
struct RemovedJson {
    action: &'static str,
    file: String,
}

#[derive(Serialize)]
struct ShownJson {
    file: Option<String>,
    stored: Option<String>,
    in_use: Option<InUseJson>,
}

#[derive(Serialize)]
struct InUseJson {
    source: &'static str,
    key: String,
}

/// Chooses the Archive key of this run: `--key`, then `P4KX_ARCHIVE_KEY`, then the stored key.
/// A stored key that cannot be read only costs a warning, as the commands that need no key still
/// work. The `key` commands choose for themselves.
pub(crate) fn init(cli: &Cli) -> Result<()> {
    RECOVER.store(cli.recover, Ordering::Relaxed);
    if matches!(cli.command, Command::Key(_)) {
        return Ok(());
    }
    let store = store(cli.key_file.as_deref());
    let variable = std::env::var(KEY_VARIABLE).ok();
    let chosen = match choose_key(
        cli.archive_key.as_ref().map(KeyText::as_str),
        variable.as_deref(),
        store.as_ref(),
    ) {
        Ok(chosen) => chosen,
        Err(KeyChoiceError::Profile(error)) => {
            let _ignored = writeln!(io::stderr().lock(), "warning: {error:#}; no key is used");
            None
        }
        Err(error) => return Err(InvalidKey::from(error).into()),
    };
    let _already = KEY.set(chosen.map(|chosen| chosen.key));
    Ok(())
}

/// Opens the Archive at `path` with the key of this run, if there is one, and notes on stderr
/// that it belongs to a test build.
pub(crate) fn open(path: impl AsRef<Path>) -> Result<Archive, ArchiveError> {
    crate::guard::on_archive_open(path.as_ref());
    let options = OpenOptions::new()
        .key(KEY.get().cloned().flatten())
        .recover(RECOVER.load(Ordering::Relaxed));
    let archive = Archive::open_with(path.as_ref(), options)?;
    crate::issues::warn_archive_wide(&archive);
    crate::test_build::announce(path.as_ref());
    Ok(archive)
}

/// Returns the line that tells how to give the key when `error` is about a missing or wrong key.
pub(crate) fn hint(error: &anyhow::Error) -> Option<&'static str> {
    let archive_error = archive_error(error)?;
    if archive_error.is_key_required() {
        Some(
            "give the Archive key with --key <hex> or the P4KX_ARCHIVE_KEY environment variable, \
             or store it once with `p4kx key set`",
        )
    } else if archive_error.is_wrong_key() {
        Some(
            "the Archive key does not fit this Archive; correct --key or P4KX_ARCHIVE_KEY, or \
             replace the stored key with `p4kx key set`",
        )
    } else {
        None
    }
}

/// Fails before anything is written when `entries` hold an encrypted Entry and `archive` has no
/// key, or one that does not fit, instead of failing every encrypted Entry on its own.
pub(crate) fn require(archive: &Archive, entries: &[&Entry]) -> Result<()> {
    let Some(encrypted) = entries.iter().find(|entry| entry.is_encrypted()) else {
        return Ok(());
    };
    if !archive.has_key() {
        return Err(ArchiveError::KeyRequired {
            path: archive.location().to_path_buf(),
            entry: encrypted.path().to_owned(),
        }
        .into());
    }
    archive.check_key()?;
    Ok(())
}

/// Returns whether `error` is about a missing or wrong Archive key.
pub(crate) fn is_key_error(error: &anyhow::Error) -> bool {
    archive_error(error).is_some_and(|error| error.is_key_required() || error.is_wrong_key())
}

fn archive_error(error: &anyhow::Error) -> Option<&ArchiveError> {
    match error.downcast_ref::<LineDiffError>() {
        Some(LineDiffError::Archive(found)) => Some(found),
        Some(_) => None,
        None => archive_error_in(error.as_ref()),
    }
}

/// Stores, shows or removes the Archive key of the profile.
pub(crate) fn run(args: &KeyArgs, given: &Given, as_json: bool) -> Result<()> {
    let store = store(given.file.as_deref())
        .context("this platform has no config directory; give --key-file")?;
    match &args.action {
        KeyAction::Set { key, archive } => set(
            &store,
            key.as_ref().map(KeyText::as_str),
            archive.as_deref(),
            as_json,
        ),
        KeyAction::Show => show(&store, given.key.as_ref().map(KeyText::as_str), as_json),
        KeyAction::Remove => remove(&store, as_json),
    }
}

fn store(key_file: Option<&Path>) -> Option<KeyStore> {
    key_file.map_or_else(KeyStore::in_profile, |file| Some(KeyStore::new(file)))
}

fn set(store: &KeyStore, text: Option<&str>, archive: Option<&Path>, as_json: bool) -> Result<()> {
    let line;
    let text = if let Some(text) = text {
        text
    } else {
        line = read_line()?;
        &line
    };
    let key = ArchiveKey::from_hex(text).map_err(InvalidKey::Text)?;
    let checked_against = check_against(&key, archive)?;
    store.save(&key)?;
    if as_json {
        json::print(&StoredJson {
            action: "stored",
            file: json::path(store.file()),
            checked_against: checked_against.as_deref().map(json::path),
        })?;
    } else {
        let note = checked_against.as_ref().map_or_else(
            || {
                "not checked: no Archive with an encrypted Entry was found, name one with \
                 --archive"
                    .to_owned()
            },
            |archive| format!("it decrypts an Entry of {}", archive.display()),
        );
        writeln!(
            io::stdout().lock(),
            "stored the Archive key in {} ({note})",
            store.file().display()
        )?;
    }
    Ok(())
}

/// Reads the key from the terminal without echoing it, or the first line of a piped stdin.
fn read_line() -> io::Result<String> {
    if io::stdin().is_terminal() {
        write!(io::stderr().lock(), "Archive key (hidden): ")?;
        return rpassword::read_password();
    }
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line)
}

fn check_against(key: &ArchiveKey, archive: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(path) = archive {
        let checked = open_unkeyed(path)?.verify_key(key)?;
        return Ok((checked == KeyCheck::Verified).then(|| path.to_path_buf()));
    }
    for path in installed_archives() {
        let Ok(archive) = open_unkeyed(&path) else {
            continue;
        };
        if archive.verify_key(key)? == KeyCheck::Verified {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn open_unkeyed(path: &Path) -> Result<Archive, ArchiveError> {
    Archive::open(path)
}

fn installed_archives() -> Vec<PathBuf> {
    let settings = default_settings_file()
        .and_then(|file| Settings::load(&file).ok())
        .unwrap_or_default();
    let standard = SearchPaths::standard();
    let paths = SearchPaths {
        default_library: standard.default_library,
        launcher: standard.launcher,
        wine_prefixes: standard.wine_prefixes,
        manual_folders: settings.channels.folders,
        manual_archives: settings.channels.archives,
    };
    detect(&paths)
        .iter()
        .map(|channel| channel.archive().to_path_buf())
        .collect()
}

fn show(store: &KeyStore, flag: Option<&str>, as_json: bool) -> Result<()> {
    let stored = store.load()?;
    let variable = std::env::var(KEY_VARIABLE).ok();
    let chosen = choose_key(flag, variable.as_deref(), Some(store)).map_err(InvalidKey::from)?;
    let stored_mask = stored.as_ref().map(ArchiveKey::masked);
    let in_use = chosen.as_ref().map(in_use);
    if as_json {
        json::print(&ShownJson {
            file: Some(json::path(store.file())),
            stored: stored_mask,
            in_use,
        })?;
        return Ok(());
    }
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "Stored: {} ({})",
        stored_mask.as_deref().unwrap_or("none"),
        store.file().display()
    )?;
    match in_use {
        Some(in_use) => writeln!(out, "In use: {} ({})", in_use.key, in_use.source)?,
        None => writeln!(out, "In use: none")?,
    }
    Ok(())
}

fn in_use(chosen: &ChosenKey) -> InUseJson {
    InUseJson {
        source: match chosen.source {
            KeySource::Flag => "flag",
            KeySource::Environment => "environment",
            KeySource::Profile => "profile",
        },
        key: chosen.key.masked(),
    }
}

fn remove(store: &KeyStore, as_json: bool) -> Result<()> {
    let removed = store.remove()?;
    if as_json {
        json::print(&RemovedJson {
            action: if removed { "removed" } else { "none_stored" },
            file: json::path(store.file()),
        })?;
    } else if removed {
        writeln!(
            io::stdout().lock(),
            "removed the Archive key from {}",
            store.file().display()
        )?;
    } else {
        writeln!(io::stdout().lock(), "no Archive key was stored")?;
    }
    Ok(())
}
