// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;

use rayon::slice::ParallelSliceMut;

use super::ReferenceIndex;
use crate::DataForge;
use crate::strings::compare_paths;

const NO_FILE: u32 = u32::MAX;

/// The record files of a DataForge and which of them refer to which.
///
/// A record file is a record path ignoring ASCII case, the kind of separator and a trailing
/// `.xml`; the Records sharing one belong to one file. There is an edge from file A to file B,
/// A different from B, when a Record of A refers to a Record of B, so a chain of References
/// through Records of one file ends at the files they reach. Unresolved References give no
/// edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileGraph<'f> {
    paths: Vec<&'f str>,
    edges: Vec<(u32, u32)>,
}

impl<'f> FileGraph<'f> {
    /// Returns the record path of each file in record-path order, as written by its first Record.
    #[must_use]
    pub fn paths(&self) -> &[&'f str] {
        &self.paths
    }

    /// Returns the edges as pairs of positions in [`paths`](Self::paths), sorted and without
    /// repeats.
    #[must_use]
    pub fn edges(&self) -> &[(u32, u32)] {
        &self.edges
    }

    /// Returns the record paths of the files that the file at `file` refers to.
    pub fn refers_to(&self, file: u32) -> impl Iterator<Item = &'f str> + '_ {
        self.edges
            .iter()
            .filter(move |(from, _)| *from == file)
            .filter_map(|(_, to)| self.paths.get(*to as usize).copied())
    }

    /// Returns the position of the file the record path `path` names, ignoring ASCII case and
    /// the kind of separator.
    #[must_use]
    pub fn position(&self, path: &str) -> Option<u32> {
        let at = self
            .paths
            .binary_search_by(|candidate| compare_paths(candidate, path))
            .ok()?;
        u32::try_from(at).ok()
    }
}

impl ReferenceIndex {
    /// Groups the References by the record files of their Records and returns the edges between
    /// different files; `forge` is the DataForge the index was built from.
    #[must_use]
    pub fn file_graph<'f>(&self, forge: &'f DataForge<'f>) -> FileGraph<'f> {
        let Ok(assignments) = FileAssignments::read(forge, |_| Ok::<(), Infallible>(()));
        let mut edges: Vec<_> = self
            .file_candidates(&assignments)
            .map(|candidate| candidate.pair)
            .collect();
        edges.par_sort_unstable();
        edges.dedup();
        FileGraph {
            paths: assignments.paths,
            edges,
        }
    }

    pub(super) fn file_candidates<'index>(
        &'index self,
        assignments: &'index FileAssignments<'_>,
    ) -> impl Iterator<Item = FileCandidate> + 'index {
        self.sources
            .iter()
            .zip(&self.targets)
            .enumerate()
            .filter_map(move |(entry, (source, target))| {
                if *target == super::UNRESOLVED {
                    return None;
                }
                let from = assignments.position(*source)?;
                let to = assignments.position(*target)?;
                (from != to).then_some(FileCandidate {
                    pair: (from, to),
                    entry,
                })
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct FileCandidate {
    pub(super) pair: (u32, u32),
    pub(super) entry: usize,
}

#[derive(Debug)]
pub(super) struct FileAssignments<'source> {
    pub(super) paths: Vec<&'source str>,
    file_of: Vec<u32>,
}

impl<'source> FileAssignments<'source> {
    pub(super) fn read<E>(
        forge: &'source DataForge<'source>,
        mut group: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        let mut file_of = vec![NO_FILE; forge.record_count()];
        let mut paths: Vec<&'source str> = Vec::new();
        for (position, &record) in forge.path_order().iter().enumerate() {
            let Some(path) = forge.path_of(record) else {
                continue;
            };
            if paths
                .last()
                .is_none_or(|last| compare_paths(last, path).is_ne())
            {
                paths.push(path);
                group(position)?;
            }
            if let Some(slot) = file_of.get_mut(record as usize) {
                *slot = u32::try_from(paths.len().saturating_sub(1)).unwrap_or(NO_FILE);
            }
        }
        Ok(Self { paths, file_of })
    }

    pub(super) fn position(&self, record: u32) -> Option<u32> {
        self.file_of
            .get(record as usize)
            .copied()
            .filter(|file| *file != NO_FILE)
    }
}
