// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::hash::Hash;

use p4kx_archive::Checksum;

use crate::entry_change::EntryDecision;
use crate::path_key::eq_normalised;
use crate::{CompareOptions, ComparisonError, Entry};

use super::{JoinResult, Joined, added, changed, check_cancel, difference, removed};

type Candidate<'archive> = (usize, &'archive Entry);

#[derive(Debug, Clone)]
pub(crate) struct CollisionEvidence {
    pub(crate) old: Box<[usize]>,
    pub(crate) new: Box<[usize]>,
}

struct Occurrence<'archive> {
    position: usize,
    entry: &'archive Entry,
    taken: bool,
    ambiguous: bool,
}

pub(super) fn join_if_needed(
    old: &[Candidate<'_>],
    new: &[Candidate<'_>],
    path: &str,
    result: &mut JoinResult,
    options: &CompareOptions<'_>,
) -> Result<Option<(usize, usize)>, ComparisonError> {
    let duplicates = [old, new].iter().any(|side| {
        side.get(1)
            .is_some_and(|(_, entry)| eq_normalised(entry.path(), path))
    });
    if !duplicates {
        return Ok(None);
    }
    let old_count = old.partition_point(|(_, entry)| eq_normalised(entry.path(), path));
    let new_count = new.partition_point(|(_, entry)| eq_normalised(entry.path(), path));
    let old = old.split_at(old_count).0;
    let new = new.split_at(new_count).0;
    let changes = join(old, new, result.collisions.len(), options)?;
    if !changes.is_empty() {
        result.collisions.push(CollisionEvidence {
            old: old.iter().map(|(position, _)| *position).collect(),
            new: new.iter().map(|(position, _)| *position).collect(),
        });
        result.extend(changes);
    }
    Ok(Some((old_count, new_count)))
}

fn join<'archive>(
    old: &[Candidate<'archive>],
    new: &[Candidate<'archive>],
    evidence: usize,
    options: &CompareOptions<'_>,
) -> Result<Vec<Joined>, ComparisonError> {
    let mut old = occurrences(old);
    let mut new = occurrences(new);
    cancel_equal(
        &mut old,
        &mut new,
        |entry| (entry.path(), contents(entry)),
        options,
    )?;
    let mut changes = Vec::new();
    pair_unique_names(&mut old, &mut new, evidence, &mut changes, options)?;
    cancel_equal(&mut old, &mut new, contents, options)?;
    remaining(&mut old, &mut new, evidence, &mut changes, options)?;
    changes.sort_unstable_by_key(|change| (change.new.or(change.old), change.new.is_none()));
    Ok(changes)
}

fn occurrences<'archive>(candidates: &[Candidate<'archive>]) -> Vec<Occurrence<'archive>> {
    candidates
        .iter()
        .map(|&(position, entry)| Occurrence {
            position,
            entry,
            taken: false,
            ambiguous: false,
        })
        .collect()
}

const fn contents(entry: &Entry) -> (u64, Checksum) {
    (entry.uncompressed_size(), entry.checksum())
}

fn groups<'group, 'archive, Key: Eq + Hash>(
    side: &'group mut [Occurrence<'archive>],
    key: &impl Fn(&'archive Entry) -> Key,
    options: &CompareOptions<'_>,
) -> Result<HashMap<Key, Vec<&'group mut Occurrence<'archive>>>, ComparisonError> {
    let mut groups = HashMap::<_, Vec<_>>::new();
    for candidate in side.iter_mut().filter(|candidate| !candidate.taken) {
        check_cancel(options)?;
        groups
            .entry(key(candidate.entry))
            .or_default()
            .push(candidate);
    }
    Ok(groups)
}

fn cancel_equal<'archive, Key: Eq + Hash>(
    old: &mut [Occurrence<'archive>],
    new: &mut [Occurrence<'archive>],
    key: impl Fn(&'archive Entry) -> Key,
    options: &CompareOptions<'_>,
) -> Result<(), ComparisonError> {
    let mut after = groups(new, &key, options)?;
    for (fingerprint, before) in groups(old, &key, options)? {
        let Some(after) = after.remove(&fingerprint) else {
            continue;
        };
        let matched = before.len().min(after.len());
        let tied = before.len() != after.len();
        for group in [before, after] {
            for (index, candidate) in group.into_iter().enumerate() {
                check_cancel(options)?;
                candidate.taken = index < matched;
                candidate.ambiguous |= !candidate.taken && tied;
            }
        }
    }
    Ok(())
}

fn pair_unique_names(
    old: &mut [Occurrence<'_>],
    new: &mut [Occurrence<'_>],
    evidence: usize,
    changes: &mut Vec<Joined>,
    options: &CompareOptions<'_>,
) -> Result<(), ComparisonError> {
    let mut after = groups(new, &Entry::path, options)?;
    for (path, mut before) in groups(old, &Entry::path, options)? {
        check_cancel(options)?;
        let Some(mut after) = after.remove(path) else {
            continue;
        };
        if let ([before], [after]) = (before.as_mut_slice(), after.as_mut_slice()) {
            pair(before, after, evidence, changes);
        }
    }
    Ok(())
}

fn pair(
    old: &mut Occurrence<'_>,
    new: &mut Occurrence<'_>,
    evidence: usize,
    changes: &mut Vec<Joined>,
) {
    old.taken = true;
    new.taken = true;
    if let Some(decision) = difference(old.entry, new.entry) {
        let ambiguous = old.ambiguous || new.ambiguous;
        let decision = if matches!(decision, EntryDecision::Incomparable { .. }) {
            EntryDecision::Incomparable {
                collision: Some(evidence),
                ambiguous,
            }
        } else {
            EntryDecision::Collision {
                evidence,
                ambiguous,
            }
        };
        changes.push(changed(old.position, new.position, decision));
    }
}

fn remaining(
    old: &mut [Occurrence<'_>],
    new: &mut [Occurrence<'_>],
    evidence: usize,
    changes: &mut Vec<Joined>,
    options: &CompareOptions<'_>,
) -> Result<(), ComparisonError> {
    let two_sided = old.iter().any(|candidate| !candidate.taken)
        && new.iter().any(|candidate| !candidate.taken);
    let mut before = old.iter_mut().filter(|candidate| !candidate.taken);
    let mut after = new.iter_mut().filter(|candidate| !candidate.taken);
    if let (Some(left), Some(right), None, None) =
        (before.next(), after.next(), before.next(), after.next())
    {
        pair(left, right, evidence, changes);
        return Ok(());
    }
    for (side, change) in [
        (&*old, removed as fn(usize, EntryDecision) -> Joined),
        (&*new, added),
    ] {
        for candidate in side.iter().filter(|candidate| !candidate.taken) {
            check_cancel(options)?;
            let decision = EntryDecision::Collision {
                evidence,
                ambiguous: candidate.ambiguous || two_sided,
            };
            changes.push(change(candidate.position, decision));
        }
    }
    Ok(())
}
