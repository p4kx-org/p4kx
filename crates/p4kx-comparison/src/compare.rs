// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::cmp::Ordering;

use rayon::prelude::*;

use crate::comparison::RecordSources;
use crate::entry_change::EntryDecision;
use crate::path_key::cmp_normalised;
use crate::record_compare::compare_records;
use crate::{
    Archive, CancelToken, CompareOptions, Comparison, ComparisonError, ComparisonProgress,
    DataForgeEntry, DataForges, Entry, EntryChange, Side,
};

#[path = "compare_collision.rs"]
mod collision;

pub(crate) use collision::CollisionEvidence;

const PROGRESS_STEP: usize = 1 << 16;

#[derive(Debug, Clone, Default)]
pub(crate) struct JoinResult {
    pub(crate) changes: Vec<Joined>,
    pub(crate) collisions: Vec<CollisionEvidence>,
    total: usize,
}

/// One changed Entry as positions in the Entry lists of both Archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Joined {
    pub(crate) old: Option<usize>,
    pub(crate) new: Option<usize>,
    decision: EntryDecision,
}

impl Joined {
    /// Returns the change of the Entries at these positions, `None` when a position is out of
    /// range or both are missing.
    pub(crate) fn resolve<'a>(self, old: &'a Archive, new: &'a Archive) -> Option<EntryChange<'a>> {
        let before = match self.old {
            Some(position) => Some(old.entries().get(position)?),
            None => None,
        };
        let after = match self.new {
            Some(position) => Some(new.entries().get(position)?),
            None => None,
        };
        match (before, after) {
            (Some(before), Some(after)) => Some(EntryChange::changed(before, after, self.decision)),
            (Some(before), None) => Some(EntryChange::removed(before, self.decision)),
            (None, Some(after)) => Some(EntryChange::added(after, self.decision)),
            (None, None) => None,
        }
    }
}

/// Compares the `old` with the `new` Archive on the selected levels.
///
/// Entries are joined by their path with ASCII case folded and `/` read as `\`; an Entry is
/// changed when its uncompressed size or declared checksum differs. Duplicate occurrences are
/// reconciled using original spellings and declared content. Nothing is decompressed.
///
/// Records of the `dataforges` are joined by GUID: a Record is moved when its record path
/// differs, ignoring case and separator kind, and changed when its name, its type or the value
/// of a field differs. Fields are joined by name and floats compared by their bits.
///
/// # Errors
///
/// Returns [`ComparisonError::NoLevel`] when no level is selected,
/// [`ComparisonError::NoDataForge`] when the Record level is selected without `dataforges`
/// and [`ComparisonError::Cancelled`] when the Comparison was cancelled. A Record that cannot be
/// read is reported as unreadable, not as an error.
pub fn compare<'a>(
    old: &'a Archive,
    new: &'a Archive,
    dataforges: Option<DataForges<'a>>,
    options: &CompareOptions<'_>,
) -> Result<Comparison<'a>, ComparisonError> {
    compare_inner(old, new, dataforges, options, RecordSources::default())
}

fn compare_inner<'a>(
    old: &'a Archive,
    new: &'a Archive,
    dataforges: Option<DataForges<'a>>,
    options: &CompareOptions<'_>,
    record_sources: RecordSources,
) -> Result<Comparison<'a>, ComparisonError> {
    check_levels(options, dataforges.is_some())?;
    let joined = join(old, new, options)?;
    let entries_total = joined.total;
    let progress = ComparisonProgress {
        entries_done: entries_total,
        entries_total,
        ..ComparisonProgress::default()
    };
    let records = match dataforges.filter(|_| options.levels.records) {
        Some(forges) => compare_records(forges, options, progress, &record_sources)?,
        None => Vec::new(),
    };
    Ok(Comparison {
        old: Side::of(old).with_public_patches(options.public_patches),
        new: Side::of(new).with_public_patches(options.public_patches),
        levels: options.levels,
        filter: options.filter.map(|filter| filter.as_str().to_owned()),
        entries: joined
            .changes
            .iter()
            .filter_map(|joined| joined.resolve(old, new))
            .collect(),
        entry_collisions: Cow::Owned(joined.collisions),
        records,
        record_sources,
    })
}

/// Compares the Archives and retains the original input of each bound DataForge.
///
/// Evidence requires the exact opened bytes and the original Archive owner.
///
/// # Errors
///
/// Returns the same comparison errors as [compare].
pub fn compare_with_dataforge_entries<'a>(
    old: &'a Archive,
    new: &'a Archive,
    dataforges: DataForges<'a>,
    entries: [&DataForgeEntry; 2],
    options: &CompareOptions<'_>,
) -> Result<Comparison<'a>, ComparisonError> {
    let sources = if options.levels.records {
        let [old_entry, new_entry] = entries;
        RecordSources::new(
            old_entry
                .origin_of(dataforges.old)
                .filter(|origin| origin.matches_archive(old)),
            new_entry
                .origin_of(dataforges.new)
                .filter(|origin| origin.matches_archive(new)),
        )
    } else {
        RecordSources::default()
    };
    compare_inner(old, new, Some(dataforges), options, sources)
}

/// Checks that a level is selected, that the Record level has DataForges, and that the
/// Comparison was not cancelled yet.
pub(crate) fn check_levels(
    options: &CompareOptions<'_>,
    has_dataforges: bool,
) -> Result<(), ComparisonError> {
    if options.levels.is_empty() {
        return Err(ComparisonError::NoLevel);
    }
    if options.levels.records && !has_dataforges {
        return Err(ComparisonError::NoDataForge);
    }
    check_cancel(options)
}

/// Joins the Entries of both Archives into the positions of the changed ones, ordered by
/// normalised path, and retains physical evidence for collisions.
pub(crate) fn join(
    old: &Archive,
    new: &Archive,
    options: &CompareOptions<'_>,
) -> Result<JoinResult, ComparisonError> {
    if options.levels.entries {
        join_entries(old, new, options)
    } else {
        Ok(JoinResult::default())
    }
}

const fn removed(position: usize, decision: EntryDecision) -> Joined {
    Joined {
        old: Some(position),
        new: None,
        decision,
    }
}

const fn added(position: usize, decision: EntryDecision) -> Joined {
    Joined {
        old: None,
        new: Some(position),
        decision,
    }
}

const fn changed(before: usize, after: usize, decision: EntryDecision) -> Joined {
    Joined {
        old: Some(before),
        new: Some(after),
        decision,
    }
}

fn join_entries(
    old: &Archive,
    new: &Archive,
    options: &CompareOptions<'_>,
) -> Result<JoinResult, ComparisonError> {
    let (old, new) = rayon::join(
        || sorted_selection(old, options),
        || sorted_selection(new, options),
    );
    check_cancel(options)?;
    merge_sorted_entries(&old, &new, options)
}

fn merge_sorted_entries(
    old: &[(usize, &Entry)],
    new: &[(usize, &Entry)],
    options: &CompareOptions<'_>,
) -> Result<JoinResult, ComparisonError> {
    let total = old.len() + new.len();
    report_entry_progress(options, 0, total);
    let mut result = JoinResult {
        total,
        ..JoinResult::default()
    };
    let (mut i, mut j) = (0, 0);
    let mut next_report = PROGRESS_STEP;
    loop {
        let change = match (old.get(i), new.get(j)) {
            (None, None) => break,
            (Some(&(before, _)), None) => {
                i += 1;
                Some(removed(before, EntryDecision::OppositeExhausted))
            }
            (None, Some(&(after, _))) => {
                j += 1;
                Some(added(after, EntryDecision::OppositeExhausted))
            }
            (Some(&(before, old_entry)), Some(&(after, new_entry))) => {
                match cmp_normalised(old_entry.path(), new_entry.path()) {
                    Ordering::Less => {
                        i += 1;
                        Some(removed(before, EntryDecision::OrderedAgainst(after)))
                    }
                    Ordering::Greater => {
                        j += 1;
                        Some(added(after, EntryDecision::OrderedAgainst(before)))
                    }
                    Ordering::Equal => {
                        if let Some((old_count, new_count)) = collision::join_if_needed(
                            old.split_at(i).1,
                            new.split_at(j).1,
                            old_entry.path(),
                            &mut result,
                            options,
                        )? {
                            i += old_count;
                            j += new_count;
                            None
                        } else {
                            i += 1;
                            j += 1;
                            difference(old_entry, new_entry)
                                .map(|decision| changed(before, after, decision))
                        }
                    }
                }
            }
        };
        result.changes.extend(change);
        if i + j >= next_report {
            check_cancel(options)?;
            report_entry_progress(options, i + j, total);
            next_report = i + j + PROGRESS_STEP;
        }
    }
    report_entry_progress(options, total, total);
    Ok(result)
}

fn report_entry_progress(options: &CompareOptions<'_>, done: usize, total: usize) {
    if let Some(progress) = options.progress {
        progress(ComparisonProgress {
            entries_done: done,
            entries_total: total,
            ..ComparisonProgress::default()
        });
    }
}

fn sorted_selection<'a>(
    archive: &'a Archive,
    options: &CompareOptions<'_>,
) -> Vec<(usize, &'a Entry)> {
    let mut entries: Vec<(usize, &Entry)> = options.filter.map_or_else(
        || archive.entries().iter().enumerate().collect(),
        |filter| {
            archive
                .entries()
                .par_iter()
                .enumerate()
                .filter(|(_, entry)| filter.matches(entry.path()))
                .collect()
        },
    );
    entries.par_sort_by(|(_, a), (_, b)| {
        cmp_normalised(a.path(), b.path()).then_with(|| a.path().cmp(b.path()))
    });
    entries
}

fn difference(old: &Entry, new: &Entry) -> Option<EntryDecision> {
    if old.uncompressed_size() != new.uncompressed_size() {
        Some(EntryDecision::SizeDiffers)
    } else if old.checksum() != new.checksum() {
        Some(EntryDecision::ChecksumDiffers)
    } else {
        None
    }
}

pub(crate) fn check_cancel(options: &CompareOptions<'_>) -> Result<(), ComparisonError> {
    if options.cancel.is_some_and(CancelToken::is_cancelled) {
        Err(ComparisonError::Cancelled)
    } else {
        Ok(())
    }
}
