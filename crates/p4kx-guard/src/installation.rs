// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Component, Path, PathBuf, Prefix};

use same_file::Handle;

use crate::WriteRefused;

/// A set of folders that belong to a Star Citizen installation and are never written to.
#[derive(Debug, Clone, Default)]
pub struct Installation {
    folders: Vec<Protected>,
}

#[derive(Debug, Clone)]
struct Protected {
    shown: PathBuf,
    key: Vec<String>,
}

impl Installation {
    /// Creates an empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            folders: Vec::new(),
        }
    }

    /// Adds `folder` and everything below it; a folder already covered is not added twice.
    pub fn add(&mut self, folder: &Path) {
        let resolved = resolve(folder);
        let key = key(&resolved);
        if key.is_empty() || self.folders.iter().any(|known| known.key == key) {
            return;
        }
        self.folders.push(Protected {
            shown: resolved,
            key,
        });
    }

    /// Returns whether no folder is protected.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.folders.is_empty()
    }

    /// Checks that writing to `target`, a file or folder that may not exist yet, stays outside
    /// every protected folder.
    ///
    /// The comparison is canonical: `.` and `..`, symbolic links and junctions, the `\\?\` form
    /// and, on Windows and macOS, letter case do not change the outcome. A path that reaches the
    /// same folder by another route, such as a loopback share `\\localhost\D$\...`, is caught by
    /// the identity of the folder (volume and file index) of the target's existing ancestors. For
    /// a path that does not exist yet, its nearest existing ancestor decides.
    ///
    /// # Errors
    ///
    /// Returns [`WriteRefused`] naming the protected folder `target` lies in.
    pub fn check(&self, target: &Path) -> Result<(), WriteRefused> {
        if self.folders.is_empty() {
            return Ok(());
        }
        let resolved = resolve(target);
        let target_key = key(&resolved);
        let refused = |folder: &Protected| WriteRefused {
            target: target.to_path_buf(),
            installation: without_verbatim_prefix(&folder.shown),
        };
        if let Some(folder) = self
            .folders
            .iter()
            .find(|folder| target_key.starts_with(&folder.key))
        {
            return Err(refused(folder));
        }
        let ancestors: Vec<Handle> = resolved
            .ancestors()
            .filter_map(|ancestor| Handle::from_path(ancestor).ok())
            .collect();
        self.folders
            .iter()
            .find(|folder| {
                Handle::from_path(&folder.shown).is_ok_and(|handle| ancestors.contains(&handle))
            })
            .map_or(Ok(()), |folder| Err(refused(folder)))
    }

    pub(crate) fn check_namespace(&self, target: &Path) -> Result<(), WriteRefused> {
        let Some(parent) = target.parent() else {
            return self.check(target);
        };
        self.check(parent)?;
        let target_key = key(target);
        self.folders
            .iter()
            .find(|folder| target_key.starts_with(&folder.key))
            .map_or(Ok(()), |folder| {
                Err(WriteRefused {
                    target: target.to_path_buf(),
                    installation: without_verbatim_prefix(&folder.shown),
                })
            })
    }
}

/// Returns `path` spelled the way a person writes it: `\\?\C:\x` as `C:\x` and `\\?\UNC\s\x` as
/// `\\s\x`. Any other path is returned unchanged.
#[must_use]
pub fn without_verbatim_prefix(path: &Path) -> PathBuf {
    let text = path.as_os_str().to_string_lossy();
    if let Some(share) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{share}"));
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

/// Makes `path` absolute and resolves `.`, `..` and every link that exists.
pub(crate) fn resolve(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut resolved = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => resolved.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(name) => {
                resolved.push(name);
                if let Ok(real) = fs::canonicalize(&resolved) {
                    resolved = real;
                }
            }
        }
    }
    resolved
}

fn key(resolved: &Path) -> Vec<String> {
    resolved
        .components()
        .filter_map(|component| match component {
            Component::Prefix(prefix) => Some(prefix_key(prefix.kind())),
            Component::Normal(name) => Some(fold(&name.to_string_lossy())),
            Component::RootDir | Component::CurDir | Component::ParentDir => None,
        })
        .collect()
}

fn prefix_key(prefix: Prefix<'_>) -> String {
    match prefix {
        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
            format!("{}:", char::from(letter.to_ascii_lowercase()))
        }
        Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => fold(&format!(
            "\\\\{}\\{}",
            server.to_string_lossy(),
            share.to_string_lossy()
        )),
        Prefix::Verbatim(name) | Prefix::DeviceNS(name) => fold(&name.to_string_lossy()),
    }
}

fn fold(name: &str) -> String {
    if cfg!(any(windows, target_os = "macos")) {
        name.to_lowercase()
    } else {
        name.to_owned()
    }
}
