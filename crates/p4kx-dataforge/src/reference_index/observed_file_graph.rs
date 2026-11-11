// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use rayon::slice::ParallelSliceMut;

use super::files::{FileAssignments, FileCandidate};
use super::{IndexedReference, ObservedReferenceIndex, ReferenceIndexError};
use crate::Record;

/// File groups and their original Reference witnesses from one observed index.
#[derive(Debug)]
pub struct ObservedFileGraph<'index, 'source> {
    index: &'index ObservedReferenceIndex<'source>,
    assignments: FileAssignments<'source>,
    group_starts: Vec<usize>,
    candidates: Vec<FileCandidate>,
}

impl<'source> ObservedReferenceIndex<'source> {
    /// Groups this index's actual Reference occurrences by their original files.
    ///
    /// # Errors
    ///
    /// Returns an error when the supporting occurrence storage cannot be allocated.
    pub fn file_graph(&self) -> Result<ObservedFileGraph<'_, 'source>, ReferenceIndexError> {
        let mut group_starts = Vec::new();
        let assignments = FileAssignments::read(self.forge(), |position| {
            group_starts.try_reserve(1)?;
            group_starts.push(position);
            Ok::<(), ReferenceIndexError>(())
        })?;
        let mut candidates = Vec::new();
        for candidate in self.index().file_candidates(&assignments) {
            candidates.try_reserve(1)?;
            candidates.push(candidate);
        }
        candidates.par_sort_unstable();
        Ok(ObservedFileGraph {
            index: self,
            assignments,
            group_starts,
            candidates,
        })
    }
}

impl<'source> ObservedFileGraph<'_, 'source> {
    /// Borrows each actual normalized display path in file order.
    #[must_use]
    pub fn paths(&self) -> &[&'source str] {
        &self.assignments.paths
    }

    /// Returns the file assigned to this original physical Record during grouping.
    #[must_use]
    pub fn position_of_record(&self, record: u32) -> Option<u32> {
        self.assignments.position(record)
    }

    /// Returns the original Record that supplied the displayed spelling of a file.
    #[must_use]
    pub fn representative(&self, file: u32) -> Option<Record<'source>> {
        let forge = self.index.forge();
        let record = *self.members(file)?.first()?;
        forge.record(record)
    }

    /// Visits the Records of this file in the order used to select its display path.
    #[must_use]
    pub fn member_records(
        &self,
        file: u32,
    ) -> Option<impl ExactSizeIterator<Item = Result<Record<'source>, ReferenceIndexError>> + '_>
    {
        let forge = self.index.forge();
        let members = self.members(file)?;
        Some(
            members
                .iter()
                .map(move |&index| forge.record(index).ok_or(ReferenceIndexError::Inconsistent)),
        )
    }

    fn members(&self, file: u32) -> Option<&[u32]> {
        let file = file as usize;
        let order = self.index.forge().path_order();
        let start = *self.group_starts.get(file)?;
        let end = self
            .group_starts
            .get(file + 1)
            .copied()
            .unwrap_or(order.len());
        order.get(start..end)
    }

    /// Visits each distinct file pair and all its actual supporting occurrences.
    pub fn edges(&self) -> impl Iterator<Item = ObservedFileEdge<'_, 'source>> {
        self.candidates
            .chunk_by(|left, right| left.pair == right.pair)
            .filter_map(|candidates| {
                let first = candidates.first()?;
                Some(ObservedFileEdge {
                    index: self.index,
                    pair: first.pair,
                    candidates,
                })
            })
    }
}

/// One distinct file pair and its actual supporting Reference occurrences.
#[derive(Debug, Clone, Copy)]
pub struct ObservedFileEdge<'graph, 'source> {
    index: &'graph ObservedReferenceIndex<'source>,
    pair: (u32, u32),
    candidates: &'graph [FileCandidate],
}

impl ObservedFileEdge<'_, '_> {
    /// Returns the source and target file positions.
    #[must_use]
    pub const fn files(&self) -> (u32, u32) {
        self.pair
    }

    /// Visits every original supporting Reference in original index order.
    pub fn witnesses(&self) -> impl ExactSizeIterator<Item = IndexedReference<'_>> {
        let index = self.index.index();
        self.candidates
            .iter()
            .map(move |candidate| IndexedReference {
                index,
                entry: candidate.entry,
            })
    }
}
