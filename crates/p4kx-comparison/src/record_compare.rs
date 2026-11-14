// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

use rayon::prelude::*;

use crate::compare::check_cancel;
use crate::field_diff::diff_fields;
use crate::path_key::cmp_normalised;
use crate::{
    CompareOptions, ComparisonError, ComparisonProgress, DataForge, DataForgeError, DataForges,
    Record, RecordChange, RecordChangeKind, RecordError, RecordKey, RecordSide, RecordState,
};

const PROGRESS_STEP: usize = 1 << 12;

/// The Records of one GUID: on one side only, or on both.
#[derive(Clone, Copy)]
enum Joined<'a> {
    Removed(Record<'a>),
    Added(Record<'a>),
    Both(Record<'a>, Record<'a>),
}

/// Joins the Records of both DataForges by GUID and returns the added, removed, moved and
/// changed ones, ordered by record path ignoring case and separator kind, then by GUID.
pub(crate) fn compare_records<'a>(
    forges: DataForges<'a>,
    options: &CompareOptions<'_>,
    progress_so_far: ComparisonProgress,
) -> Result<Vec<RecordChange<'a>>, ComparisonError> {
    let (old, new) = rayon::join(|| by_guid(forges.old), || by_guid(forges.new));
    check_cancel(options)?;
    let joined = join(&old, &new);
    drop((old, new));
    let total = joined
        .iter()
        .filter(|pair| matches!(pair, Joined::Both(..)))
        .count();
    let done = AtomicUsize::new(0);
    let report = |done| {
        if let Some(progress) = options.progress {
            progress(ComparisonProgress {
                records_done: done,
                records_total: total,
                ..progress_so_far
            });
        }
    };
    report(0);
    let changes: Result<Vec<Option<RecordChange<'a>>>, ComparisonError> = joined
        .into_par_iter()
        .map(|pair| {
            check_cancel(options)?;
            let change = diff(pair, options);
            if matches!(pair, Joined::Both(..)) {
                let finished = done.fetch_add(1, AtomicOrdering::Relaxed).saturating_add(1);
                if finished.is_multiple_of(PROGRESS_STEP) {
                    report(finished);
                }
            }
            Ok(change)
        })
        .collect();
    let mut changes: Vec<RecordChange<'a>> = changes?.into_iter().flatten().collect();
    changes.par_sort_unstable_by(|a, b| {
        cmp_normalised(a.path(), b.path())
            .then_with(|| a.path().cmp(b.path()))
            .then_with(|| a.guid().cmp(&b.guid()))
    });
    report(total);
    Ok(changes)
}

/// Compares the Records at the positions of `key` again, as [`compare_records`] does; `None`
/// when they do not differ, neither position holds a Record or the two Records have different
/// GUIDs.
pub(crate) fn compare_record<'a>(
    forges: DataForges<'a>,
    key: RecordKey,
) -> Option<RecordChange<'a>> {
    let side = |forge: &'a DataForge<'a>, position: Option<u32>| {
        position.map_or(Some(None), |index| forge.record(index).map(Some))
    };
    let pair = match (
        side(forges.old, key.old_position())?,
        side(forges.new, key.new_position())?,
    ) {
        (Some(old), Some(new)) if old.guid() == new.guid() => Joined::Both(old, new),
        (Some(old), None) => Joined::Removed(old),
        (None, Some(new)) => Joined::Added(new),
        _ => return None,
    };
    diff(pair, &CompareOptions::default())
}

fn by_guid<'a>(forge: &'a DataForge<'a>) -> Vec<Record<'a>> {
    let mut records: Vec<Record<'a>> = forge.records().collect();
    records.par_sort_unstable_by_key(|record| (record.guid(), record.index()));
    records
}

fn join<'a>(old: &[Record<'a>], new: &[Record<'a>]) -> Vec<Joined<'a>> {
    let mut joined = Vec::with_capacity(old.len().max(new.len()));
    let (mut i, mut j) = (0, 0);
    loop {
        let pair = match (old.get(i), new.get(j)) {
            (None, None) => break,
            (Some(&before), None) => {
                i += 1;
                Joined::Removed(before)
            }
            (None, Some(&after)) => {
                j += 1;
                Joined::Added(after)
            }
            (Some(&before), Some(&after)) => match before.guid().cmp(&after.guid()) {
                Ordering::Less => {
                    i += 1;
                    Joined::Removed(before)
                }
                Ordering::Greater => {
                    j += 1;
                    Joined::Added(after)
                }
                Ordering::Equal => {
                    i += 1;
                    j += 1;
                    Joined::Both(before, after)
                }
            },
        };
        joined.push(pair);
    }
    joined
}

fn diff<'a>(pair: Joined<'a>, options: &CompareOptions<'_>) -> Option<RecordChange<'a>> {
    let (old, new) = match pair {
        Joined::Removed(old) => (Some(old), None),
        Joined::Added(new) => (None, Some(new)),
        Joined::Both(old, new) => (Some(old), Some(new)),
    };
    let mut errors = Vec::new();
    let old_path = old.and_then(|record| read(record.record_path(), RecordSide::Old, &mut errors));
    let new_path = new.and_then(|record| read(record.record_path(), RecordSide::New, &mut errors));
    let mut paths = old_path.into_iter().chain(new_path).peekable();
    if let Some(filter) = options.filter
        && paths.peek().is_some()
        && !paths.any(|path| filter.matches(path))
    {
        return None;
    }
    let old_state = old
        .zip(old_path)
        .and_then(|(record, path)| read(state(record, path), RecordSide::Old, &mut errors));
    let new_state = new
        .zip(new_path)
        .and_then(|(record, path)| read(state(record, path), RecordSide::New, &mut errors));
    let mut change = RecordChange {
        key: RecordKey::new(
            old.map(|record| record.index()),
            new.map(|record| record.index()),
        ),
        guid: new.or(old).map_or_default(|record| record.guid()),
        old: old_state,
        new: new_state,
        fields: Vec::new(),
        errors,
    };
    let (Some(old), Some(new), Some(before), Some(after), true) =
        (old, new, old_state, new_state, change.errors.is_empty())
    else {
        return Some(change);
    };
    let old_tree = read(old.tree(), RecordSide::Old, &mut change.errors);
    let new_tree = read(new.tree(), RecordSide::New, &mut change.errors);
    let (Some(old_tree), Some(new_tree)) = (old_tree, new_tree) else {
        return Some(change);
    };
    change.fields = diff_fields(old_tree.fields(), new_tree.fields());
    let differs = !change.fields.is_empty()
        || before.name != after.name
        || before.tag != after.tag
        || before.type_name != after.type_name
        || change.kind() == RecordChangeKind::Moved;
    differs.then_some(change)
}

fn state<'a>(record: Record<'a>, record_path: &'a str) -> Result<RecordState<'a>, DataForgeError> {
    Ok(RecordState {
        record_path,
        name: record.name()?,
        tag: record.tag()?.filter(|tag| !tag.is_empty()),
        type_name: record.type_name()?,
    })
}

/// Returns the value, or records the error for `side` and returns `None`.
fn read<T>(
    result: Result<T, DataForgeError>,
    side: RecordSide,
    errors: &mut Vec<RecordError>,
) -> Option<T> {
    result
        .map_err(|error| errors.push(RecordError { side, error }))
        .ok()
}

#[cfg(test)]
mod tests {
    use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType};

    use super::compare_record;
    use crate::{DataForge, DataForges, RecordKey};

    fn database(guid: u128) -> Vec<u8> {
        DataForgeBuilder::new()
            .structure(TestStruct::new("Ship").field("speed", TestType::Int32))
            .record(TestRecord::new(
                guid,
                "Ship",
                "Ship.One",
                "libs/ships/one.xml",
            ))
            .build()
    }

    #[test]
    fn a_key_over_records_of_different_guids_has_no_change() -> Result<(), crate::DataForgeError> {
        let (old, new) = (database(1), database(2));
        let (old, new) = (DataForge::open(&old)?, DataForge::open(&new)?);
        let forges = DataForges {
            old: &old,
            new: &new,
        };

        let change = compare_record(forges, RecordKey::new(Some(0), Some(0)));

        assert!(change.is_none(), "two unrelated Records: {change:?}");
        Ok(())
    }
}
