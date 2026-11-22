// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};
use std::sync::Arc;

use p4kx_assembly::ChildContainerPath;
use p4kx_extraction::{Archive, Entry};

pub(crate) const SYSTEM_FOLDER: &str = "data\\objectcontainers\\pu\\system\\";
pub(crate) const SOCPAK: &str = ".socpak";
const SYSTEM_SUFFIX: &str = "system.socpak";
const UNREFERENCED_SAMPLE: usize = 200;

/// The socpak Entries of an Archive by normalised path.
pub(crate) struct Index<'a> {
    pub(crate) entries: &'a [Entry],
    pub(crate) by_path: HashMap<String, usize>,
    duplicates: HashMap<String, Vec<usize>>,
}

impl<'a> Index<'a> {
    pub(crate) fn new(archive: &'a Archive) -> Self {
        let entries = archive.entries();
        let mut by_path = HashMap::new();
        let mut duplicates = HashMap::new();
        for (at, entry) in entries
            .iter()
            .enumerate()
            .filter(|(_, e)| is_socpak(e.path()))
        {
            let path = key(entry.path());
            if let Some(previous) = by_path.insert(path.clone(), at) {
                duplicates
                    .entry(path)
                    .or_insert_with(|| vec![previous])
                    .push(at);
            }
        }
        Self {
            entries,
            by_path,
            duplicates,
        }
    }

    #[cfg(test)]
    pub(crate) fn find(&self, path: &str) -> Option<usize> {
        match self.candidates(path) {
            [entry] => Some(*entry),
            _ => None,
        }
    }

    pub(crate) fn candidates(&self, path: &str) -> &[usize] {
        let Some((key, selected)) = self.selected_key(path) else {
            return &[];
        };
        self.candidates_for(key, selected)
    }

    /// Returns every physical Entry at a normalized root key without applying other lookup forms.
    pub(crate) fn exact_normalized_candidates(&self, normalized: &str) -> &[usize] {
        let Some(selected) = self.by_path.get(normalized) else {
            return &[];
        };
        self.candidates_for(normalized, selected)
    }

    fn candidates_for<'index>(&'index self, key: &str, selected: &'index usize) -> &'index [usize] {
        self.duplicates
            .get(key)
            .map_or_else(|| std::slice::from_ref(selected), Vec::as_slice)
    }

    fn selected_key(&self, path: &str) -> Option<(&String, &usize)> {
        let mut wanted = key(path);
        if !wanted.starts_with("data\\") {
            wanted.insert_str(0, "data\\");
        }
        self.by_path.get_key_value(&wanted).or_else(|| {
            ChildContainerPath::new(path)?
                .root_candidates()
                .find_map(|candidate| self.by_path.get_key_value(&key(&candidate)))
        })
    }

    /// Returns the folder name and the Entry of the main container of every star system, ordered
    /// by folder name: `<folder>system.socpak` of a folder below `ObjectContainers\PU\system`,
    /// else the first `*system.socpak` of the folder by path.
    pub(crate) fn systems(&self) -> Vec<(Arc<str>, usize)> {
        let mut folders: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
        for (path, at) in &self.by_path {
            if let Some((folder, file)) = system_path_parts(path)
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
                let (folder, _) = system_path_parts(path)?;
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

/// Splits a normalized system directory key into its folder and relative file.
pub(crate) fn system_path_parts(key: &str) -> Option<(&str, &str)> {
    key.strip_prefix(SYSTEM_FOLDER)?.split_once('\\')
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
    let mut sample = BinaryHeap::new();
    let mut total = 0;
    for (path, at) in &index.by_path {
        if !path.starts_with(SYSTEM_FOLDER) || opened.contains(at) {
            continue;
        }
        total += 1;
        let candidate = path.as_str();
        if sample.len() < UNREFERENCED_SAMPLE {
            sample.push(candidate);
        } else if let Some(mut greatest) = sample.peek_mut()
            && candidate < *greatest
        {
            *greatest = candidate;
        }
    }
    let mut paths: Vec<_> = sample
        .into_sorted_vec()
        .into_iter()
        .map(str::to_owned)
        .collect();
    paths.push(format!("{total} in total"));
    paths
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::{Index, SYSTEM_FOLDER, UNREFERENCED_SAMPLE, unreferenced};

    #[test]
    fn the_unreferenced_report_keeps_the_sorted_sample_and_full_count() {
        let count = UNREFERENCED_SAMPLE + 50;
        let visited = HashSet::from([1, UNREFERENCED_SAMPLE - 1, count - 1]);
        let path = |ordinal| format!("{SYSTEM_FOLDER}fixture\\item-{ordinal:04}.socpak");
        let mut by_path: HashMap<_, _> = (0..count)
            .rev()
            .map(|ordinal| (path(ordinal), ordinal))
            .collect();
        by_path.insert("data\\other\\excluded.socpak".to_owned(), count);
        let index = Index {
            entries: &[],
            by_path,
            duplicates: HashMap::new(),
        };
        let mut expected: Vec<_> = (0..count)
            .filter(|ordinal| !visited.contains(ordinal))
            .take(UNREFERENCED_SAMPLE)
            .map(path)
            .collect();
        expected.push(format!("{} in total", count - visited.len()));
        assert_eq!(
            unreferenced(&index, &visited),
            expected,
            "retain the smallest reported paths and count every eligible entry"
        );
    }
}
