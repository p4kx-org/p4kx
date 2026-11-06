// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once, PoisonError};
use std::thread::{self, JoinHandle};

use p4kx_extraction::{
    ConversionError, ExtractionError, QueryExportError, WriteRefused, game_is_running,
    protect_archive_folder, protect_libraries, protect_manual,
};
use p4kx_settings::{Settings, default_settings_file};

const CONFIG_DIR_VARIABLE: &str = "P4KX_CONFIG_DIR";

const GAME_CHECK_VARIABLE: &str = "P4KX_GAME_CHECK";

static WARNED: Once = Once::new();
static CHECK: Mutex<Option<JoinHandle<bool>>> = Mutex::new(None);

/// How the look for the running game is done, chosen with `P4KX_GAME_CHECK`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum GameCheck {
    Process,
    Off,
    Running,
    Closed,
}

/// Reads `P4KX_GAME_CHECK`: `off` skips the look, `running` and `closed` stand in for the process
/// list so that tests do not depend on the machine; anything else looks at the process list.
fn game_check() -> GameCheck {
    match std::env::var(GAME_CHECK_VARIABLE) {
        Ok(value) if value.eq_ignore_ascii_case("off") => GameCheck::Off,
        Ok(value) if value.eq_ignore_ascii_case("running") => GameCheck::Running,
        Ok(value) if value.eq_ignore_ascii_case("closed") => GameCheck::Closed,
        _ => GameCheck::Process,
    }
}

fn look_for_game() -> bool {
    match game_check() {
        GameCheck::Process => game_is_running(),
        GameCheck::Running => true,
        GameCheck::Off | GameCheck::Closed => false,
    }
}

/// Protects the Library folders and the folders and Archives added by hand to the settings, which
/// `P4KX_CONFIG_DIR` can point to another folder.
pub(crate) fn protect_installations() {
    protect_libraries();
    let file = std::env::var_os(CONFIG_DIR_VARIABLE)
        .map(|dir| PathBuf::from(dir).join("settings.toml"))
        .or_else(default_settings_file);
    if let Some(settings) = file.and_then(|file| Settings::load(&file).ok()) {
        protect_manual(&settings.channels.folders, &settings.channels.archives);
    }
}

/// Starts looking for the running game in the background, so that the first Archive opened does
/// not wait for it.
pub(crate) fn start_game_check() {
    if game_check() == GameCheck::Off {
        return;
    }
    let spawned = thread::Builder::new()
        .name("game-check".to_owned())
        .spawn(look_for_game);
    if let Ok(handle) = spawned {
        *CHECK.lock().unwrap_or_else(PoisonError::into_inner) = Some(handle);
    }
}

/// Protects the installation around the Archive at `path` and warns once per run when the game is
/// running.
pub(crate) fn on_archive_open(path: &Path) {
    protect_archive_folder(path);
    if game_check() == GameCheck::Off {
        return;
    }
    WARNED.call_once(|| {
        let started = CHECK.lock().unwrap_or_else(PoisonError::into_inner).take();
        let running = started.map_or_else(look_for_game, |handle| handle.join().unwrap_or(false));
        if running {
            let _ignored = writeln!(
                io::stderr().lock(),
                "warning: StarCitizen.exe may be running; use this tool with the game closed"
            );
        }
    });
}

/// Returns whether `error` is a write that was refused because the target lies inside a Star
/// Citizen installation.
pub(crate) fn is_refusal(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<WriteRefused>().is_some()
            || matches!(
                cause.downcast_ref::<ConversionError>(),
                Some(ConversionError::Installation(_))
            )
            || matches!(
                cause.downcast_ref::<p4kx_blender::PackageError>(),
                Some(p4kx_blender::PackageError::Refused(_))
            )
            || matches!(
                cause.downcast_ref::<ExtractionError>(),
                Some(ExtractionError::Installation(_))
            )
            || matches!(
                cause.downcast_ref::<QueryExportError>(),
                Some(QueryExportError::Installation(_))
            )
            || cause
                .downcast_ref::<io::Error>()
                .and_then(io::Error::get_ref)
                .and_then(|inner| inner.downcast_ref::<WriteRefused>())
                .is_some()
    })
}

/// Refuses `target`, when there is one, when it lies inside a Star Citizen installation.
pub(crate) fn ensure_writable_if(target: Option<&Path>) -> io::Result<()> {
    target.map_or(Ok(()), ensure_writable)
}

/// Refuses `target` when it lies inside a Star Citizen installation.
pub(crate) fn ensure_writable(target: &Path) -> io::Result<()> {
    p4kx_extraction::ensure_writable(target)
        .map_err(|refused| io::Error::new(io::ErrorKind::PermissionDenied, refused))
}
