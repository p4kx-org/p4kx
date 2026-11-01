// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use crate::launcher_log::{self, LauncherLog, Wanted, launcher_folder, launcher_logs_folder};
use crate::wine::{self, WineEnvironment};
use crate::{Channel, ChannelSource, PublicPatches};

const ARCHIVE_NAME: &str = "Data.p4k";
const DEFAULT_LIBRARY: Option<&str> = if cfg!(windows) {
    Some(r"C:\Program Files\Roberts Space Industries\StarCitizen")
} else {
    None
};

/// The places [`detect`] looks for Channels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchPaths {
    /// The default library folder, `C:\Program Files\Roberts Space Industries\StarCitizen` on
    /// Windows.
    pub default_library: Option<PathBuf>,
    /// The RSI Launcher's application-data folder, whose `logs` folder names library folders.
    pub launcher: Option<PathBuf>,
    /// Wine prefixes that may hold the game and the launcher; a missing one is skipped.
    pub wine_prefixes: Vec<PathBuf>,
    /// Manually added folders, each a Channel folder or a library folder.
    pub manual_folders: Vec<PathBuf>,
    /// Manually added Archives.
    pub manual_archives: Vec<PathBuf>,
}

impl SearchPaths {
    /// Returns the default library folder, the launcher folder and, on Unix, the existing Wine
    /// prefixes of this machine, without manual entries.
    #[must_use]
    pub fn standard() -> Self {
        Self {
            default_library: DEFAULT_LIBRARY.map(PathBuf::from),
            launcher: launcher_folder(),
            wine_prefixes: if cfg!(unix) {
                WineEnvironment::from_process().existing_prefixes()
            } else {
                Vec::new()
            },
            manual_folders: Vec::new(),
            manual_archives: Vec::new(),
        }
    }
}

/// Returns the Channels found in `paths`, each Archive once, in the order default library, the
/// libraries of the Wine prefixes, launcher library folders, manual folders, manual Archives.
///
/// A Channel folder is a folder holding a `Data.p4k`; a library folder holds Channel folders.
/// Only directory listings, `build_manifest.id` files and the launcher's log files are read; the
/// log files are read once and also give each Channel's Build its public patch.
#[must_use]
pub fn detect(paths: &SearchPaths) -> Vec<Channel> {
    detect_with_patches(paths).0
}

/// Returns the Channels like [`detect`] and the public patches the launcher log names, read in
/// the same pass, for whoever needs the public patch of a Build outside the Channels.
#[must_use]
pub fn detect_with_patches(paths: &SearchPaths) -> (Vec<Channel>, PublicPatches) {
    let log = scan_launchers(paths, Wanted::BOTH);
    let mut found = Found::new(&log.patches);
    if let Some(library) = &paths.default_library {
        found.library(library, ChannelSource::DefaultPath);
    }
    for prefix in &paths.wine_prefixes {
        found.library(&wine::game_library(prefix), ChannelSource::DefaultPath);
    }
    for library in &log.libraries {
        found.library(library, ChannelSource::Launcher);
    }
    for folder in &paths.manual_folders {
        let archive = folder.join(ARCHIVE_NAME);
        if archive.is_file() {
            found.archive(archive, ChannelSource::Manual);
        } else {
            found.library(folder, ChannelSource::Manual);
        }
    }
    for archive in &paths.manual_archives {
        if archive.is_file() {
            found.archive(archive.clone(), ChannelSource::Manual);
        }
    }
    (found.channels, log.patches)
}

/// Returns the Library folders known from `paths`: the default one, those of the Wine prefixes
/// and those the RSI Launcher logs name, whether or not they exist.
///
/// Only the launcher's log files are read.
#[must_use]
pub fn known_library_folders(paths: &SearchPaths) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = paths.default_library.iter().cloned().collect();
    let prefix_libraries = paths
        .wine_prefixes
        .iter()
        .map(|prefix| wine::game_library(prefix));
    let logged = scan_launchers(paths, Wanted::LIBRARIES).libraries;
    for library in prefix_libraries.chain(logged) {
        if !folders.contains(&library) {
            folders.push(library);
        }
    }
    folders
}

/// Reads the logs of the launcher folder and of the launchers inside the Wine prefixes.
fn scan_launchers(paths: &SearchPaths, wanted: Wanted) -> LauncherLog {
    let mut log = paths
        .launcher
        .as_ref()
        .map_or_else(LauncherLog::default, |launcher| {
            launcher_log::scan(&launcher_logs_folder(launcher), wanted)
        });
    for prefix in &paths.wine_prefixes {
        log.absorb(wine::scan_prefix(prefix, wanted));
    }
    log
}

struct Found<'a> {
    channels: Vec<Channel>,
    keys: Vec<PathBuf>,
    patches: &'a PublicPatches,
}

impl<'a> Found<'a> {
    const fn new(patches: &'a PublicPatches) -> Self {
        Self {
            channels: Vec::new(),
            keys: Vec::new(),
            patches,
        }
    }

    fn library(&mut self, library: &Path, source: ChannelSource) {
        let Ok(items) = std::fs::read_dir(library) else {
            return;
        };
        let mut archives: Vec<PathBuf> = items
            .filter_map(Result::ok)
            .map(|item| item.path().join(ARCHIVE_NAME))
            .filter(|archive| archive.is_file())
            .collect();
        archives.sort();
        for archive in archives {
            self.archive(archive, source);
        }
    }

    fn archive(&mut self, archive: PathBuf, source: ChannelSource) {
        let key = std::fs::canonicalize(&archive).unwrap_or_else(|_| archive.clone());
        if !self.keys.contains(&key) {
            self.keys.push(key);
            self.channels
                .push(Channel::new(archive, source, self.patches));
        }
    }
}
