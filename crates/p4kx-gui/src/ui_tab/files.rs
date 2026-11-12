// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::EntryIndex;

use super::job::Job;

/// How looking the UI files up ended.
#[derive(Debug)]
pub(super) enum Outcome {
    Ready(Vec<String>),
    ArchiveChanged,
}

/// Returns whether the Entry at `path` is a GFx or SWF movie.
fn is_movie_path(path: &str) -> bool {
    let extension = path.rsplit_once('.').map(|(_, extension)| extension);
    extension.is_some_and(|extension| {
        extension.eq_ignore_ascii_case("gfx") || extension.eq_ignore_ascii_case("swf")
    })
}

/// Looks up the GFx and SWF Entries of `index` on a background thread.
pub(super) fn start(
    index: &Arc<EntryIndex>,
    wake: impl Fn() + Send + 'static,
) -> Result<Job<Outcome>, String> {
    let index = Arc::clone(index);
    Job::spawn("ui-files", wake, move || {
        if index.watch().check().is_err() {
            return Outcome::ArchiveChanged;
        }
        let mut paths: Vec<String> = index
            .archive()
            .map_or_default(p4kx_extraction::Archive::entries)
            .iter()
            .map(p4kx_extraction::Entry::path)
            .filter(|path| is_movie_path(path))
            .map(str::to_owned)
            .collect();
        paths.sort_unstable_by_key(|path| path.to_ascii_lowercase());
        if index.watch().check().is_err() {
            return Outcome::ArchiveChanged;
        }
        Outcome::Ready(paths)
    })
}

/// The GFx and SWF Entries of the Archive that the search text leaves.
#[derive(Debug, Clone)]
pub struct FileList {
    paths: Vec<String>,
    shown: Vec<u32>,
}

impl FileList {
    pub(super) fn new(paths: Vec<String>) -> Self {
        let mut list = Self {
            paths,
            shown: Vec::new(),
        };
        list.refilter("");
        list
    }

    pub(super) fn refilter(&mut self, query: &str) {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(str::to_ascii_lowercase)
            .collect();
        self.shown = self
            .paths
            .iter()
            .enumerate()
            .filter(|(_, path)| {
                let lower = path.to_ascii_lowercase();
                terms.iter().all(|term| lower.contains(term.as_str()))
            })
            .filter_map(|(position, _)| u32::try_from(position).ok())
            .collect();
    }

    /// Returns how many files the Archive holds.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.paths.len()
    }

    /// Returns how many files the search leaves.
    #[must_use]
    pub const fn shown_len(&self) -> usize {
        self.shown.len()
    }

    /// Returns the Entry path at `position` of the shown files.
    #[must_use]
    pub fn get(&self, position: usize) -> Option<&str> {
        let at = usize::try_from(*self.shown.get(position)?).ok()?;
        self.paths.get(at).map(String::as_str)
    }
}
