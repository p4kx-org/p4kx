// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_settings::{FolderKind, base_folder};

use crate::PublicPatches;
use crate::log_lines::{PATCH_LINE_BYTES, Part, for_each_line};

/// The only files read from the RSI Launcher's log folder, the newest first.
pub const LAUNCHER_LOG_FILES: [&str; 2] = ["log.log", "log.old.log"];

const LAUNCHER_FOLDER: &str = "rsilauncher";
const LOGS_FOLDER: &str = "logs";
const LOGS_OVERRIDE: &str = "P4KX_LAUNCHER_LOGS";

const LAUNCH: &str = "Launching Star Citizen ";
const LAUNCH_FROM: &str = " from (";
const LAUNCH_END: &str = ")\"";
const INSTALLERS: [&str; 2] = ["Installing Star Citizen ", "Verifying Star Citizen "];
const INSTALL_AT: &str = " at ";
const INSTALL_END: &str = " (force DP";

/// What a scan of the log files collects.
#[derive(Clone, Copy)]
pub(crate) struct Wanted {
    libraries: bool,
    patches: bool,
}

impl Wanted {
    pub(crate) const LIBRARIES: Self = Self {
        libraries: true,
        patches: false,
    };
    pub(crate) const PATCHES: Self = Self {
        libraries: false,
        patches: true,
    };
    pub(crate) const BOTH: Self = Self {
        libraries: true,
        patches: true,
    };
}

/// What the RSI Launcher's log files held, as far as [`Wanted`] asked for it.
#[derive(Default)]
pub(crate) struct LauncherLog {
    pub(crate) libraries: Vec<PathBuf>,
    pub(crate) patches: PublicPatches,
}

impl LauncherLog {
    /// Adds the library folders of `newer` that are not known yet and lets its patches win over
    /// the known ones by order of absorption, not by the age of their log lines.
    pub(crate) fn absorb(&mut self, newer: Self) {
        for library in newer.libraries {
            if !self.libraries.contains(&library) {
                self.libraries.push(library);
            }
        }
        self.patches.absorb_newer(newer.patches);
        self.patches.trim();
    }
}

/// Returns the folder of the launcher's logs inside the launcher's application-data folder.
#[must_use]
pub fn launcher_logs_folder(launcher: &Path) -> PathBuf {
    launcher.join(LOGS_FOLDER)
}

/// Returns the application-data folder of the RSI Launcher on this machine.
#[must_use]
pub(crate) fn launcher_folder() -> Option<PathBuf> {
    base_folder(FolderKind::Config).map(|dir| dir.join(LAUNCHER_FOLDER))
}

/// Returns the folder of the launcher's logs: `P4KX_LAUNCHER_LOGS` when set, otherwise the
/// `logs` folder of the launcher's application-data folder.
#[must_use]
pub(crate) fn standard_log_folder() -> Option<PathBuf> {
    std::env::var_os(LOGS_OVERRIDE)
        .map(PathBuf::from)
        .or_else(|| launcher_folder().map(|launcher| launcher_logs_folder(&launcher)))
}

/// Reads the files of [`LAUNCHER_LOG_FILES`] in `log_folder` once and collects what `wanted`
/// names: the library folders without duplicates, and the public patches, the newest line
/// winning.
///
/// A missing or unreadable file is skipped.
#[must_use]
pub(crate) fn scan(log_folder: &Path, wanted: Wanted) -> LauncherLog {
    let mut log = LauncherLog::default();
    let part = if wanted.libraries {
        Part::Whole
    } else {
        Part::Tail
    };
    let mut per_file = Vec::new();
    for name in LAUNCHER_LOG_FILES {
        let mut file_patches = PublicPatches::default();
        for_each_line(&log_folder.join(name), part, |in_tail, line| {
            let text = String::from_utf8_lossy(line);
            if wanted.libraries
                && let Some(folder) = library_folder(&text)
                && !log.libraries.contains(&folder)
            {
                log.libraries.push(folder);
            }
            if wanted.patches && in_tail && line.len() <= PATCH_LINE_BYTES {
                file_patches.note_line(&text);
            }
        });
        per_file.push(file_patches);
    }
    for file_patches in per_file.into_iter().rev() {
        log.patches.absorb_newer(file_patches);
    }
    log.patches.trim();
    log
}

fn library_folder(line: &str) -> Option<PathBuf> {
    if let Some(channel_folder) = launched_channel_folder(line) {
        return parent_folder(&channel_folder);
    }
    INSTALLERS.iter().find_map(|marker| {
        let rest = after(line, marker)?;
        let rest = after(rest, INSTALL_AT)?;
        let end = rest.find(INSTALL_END)?;
        unescape(rest.get(..end)?).map(PathBuf::from)
    })
}

/// Returns the parent of `folder`, also for a Windows path that a non-Windows host sees as a
/// single file name (a launcher log inside a Wine prefix).
fn parent_folder(folder: &Path) -> Option<PathBuf> {
    if let Some(parent) = folder.parent()
        && !parent.as_os_str().is_empty()
    {
        return Some(parent.to_path_buf());
    }
    let (parent, _) = folder.to_str()?.rsplit_once(['\\', '/'])?;
    (!parent.is_empty()).then(|| PathBuf::from(parent))
}

fn launched_channel_folder(line: &str) -> Option<PathBuf> {
    let rest = after(line, LAUNCH)?;
    let rest = after(rest, LAUNCH_FROM)?;
    let end = rest.find(LAUNCH_END)?;
    unescape(rest.get(..end)?).map(PathBuf::from)
}

fn after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let start = text.find(marker)? + marker.len();
    text.get(start..)
}

fn unescape(json_fragment: &str) -> Option<String> {
    let path: String = serde_json::from_str(&format!("\"{json_fragment}\"")).ok()?;
    (!path.is_empty()).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parent_of_a_windows_path_is_found_on_every_host() {
        assert_eq!(
            parent_folder(Path::new(r"C:\Games\SC\LIVE")),
            Some(PathBuf::from(r"C:\Games\SC"))
        );
    }

    #[test]
    fn a_name_without_a_separator_has_no_parent() {
        assert_eq!(parent_folder(Path::new("LIVE")), None);
    }
}
