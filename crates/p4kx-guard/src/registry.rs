// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError, RwLock};
use std::time::SystemTime;

use p4kx_channels::{LAUNCHER_LOG_FILES, SearchPaths, known_library_folders, launcher_logs_folder};

use crate::{Installation, WriteProtectionError, WriteRefused};

const GAME_FOLDER: &str = "Bin64";
const EXTRA_FOLDERS_VARIABLE: &str = "P4KX_PROTECT_FOLDERS";

static PROTECTED: RwLock<Installation> = RwLock::new(Installation::new());
static LOG_STAMP: Mutex<Option<LogStamp>> = Mutex::new(None);

/// Protects `folder` and everything below it for the rest of the process.
pub fn protect(folder: &Path) {
    PROTECTED
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .add(folder);
}

/// Protects the Library folders of this machine: the default one and those the RSI Launcher log
/// names. Call it once at program start, before any file is written.
///
/// The folders in the path list of the environment variable `P4KX_PROTECT_FOLDERS` are protected
/// as well. It is a test aid for fake installations and can only add protection.
pub fn protect_libraries() {
    let paths = SearchPaths::standard();
    let stamp = log_stamp(&paths);
    let mut seen = LOG_STAMP.lock().unwrap_or_else(PoisonError::into_inner);
    if seen.as_ref() != Some(&stamp) {
        for library in known_library_folders(&paths) {
            protect(&library);
        }
        *seen = Some(stamp);
    }
    drop(seen);
    if let Some(extra) = std::env::var_os(EXTRA_FOLDERS_VARIABLE) {
        for folder in std::env::split_paths(&extra).filter(|folder| !folder.as_os_str().is_empty())
        {
            protect(&folder);
        }
    }
}

/// Protects the Library folders of this machine and, when it holds a game installation, the
/// folder of the Archive at `archive`.
///
/// The Archive's folder is a game installation when it holds the game's `Bin64` folder; a copy of
/// an Archive elsewhere is not one. The launcher logs are read again only when their size or
/// modification time changed.
pub fn protect_archive_folder(archive: &Path) {
    protect_libraries();
    protect_installation_of(archive);
}

/// Protects the folders the user entered by hand that are an installation, and the folder of each
/// Archive entered by hand that is one.
///
/// A folder is an installation when it holds the game's `Bin64` folder itself (a Channel folder)
/// or has a child folder that does (a Library folder); any other folder stays writable. Call it
/// with the manual entries of the settings whenever they are loaded or extended.
pub fn protect_manual(folders: &[PathBuf], archives: &[PathBuf]) {
    for folder in folders {
        if is_installation(folder) {
            protect(folder);
        }
    }
    for archive in archives {
        protect_installation_of(archive);
    }
}

fn is_installation(folder: &Path) -> bool {
    folder.join(GAME_FOLDER).is_dir()
        || std::fs::read_dir(folder).is_ok_and(|items| {
            items
                .filter_map(Result::ok)
                .any(|item| item.path().join(GAME_FOLDER).is_dir())
        })
}

type LogStamp = Vec<Option<(SystemTime, u64)>>;

fn log_stamp(paths: &SearchPaths) -> LogStamp {
    LAUNCHER_LOG_FILES
        .iter()
        .map(|name| {
            let logs = launcher_logs_folder(paths.launcher.as_ref()?).join(name);
            let metadata = std::fs::metadata(logs).ok()?;
            Some((metadata.modified().ok()?, metadata.len()))
        })
        .collect()
}

fn protect_installation_of(archive: &Path) {
    if let Some(folder) = std::path::absolute(archive)
        .ok()
        .and_then(|absolute| absolute.parent().map(Path::to_path_buf))
        && folder.join(GAME_FOLDER).is_dir()
    {
        protect(&folder);
    }
}

/// Checks that writing to `target` stays outside the protected folders.
///
/// # Errors
///
/// Returns an established installation refusal or unavailable target identity.
pub fn ensure_writable(target: &Path) -> Result<(), WriteProtectionError> {
    let host =
        crate::write_target::host_target(target, &std::sync::atomic::AtomicBool::new(false))?;
    check_known(&host).map_err(Into::into)
}

pub(crate) fn check_known(target: &Path) -> Result<(), WriteRefused> {
    PROTECTED
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .check(target)
}

pub(crate) fn check_known_namespace(target: &Path) -> Result<(), WriteRefused> {
    PROTECTED
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .check_namespace(target)
}
