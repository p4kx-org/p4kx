// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use p4kx_assembly::ChildContainerPath;
use p4kx_extraction::{Archive, Entry};

pub(crate) const SYSTEM_FOLDER: &str = "data\\objectcontainers\\pu\\system\\";
pub(crate) const SOCPAK: &str = ".socpak";
const SYSTEM_SUFFIX: &str = "system.socpak";

/// The socpak Entries of an Archive by normalised path.
pub(crate) struct Index<'a> {
    pub(crate) entries: &'a [Entry],
    pub(crate) by_path: HashMap<String, usize>,
}

impl<'a> Index<'a> {
    pub(crate) fn new(archive: &'a Archive) -> Self {
        let entries = archive.entries();
        let by_path = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| is_socpak(entry.path()))
            .map(|(at, entry)| (key(entry.path()), at))
            .collect();
        Self { entries, by_path }
    }

    pub(crate) fn find(&self, path: &str) -> Option<usize> {
        let mut wanted = key(path);
        if !wanted.starts_with("data\\") {
            wanted.insert_str(0, "data\\");
        }
        self.by_path.get(&wanted).copied().or_else(|| {
            ChildContainerPath::new(path)?
                .root_candidates()
                .find_map(|candidate| self.by_path.get(&key(&candidate)).copied())
        })
    }

    /// Returns the folder name and the Entry of the main container of every star system, ordered
    /// by folder name: `<folder>system.socpak` of a folder below `ObjectContainers\PU\system`,
    /// else the first `*system.socpak` of the folder by path.
    pub(crate) fn systems(&self) -> Vec<(Arc<str>, usize)> {
        let mut folders: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
        for (path, at) in &self.by_path {
            let Some(rest) = path.strip_prefix(SYSTEM_FOLDER) else {
                continue;
            };
            if let Some((folder, file)) = rest.split_once('\\')
                && !file.contains('\\')
                && file.ends_with(SYSTEM_SUFFIX)
            {
                folders.entry(folder).or_default().insert(file, *at);
            }
        }
        folders
            .into_iter()
            .filter_map(|(folder, files)| {
                let own = format!("{folder}{SYSTEM_SUFFIX}");
                let at = files
                    .get(own.as_str())
                    .or_else(|| files.values().next())
                    .copied()?;
                Some((Arc::from(folder), at))
            })
            .collect()
    }

    /// Returns the star system folder and the Entry of every container below
    /// `ObjectContainers\PU\system`, ordered by path.
    pub(crate) fn system_containers(&self) -> Vec<(Arc<str>, usize)> {
        let mut found: Vec<(&str, &str, usize)> = self
            .by_path
            .iter()
            .filter_map(|(path, at)| {
                let rest = path.strip_prefix(SYSTEM_FOLDER)?;
                let (folder, _) = rest.split_once('\\')?;
                Some((path.as_str(), folder, *at))
            })
            .collect();
        found.sort_unstable();
        found
            .into_iter()
            .map(|(_, folder, at)| (Arc::from(folder), at))
            .collect()
    }
}

/// Returns whether `path` ends in `.socpak`, ignoring ASCII case.
fn is_socpak(path: &str) -> bool {
    path.len()
        .checked_sub(SOCPAK.len())
        .and_then(|from| path.as_bytes().get(from..))
        .is_some_and(|tail| tail.eq_ignore_ascii_case(SOCPAK.as_bytes()))
}

/// Returns `path` in lower case with `\` as separator and no leading separator.
pub(crate) fn key(path: &str) -> String {
    path.trim_start_matches(['/', '\\'])
        .chars()
        .map(|c| {
            if c == '/' {
                '\\'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

/// Returns the system containers that were not opened, up to 200, and the total.
pub(crate) fn unreferenced(index: &Index<'_>, opened: &HashSet<usize>) -> Vec<String> {
    let mut paths: Vec<String> = index
        .by_path
        .iter()
        .filter(|(path, at)| path.starts_with(SYSTEM_FOLDER) && !opened.contains(at))
        .map(|(path, _)| path.clone())
        .collect();
    paths.sort_unstable();
    let total = paths.len();
    paths.truncate(200);
    paths.push(format!("{total} in total"));
    paths
}
