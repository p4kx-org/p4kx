// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod assemble;
mod collect;
mod files;
mod observed;
mod observed_file_graph;

use std::fmt::Write as _;
use std::ops::Range;

use rayon::prelude::*;

use crate::{DataForge, Guid, TreeBuilder};

pub use files::FileGraph;
pub use observed::{
    IndexedReferenceOrigin, ObservedReferenceIndex, ReferenceIndexError, ReferenceScanGap,
};

pub use observed_file_graph::{ObservedFileEdge, ObservedFileGraph};

use observed::Evidence;

/// An optional occurrence budget for callers that request a bounded Reference index.
pub const MAX_INDEXED_REFERENCES: usize = 1 << 23;

const WAVE_TASKS_PER_THREAD: usize = 4;

pub(crate) const UNRESOLVED: u32 = u32::MAX;

/// Every Reference of every Record of a DataForge, with the field it stands in, answering which
/// Records refer to a Record and which Records a Record refers to.
///
/// A Reference is a Reference field or a weak pointer to the Record instance of another Record; a
/// weak pointer into the Record that holds it is no Reference of the index, and neither is one
/// to a nested instance of another Record, which the Record tree reports as outside the Record.
/// Each Reference keeps the field path of its place, with `[]` for every array on the way, and
/// the index of the element in each of them. A Reference to a GUID no Record has is unresolved:
/// it is listed and counted but has no target. The index holds Record indices and borrows
/// nothing from the DataForge.
#[derive(Debug, Default)]
pub struct ReferenceIndex {
    sources: Vec<u32>,
    targets: Vec<u32>,
    shapes: Vec<usize>,
    index_starts: Vec<usize>,
    indices: Vec<u32>,
    shape_text: String,
    shape_ends: Vec<usize>,
    unresolved: Vec<(usize, [u8; 16])>,
    source_offsets: Vec<usize>,
    target_offsets: Vec<usize>,
    by_target: Vec<usize>,
    unreadable: u32,
    truncated: bool,
}

impl ReferenceIndex {
    /// Reads every Record of `forge` in parallel on rayon's threads and indexes its References.
    ///
    /// Counts Records whose trees cannot be built in the unreadable Record count.
    #[must_use]
    pub fn build(forge: &DataForge<'_>) -> Self {
        Self::build_cancellable(forge, &|| false).unwrap_or_default()
    }

    /// Builds the complete index and returns None when cancelled.
    #[must_use]
    pub fn build_cancellable(
        forge: &DataForge<'_>,
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Option<Self> {
        Self::build_with(forge, usize::MAX, stop)
    }

    /// Indexes like [build](Self::build) but keeps at most `limit` References, the first ones in
    /// the order of the Records whatever the number of threads, and gives up with `None` once
    /// `stop` returns `true`; `stop` is polled from the worker threads between Records.
    #[must_use]
    pub fn build_with(
        forge: &DataForge<'_>,
        limit: usize,
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Option<Self> {
        Self::scan::<false>(forge, limit, stop).map(|(index, _)| index)
    }

    fn scan<const OBSERVED: bool>(
        forge: &DataForge<'_>,
        limit: usize,
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Option<(Self, Option<Evidence>)> {
        let count = u32::try_from(forge.record_count()).unwrap_or(u32::MAX);
        let _warm = forge.record_by_guid(Guid::default());
        let tasks = count.div_ceil(collect::RECORDS_PER_TASK);
        let wave =
            u32::try_from(rayon::current_num_threads().saturating_mul(WAVE_TASKS_PER_THREAD))
                .unwrap_or(u32::MAX)
                .max(1);
        let mut assembler = assemble::Assembler::new(limit, OBSERVED);
        let mut next = 0_u32;
        while next < tasks && !assembler.is_full() {
            if stop() {
                return None;
            }
            let end = tasks.min(next.saturating_add(wave));
            let chunks: Vec<_> = (next..end)
                .into_par_iter()
                .map_init(TreeBuilder::new, |builder, task| {
                    let start = task * collect::RECORDS_PER_TASK;
                    let range: Range<u32> =
                        start..count.min(start.saturating_add(collect::RECORDS_PER_TASK));
                    collect::collect::<OBSERVED>(forge, builder, range, limit, stop)
                })
                .collect();
            for chunk in chunks {
                assembler.push(chunk);
            }
            next = end;
        }
        if stop() {
            return None;
        }
        Some(assembler.finish(forge.record_count()))
    }

    fn finish(&mut self, record_count: usize) {
        self.source_offsets = offsets(self.sources.iter().copied(), record_count);
        let resolved = self
            .targets
            .iter()
            .copied()
            .filter(|target| *target != UNRESOLVED);
        self.target_offsets = offsets(resolved, record_count);
        let mut next = self.target_offsets.clone();
        self.by_target = vec![0; self.target_offsets.last().copied().unwrap_or_default()];
        for (entry, target) in self.targets.iter().copied().enumerate() {
            if let Some(cursor) = next.get_mut(target as usize)
                && let Some(slot) = self.by_target.get_mut(*cursor)
            {
                *slot = entry;
                *cursor += 1;
            }
        }
        self.sources.shrink_to_fit();
        self.targets.shrink_to_fit();
    }

    /// Returns the number of References, unresolved ones included.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.sources.len()
    }

    /// Returns whether the DataForge has no Reference.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// Returns the number of References to a Record.
    #[must_use]
    pub const fn resolved_count(&self) -> usize {
        self.by_target.len()
    }

    /// Returns the number of References to a GUID no Record has.
    #[must_use]
    pub const fn unresolved_count(&self) -> usize {
        self.unresolved.len()
    }

    /// Returns the number of Records whose tree could not be built, which are not indexed.
    #[must_use]
    pub fn unreadable_records(&self) -> usize {
        usize::try_from(self.unreadable).unwrap_or(usize::MAX)
    }

    /// Returns whether the requested occurrence budget omitted at least one Reference.
    #[must_use]
    pub const fn is_truncated(&self) -> bool {
        self.truncated
    }

    /// Returns the References to the Record at `record`, in the order of the Records that hold
    /// them and of the fields within each.
    pub fn incoming(&self, record: u32) -> impl ExactSizeIterator<Item = IndexedReference<'_>> {
        let entries = Self::window(&self.target_offsets, record)
            .and_then(|range| self.by_target.get(range))
            .unwrap_or_default();
        entries.iter().map(|entry| IndexedReference {
            index: self,
            entry: *entry,
        })
    }

    /// Returns the References the Record at `record` holds, unresolved ones included, in the
    /// order of its fields.
    pub fn outgoing(&self, record: u32) -> impl ExactSizeIterator<Item = IndexedReference<'_>> {
        let range = Self::window(&self.source_offsets, record).unwrap_or_default();
        range.map(|entry| IndexedReference { index: self, entry })
    }

    /// Returns every unresolved Reference in the order of the Records that hold them.
    pub fn unresolved(&self) -> impl ExactSizeIterator<Item = IndexedReference<'_>> {
        self.unresolved.iter().map(|(entry, _)| IndexedReference {
            index: self,
            entry: *entry,
        })
    }

    /// Returns the indices of the Records that hold a Reference to the Record at `record`,
    /// ascending and without repeats.
    #[must_use]
    pub fn referrers(&self, record: u32) -> Vec<u32> {
        let mut found: Vec<u32> = self.incoming(record).map(|link| link.source()).collect();
        found.dedup();
        found
    }

    fn window(offsets: &[usize], record: u32) -> Option<Range<usize>> {
        let at = record as usize;
        let start = *offsets.get(at)?;
        let end = *offsets.get(at.checked_add(1)?)?;
        Some(start..end)
    }

    fn shape_of(&self, entry: usize) -> usize {
        self.shapes.get(entry).copied().unwrap_or_default()
    }
}

/// One Reference of a [`ReferenceIndex`].
#[derive(Debug, Clone, Copy)]
pub struct IndexedReference<'i> {
    index: &'i ReferenceIndex,
    entry: usize,
}

impl<'i> IndexedReference<'i> {
    /// Returns the index of the Record that holds the Reference.
    #[must_use]
    pub fn source(&self) -> u32 {
        self.index
            .sources
            .get(self.entry)
            .copied()
            .unwrap_or_default()
    }

    /// Returns the index of the Record the Reference links to, or `None` when it is unresolved.
    #[must_use]
    pub fn target(&self) -> Option<u32> {
        self.index
            .targets
            .get(self.entry)
            .copied()
            .filter(|target| *target != UNRESOLVED)
    }

    /// Returns the GUID of an unresolved Reference, or `None` when its target exists.
    #[must_use]
    pub fn unresolved_guid(&self) -> Option<Guid> {
        let at = self
            .index
            .unresolved
            .binary_search_by_key(&self.entry, |(entry, _)| *entry)
            .ok()?;
        self.index
            .unresolved
            .get(at)
            .map(|(_, guid)| Guid::from_stored(*guid))
    }

    /// Returns the field path with `[]` after every array on the way, e.g.
    /// `Components[].fireActions[].ammo`.
    #[must_use]
    pub fn field_path(&self) -> &'i str {
        let shape = self.index.shape_of(self.entry);
        let end = self
            .index
            .shape_ends
            .get(shape)
            .copied()
            .unwrap_or_default();
        let start = shape
            .checked_sub(1)
            .and_then(|before| self.index.shape_ends.get(before))
            .copied()
            .unwrap_or_default();
        self.index.shape_text.get(start..end).unwrap_or_default()
    }

    /// Returns the index of the element in each array of the field path, outermost first.
    #[must_use]
    pub fn indices(&self) -> &'i [u32] {
        let Some(&start) = self.index.index_starts.get(self.entry) else {
            return &[];
        };
        let end = self
            .index
            .index_starts
            .get(self.entry + 1)
            .copied()
            .unwrap_or(self.index.indices.len());
        self.index.indices.get(start..end).unwrap_or_default()
    }

    /// Returns the place of the Reference as the Record preview writes it, e.g.
    /// `Components[3].fireActions[0].ammo`.
    #[must_use]
    pub fn location(&self) -> String {
        let mut text = String::with_capacity(self.field_path().len() + 8);
        let mut indices = self.indices().iter();
        let mut rest = self.field_path();
        while let Some((before, after)) = rest.split_once("[]") {
            text.push_str(before);
            if let Some(index) = indices.next() {
                let _infallible = write!(text, "[{index}]");
            }
            rest = after;
        }
        text.push_str(rest);
        text
    }
}

fn offsets(sorted: impl Iterator<Item = u32>, record_count: usize) -> Vec<usize> {
    let mut offsets = vec![0_usize; record_count.saturating_add(1)];
    for key in sorted {
        if let Some(slot) = offsets.get_mut((key as usize).saturating_add(1)) {
            *slot += 1;
        }
    }
    let mut sum = 0_usize;
    for slot in &mut offsets {
        sum += *slot;
        *slot = sum;
    }
    offsets
}
