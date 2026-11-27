// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use super::index::{self, Entry, Field};
use crate::DataForgeError;
use crate::source_layout_allocation;

/// A physical range ordered by its earliest contributing struct and original prefix.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct DeclarationRange {
    first: usize,
    end: usize,
    rank: usize,
    base: i128,
}

/// Selects the earliest valid struct's first occurrence without expanding inherited ranges.
pub(super) fn union(
    entries: &[Entry],
    declarations: &[Field],
    names: &[Box<str>],
) -> Result<Box<[Field]>, DataForgeError> {
    let mut ranges = ranges(entries)?.into_iter().peekable();
    let mut active = BinaryHeap::new();
    active
        .try_reserve(entries.len())
        .map_err(|_| DataForgeError::LayoutAllocationRefused {
            part: "completion range sweep",
            elements: entries.len() as u64,
        })?;
    let mut selected = HashMap::new();
    selected.try_reserve(declarations.len()).map_err(|_| {
        DataForgeError::LayoutAllocationRefused {
            part: "completion union",
            elements: declarations.len() as u64,
        }
    })?;
    for (position, field) in declarations.iter().copied().enumerate() {
        while ranges.peek().is_some_and(|range| range.first <= position) {
            let Some(range) = ranges.next() else {
                return Err(DataForgeError::LayoutInvariant);
            };
            active.push(Reverse((range.rank, range.base, range.end)));
        }
        while active
            .peek()
            .is_some_and(|Reverse((_, _, end))| *end <= position)
        {
            active.pop();
        }
        if let Some(Reverse((rank, base, _))) = active.peek() {
            let ordinal = u64::try_from(*base + position as i128)
                .map_err(|_| DataForgeError::LayoutInvariant)?;
            let priority = (*rank, ordinal);
            let selection = selected.entry(field.name).or_insert((priority, field));
            if priority < selection.0 {
                *selection = (priority, field);
            }
        }
    }
    ordered(selected.into_values().map(|(_, field)| field), names)
}

fn ranges(entries: &[Entry]) -> Result<Vec<DeclarationRange>, DataForgeError> {
    let ranks = ranks(entries)?;
    let mut ranges = Vec::new();
    source_layout_allocation::reserve(&mut ranges, entries.len(), "completion ranges")?;
    for (index, entry) in entries.iter().enumerate() {
        if entry.valid && !entry.fields.is_empty() {
            ranges.push(DeclarationRange {
                first: entry.fields.start,
                end: entry.fields.end,
                rank: *ranks.get(index).ok_or(DataForgeError::LayoutInvariant)?,
                base: i128::from(entry.inherited_count) - entry.fields.start as i128,
            });
        }
    }
    ranges.sort_unstable();
    Ok(ranges)
}

fn ordered(
    fields: impl ExactSizeIterator<Item = Field>,
    names: &[Box<str>],
) -> Result<Box<[Field]>, DataForgeError> {
    let mut ordered = Vec::new();
    source_layout_allocation::reserve(&mut ordered, fields.len(), "completion sort keys")?;
    for field in fields {
        let name: &str = names
            .get(field.name)
            .ok_or(DataForgeError::LayoutInvariant)?;
        ordered.push((field, name));
    }
    ordered.sort_unstable_by(|left, right| index::by_name(left.1, right.1));
    let mut result = Vec::new();
    source_layout_allocation::reserve(&mut result, ordered.len(), "completion field names")?;
    result.extend(ordered.into_iter().map(|(field, _)| field));
    Ok(result.into_boxed_slice())
}

fn ranks(entries: &[Entry]) -> Result<Vec<usize>, DataForgeError> {
    let mut ranks = Vec::new();
    let mut order = Vec::new();
    source_layout_allocation::reserve(&mut ranks, entries.len(), "completion ranks")?;
    source_layout_allocation::reserve(&mut order, entries.len(), "completion ancestry order")?;
    ranks.extend(0..entries.len());
    order.extend(
        entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.valid)
            .map(|(index, entry)| (entry.depth, index)),
    );
    order.sort_unstable();
    for (_, index) in order.into_iter().rev() {
        let entry = entries.get(index).ok_or(DataForgeError::LayoutInvariant)?;
        if let Some(parent) = entry.parent {
            let rank = *ranks.get(index).ok_or(DataForgeError::LayoutInvariant)?;
            let inherited = ranks
                .get_mut(parent)
                .ok_or(DataForgeError::LayoutInvariant)?;
            *inherited = (*inherited).min(rank);
        }
    }
    Ok(ranks)
}
