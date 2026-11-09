// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(crate) struct OrderedLibraries {
    paths: HashMap<PathBuf, usize>,
}

impl OrderedLibraries {
    pub(crate) fn contains(&self, path: &Path) -> bool {
        self.paths.contains_key(path)
    }

    pub(crate) fn insert(&mut self, path: PathBuf) {
        let ordinal = self.paths.len();
        self.paths.entry(path).or_insert(ordinal);
    }

    pub(crate) fn finish(self) -> Vec<PathBuf> {
        let mut paths: Vec<_> = self.paths.into_iter().collect();
        paths.sort_unstable_by_key(|(_, ordinal)| *ordinal);
        paths.into_iter().map(|(path, _)| path).collect()
    }
}
