// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

pub(crate) struct Paired<'a, T> {
    pub(crate) hash: u32,
    pub(crate) ordinal: usize,
    pub(crate) old: Option<&'a T>,
    pub(crate) new: Option<&'a T>,
    pub(crate) new_at: Option<usize>,
    pub(crate) old_at: Option<usize>,
}

fn ordinals<T>(items: &[T], key: fn(&T) -> u32) -> Vec<(u32, usize)> {
    let mut seen: HashMap<u32, usize> = HashMap::with_capacity(items.len());
    items
        .iter()
        .map(|item| {
            let hash = key(item);
            let ordinal = seen.entry(hash).or_default();
            let at = *ordinal;
            *ordinal += 1;
            (hash, at)
        })
        .collect()
}

fn first_free(
    keys: &[(u32, usize)],
    taken: &[bool],
    hash: u32,
    accept: impl Fn(usize) -> bool,
) -> Option<usize> {
    keys.iter()
        .zip(taken)
        .enumerate()
        .find(|(at, ((other, _), taken))| !**taken && *other == hash && accept(*at))
        .map(|(at, _)| at)
}

fn claim(mate: Option<usize>, taken: &mut [bool]) -> Option<usize> {
    if let Some(slot) = mate.and_then(|at| taken.get_mut(at)) {
        *slot = true;
    }
    mate
}

/// Pairs the items of two lists that have the same hash: first those that are equal, then the
/// rest in order. The pairs follow the first list, the items only in the second come last.
pub(crate) fn pair<'a, T: PartialEq>(
    old: &'a [T],
    new: &'a [T],
    key: fn(&T) -> u32,
) -> Vec<Paired<'a, T>> {
    let old_keys = ordinals(old, key);
    let new_keys = ordinals(new, key);
    let mut taken = vec![false; new.len()];
    let mut mates: Vec<Option<usize>> = Vec::with_capacity(old.len());
    for (item, (hash, _)) in old.iter().zip(&old_keys) {
        let mate = first_free(&new_keys, &taken, *hash, |at| new.get(at) == Some(item));
        mates.push(claim(mate, &mut taken));
    }
    for (mate, (hash, _)) in mates.iter_mut().zip(&old_keys) {
        if mate.is_none() {
            let found = first_free(&new_keys, &taken, *hash, |_| true);
            *mate = claim(found, &mut taken);
        }
    }
    let mut pairs: Vec<_> = old
        .iter()
        .zip(&old_keys)
        .zip(&mates)
        .enumerate()
        .map(|(index, ((item, (hash, ordinal)), mate))| Paired {
            hash: *hash,
            ordinal: *ordinal,
            old: Some(item),
            new: mate.and_then(|at| new.get(at)),
            new_at: *mate,
            old_at: Some(index),
        })
        .collect();
    pairs.extend(
        new.iter()
            .zip(&new_keys)
            .zip(&taken)
            .enumerate()
            .filter(|(_, (_, taken))| !**taken)
            .map(|(index, ((item, (hash, ordinal)), _))| Paired {
                hash: *hash,
                ordinal: *ordinal,
                old: None,
                new: Some(item),
                new_at: Some(index),
                old_at: None,
            }),
    );
    pairs
}
