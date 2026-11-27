// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use crate::ExtractionError;
use crate::extract::{Ended, EntryEnded};
use crate::file_receipt;
use crate::portable_name::{self, fold};
use crate::source::SourceEntry;
use crate::write::{EntryTarget, EntryWriter, Outcome};

/// The files an Extraction has put in place, by their path folded to lower case, so an Entry whose
/// path differs only in case from a written one is found whatever file system the target is on.
///
/// A file also collides with a written file where it needs a folder, and with a written folder
/// where it is a file. The Entries that can collide run one after the other in the order of the
/// run, after all others, so the Entry named as the earlier one is always the one written.
#[derive(Debug, Default)]
pub(crate) struct WrittenPaths {
    paths: Mutex<Paths>,
}

#[derive(Debug, Default)]
struct Paths {
    files: HashMap<String, usize>,
    folders: HashMap<String, usize>,
}

impl WrittenPaths {
    /// Returns the Archive index of the Entry that wrote a file colliding with `relative`, the
    /// path of the Entry with Archive index `index`, unless that is the Entry itself.
    pub(crate) fn conflict(&self, relative: &Path, index: usize) -> Option<usize> {
        self.conflict_key(&key(relative), index)
    }

    fn conflict_key(&self, key: &str, index: usize) -> Option<usize> {
        let paths = self.paths.lock().unwrap_or_else(PoisonError::into_inner);
        let other = |holder: Option<&usize>| holder.copied().filter(|holder| *holder != index);
        other(paths.files.get(key))
            .or_else(|| other(paths.folders.get(key)))
            .or_else(|| parents(key).find_map(|parent| other(paths.files.get(parent))))
    }

    /// Records that the Entry with Archive index `index` put the file `relative` in place.
    pub(crate) fn record(&self, relative: &Path, index: usize) {
        let key = key(relative);
        let mut paths = self.paths.lock().unwrap_or_else(PoisonError::into_inner);
        for parent in parents(&key) {
            paths.folders.entry(parent.to_owned()).or_insert(index);
        }
        paths.files.entry(key).or_insert(index);
    }
}

fn key(relative: &Path) -> String {
    fold(&relative.to_string_lossy()).collect()
}

/// Returns the folders of a folded path, the outermost first.
fn parents(key: &str) -> impl Iterator<Item = &str> {
    key.match_indices('/').filter_map(|(at, _)| key.get(..at))
}

impl EntryWriter<'_> {
    /// Reserves selected native names that can overlap a CryXml companion.
    pub(crate) fn reserve_cryxml_native_paths(
        &self,
        entries: &[SourceEntry<'_>],
        dataforge: Option<usize>,
    ) {
        if !self.convert || !matches!(self.entry_target, EntryTarget::Source) {
            return;
        }
        for (position, entry) in entries.iter().enumerate() {
            if Some(position) != dataforge
                && self
                    .disk_name(entry)
                    .split(['\\', '/'])
                    .any(|part| file_receipt::is_companion_name(OsStr::new(part)))
                && let Ok(relative) = self.relative_path(entry)
                && portable_name::check_path_length(&self.target.join(&relative)).is_ok()
            {
                self.tracker
                    .cryxml_native_paths
                    .record(&relative, entry.index());
            }
        }
    }

    pub(crate) fn cryxml_companion_collision(
        &self,
        relative: &Path,
        entry: &SourceEntry<'_>,
    ) -> Result<(), ExtractionError> {
        let key = key(relative);
        let index = entry.index();
        let conflict = self
            .tracker
            .written
            .conflict_key(&key, index)
            .or_else(|| self.tracker.cryxml_native_paths.conflict_key(&key, index));
        self.report_collision(conflict)
    }

    /// Records the files of the Entries of `ended` that were put in place or kept, so a later
    /// Entry of the run that collides with one is reported.
    pub(crate) fn record_written(&self, entries: &[SourceEntry<'_>], ended: &Ended) {
        for (position, _, outcome) in ended_entries(ended) {
            let Some(entry) = entries.get(*position) else {
                continue;
            };
            if matches!(
                outcome,
                Outcome::Written | Outcome::RawWritten(_) | Outcome::Converted(_) | Outcome::Kept
            ) && let Ok(relative) = self.relative_path(entry)
            {
                self.tracker.written.record(&relative, entry.index());
            }
        }
    }

    /// Returns the error for `relative` of `entry` if another Entry's written file collides with it.
    pub(crate) fn case_collision(
        &self,
        relative: &Path,
        entry: &SourceEntry<'_>,
    ) -> Result<(), ExtractionError> {
        self.report_collision(self.tracker.written.conflict(relative, entry.index()))
    }

    fn report_collision(&self, conflict: Option<usize>) -> Result<(), ExtractionError> {
        conflict.map_or(Ok(()), |earlier| {
            Err(ExtractionError::CaseCollision {
                earlier: self.source.path_of(earlier).map_or_default(str::to_owned),
            })
        })
    }
}

fn ended_entries(ended: &Ended) -> &[EntryEnded] {
    match ended {
        Ended::Entry(entry) | Ended::Sound { entry, .. } => std::slice::from_ref(entry),
        Ended::Texture { entries, .. } => entries,
    }
}

#[cfg(test)]
mod tests {
    use super::WrittenPaths;
    use std::path::Path;

    #[test]
    fn a_path_differing_only_in_case_collides_with_the_written_one() {
        let written = WrittenPaths::default();
        assert_eq!(written.conflict(Path::new("Data/A.txt"), 4), None);
        written.record(Path::new("Data/A.txt"), 4);
        assert_eq!(written.conflict(Path::new("data/a.TXT"), 9), Some(4));
        assert_eq!(written.conflict(Path::new("Data/A.txt"), 4), None, "itself");
        assert_eq!(written.conflict(Path::new("Data/b.txt"), 9), None);
    }

    #[test]
    fn a_file_collides_with_a_written_folder_of_that_name_and_the_reverse() {
        let written = WrittenPaths::default();
        written.record(Path::new("Data/a/x.txt"), 1);
        written.record(Path::new("Data/File"), 2);
        assert_eq!(
            written.conflict(Path::new("Data/A"), 3),
            Some(1),
            "file over folder"
        );
        assert_eq!(
            written.conflict(Path::new("data/file/y.txt"), 3),
            Some(2),
            "folder under file"
        );
        assert_eq!(
            written.conflict(Path::new("Data/A/z.txt"), 3),
            None,
            "same folder"
        );
    }
}
