// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Wine and Proton prefixes that hold Star Citizen and the RSI Launcher on Linux.
//!
//! A prefix is a folder whose `drive_c` is the `C:` drive of the Windows environment the game
//! runs in.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::launcher_log::{self, LauncherLog, Wanted, launcher_logs_folder};

const DEFAULT_PREFIX: [&str; 2] = ["Games", "star-citizen"];
const DRIVE_C: &str = "drive_c";
const DOS_DEVICES: &str = "dosdevices";
const GAME_LIBRARY: [&str; 3] = ["Program Files", "Roberts Space Industries", "StarCitizen"];
const USERS: &str = "users";
const LAUNCHER_IN_USER: [&str; 3] = ["AppData", "Roaming", "rsilauncher"];

/// The environment that decides which Wine prefixes are candidates: the home folder and the
/// value of `WINEPREFIX`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WineEnvironment {
    home: Option<PathBuf>,
    wine_prefix: Option<OsString>,
}

impl WineEnvironment {
    /// Creates an environment from the home folder and the value of `WINEPREFIX`.
    #[must_use]
    pub const fn new(home: Option<PathBuf>, wine_prefix: Option<OsString>) -> Self {
        Self { home, wine_prefix }
    }

    /// Reads the environment of this process.
    #[must_use]
    pub fn from_process() -> Self {
        Self::new(dirs::home_dir(), std::env::var_os("WINEPREFIX"))
    }

    /// Returns the prefixes the game may live in, without checking that they exist: the one
    /// `WINEPREFIX` names, then `~/Games/star-citizen`.
    ///
    /// An empty or relative `WINEPREFIX` and a relative home folder are ignored; a prefix is
    /// listed once.
    #[must_use]
    pub fn candidate_prefixes(&self) -> Vec<PathBuf> {
        let from_variable = self
            .wine_prefix
            .as_ref()
            .map(PathBuf::from)
            .filter(|prefix| prefix.is_absolute());
        let default = self
            .home
            .as_ref()
            .filter(|home| home.is_absolute())
            .map(|home| {
                DEFAULT_PREFIX
                    .iter()
                    .fold(home.clone(), |path, part| path.join(part))
            });
        let mut prefixes: Vec<PathBuf> = Vec::new();
        for prefix in from_variable.into_iter().chain(default) {
            if !prefixes.contains(&prefix) {
                prefixes.push(prefix);
            }
        }
        prefixes
    }

    /// Returns the [candidates](Self::candidate_prefixes) that are folders; the others are
    /// skipped silently.
    #[must_use]
    pub fn existing_prefixes(&self) -> Vec<PathBuf> {
        let mut prefixes = self.candidate_prefixes();
        prefixes.retain(|prefix| prefix.is_dir());
        prefixes
    }
}

/// Returns the default Library folder inside `prefix`.
pub(crate) fn game_library(prefix: &Path) -> PathBuf {
    GAME_LIBRARY
        .iter()
        .fold(prefix.join(DRIVE_C), |path, part| path.join(part))
}

/// Returns the RSI Launcher's application-data folders inside `prefix`, one per Windows user
/// that has one, sorted.
fn launcher_folders(prefix: &Path) -> Vec<PathBuf> {
    let Ok(users) = std::fs::read_dir(prefix.join(DRIVE_C).join(USERS)) else {
        return Vec::new();
    };
    let mut folders: Vec<PathBuf> = users
        .filter_map(Result::ok)
        .map(|user| {
            LAUNCHER_IN_USER
                .iter()
                .fold(user.path(), |path, part| path.join(part))
        })
        .filter(|folder| folder.is_dir())
        .collect();
    folders.sort();
    folders
}

/// Reads the launcher logs of every Windows user in `prefix`; the library folders the logs name
/// are translated to paths on this machine.
pub(crate) fn scan_prefix(prefix: &Path, wanted: Wanted) -> LauncherLog {
    let mut log = LauncherLog::default();
    for launcher in launcher_folders(prefix) {
        let mut found = launcher_log::scan(&launcher_logs_folder(&launcher), wanted);
        found.libraries = found
            .libraries
            .iter()
            .filter_map(|library| host_path(prefix, library))
            .collect();
        log.absorb(found);
    }
    log
}

/// Translates a Windows path of the prefix to the path on this machine: `C:` is `drive_c`, any
/// other drive letter is the link `dosdevices/<letter>:`.
///
/// A path without a drive letter (a UNC or `\\?\` path, a relative or a host path) and one with a
/// `..` part give `None`.
fn host_path(prefix: &Path, path: &Path) -> Option<PathBuf> {
    let (drive, rest) = split_drive(path.to_str()?)?;
    let mut host = if drive == 'c' {
        prefix.join(DRIVE_C)
    } else {
        prefix.join(DOS_DEVICES).join(format!("{drive}:"))
    };
    for part in rest.split(['\\', '/']).filter(|part| !part.is_empty()) {
        match part {
            "." => {}
            ".." => return None,
            _ => host.push(part),
        }
    }
    Some(host)
}

fn split_drive(text: &str) -> Option<(char, &str)> {
    let mut chars = text.chars();
    let drive = chars.next().filter(char::is_ascii_alphabetic)?;
    (chars.next()? == ':').then(|| (drive.to_ascii_lowercase(), chars.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefix() -> PathBuf {
        PathBuf::from("prefix")
    }

    #[test]
    fn c_drive_paths_map_below_drive_c() {
        let host = host_path(&prefix(), Path::new(r"C:\Program Files\Game\LIVE"));
        let expected = prefix()
            .join("drive_c")
            .join("Program Files")
            .join("Game")
            .join("LIVE");
        assert_eq!(host, Some(expected));
    }

    #[test]
    fn other_drives_map_through_dosdevices() {
        let host = host_path(&prefix(), Path::new(r"D:\Games\SC"));
        let expected = prefix()
            .join("dosdevices")
            .join("d:")
            .join("Games")
            .join("SC");
        assert_eq!(host, Some(expected));
    }

    #[test]
    fn forward_slashes_and_lower_case_drives_map_too() {
        let host = host_path(&prefix(), Path::new("c:/Games//SC/./LIVE"));
        let expected = prefix()
            .join("drive_c")
            .join("Games")
            .join("SC")
            .join("LIVE");
        assert_eq!(host, Some(expected));
    }

    #[test]
    fn a_path_leaving_the_drive_is_refused() {
        assert_eq!(host_path(&prefix(), Path::new(r"C:\Games\..\..\etc")), None);
    }

    #[test]
    fn paths_without_a_drive_letter_are_refused() {
        for path in [
            "/data/games",
            "games/SC",
            r"\\server\share\SC",
            r"\\?\C:\Games\SC",
            r"\\?\UNC\server\share",
        ] {
            assert_eq!(host_path(&prefix(), Path::new(path)), None, "{path}");
        }
    }
}
