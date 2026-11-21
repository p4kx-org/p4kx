// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Statistics per field path over the Records of a DataForge, defined in `docs/query.md`.
//!
//! [`FieldStatistics::of_all`] and [`FieldStatistics::of_records`] walk the Records in parallel
//! once and count, for every path, how often it occurs, in how many Records, of which kinds its
//! values are, how many are empty, how many distinct values it has (up to a cap), and the
//! smallest and largest number. The result is the same for every number of threads.

mod layouts;
mod navigate;
mod number;
mod part;
mod seen;
#[cfg(test)]
mod tests;
mod trie;
mod walk;
mod walk_frame;

use std::cmp::Ordering;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering as Atomic};

use rayon::prelude::*;
use thiserror::Error;

use crate::DataForge;
use crate::DataType;
use crate::fields::layouts::SharedLayouts;
use crate::fields::part::Part;
use crate::fields::trie::ROOT;
pub use number::{MAX_DISTINCT_VALUES, Number};

/// The most distinct field paths the statistics keep; a DataForge with more is an error, so
/// that the memory stays bounded.
pub const MAX_FIELD_PATHS: usize = 1 << 20;

/// The statistics cannot be computed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum FieldStatisticsError {
    /// A source-layout operation failed before complete statistics could be produced.
    #[error("field statistics could not read a layout: {source}")]
    Read {
        /// The layout operation that failed.
        #[source]
        source: crate::DataForgeError,
    },
    /// The Records have more distinct field paths than [`MAX_FIELD_PATHS`].
    #[error("the Records have more than {limit} distinct field paths")]
    TooManyPaths {
        /// The most paths kept.
        limit: usize,
    },
}

/// The kind of the values at a path: a data type, and whether they are array elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldKind {
    data_type: DataType,
    array: bool,
}

impl FieldKind {
    /// Creates the kind of the values of `data_type`, array elements if `array`.
    #[must_use]
    pub const fn new(data_type: DataType, array: bool) -> Self {
        Self { data_type, array }
    }

    /// Returns the data type of the values; [`DataType::Class`] for the value of a path that
    /// holds fields: an inline struct or the struct a strong pointer owns.
    #[must_use]
    pub const fn data_type(&self) -> DataType {
        self.data_type
    }

    /// Returns whether the values are elements of an array field.
    #[must_use]
    pub const fn is_array(&self) -> bool {
        self.array
    }

    const fn rank(self) -> (u8, bool) {
        (self.data_type as u8, self.array)
    }
}

impl fmt::Display for FieldKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.array {
            write!(f, "{}[]", self.data_type)
        } else {
            self.data_type.fmt(f)
        }
    }
}

/// What the Records show about one field path.
///
/// A path is the field names from the Record down, joined by `/`, with no array positions; the
/// target of a strong pointer adds the name of its actual struct as a segment, e.g.
/// `Components/SCItemWeaponComponentParams/fireActions`.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldStat {
    path: String,
    occurrences: u64,
    records: u64,
    empty: u64,
    kinds: Box<[(FieldKind, u64)]>,
    distinct: usize,
    distinct_capped: bool,
    min: Option<Number>,
    max: Option<Number>,
}

impl FieldStat {
    /// Returns the path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns how many values the path has: one per field, one per array element. An empty
    /// array has none.
    #[must_use]
    pub const fn occurrences(&self) -> u64 {
        self.occurrences
    }

    /// Returns in how many Records the path occurs.
    #[must_use]
    pub const fn records(&self) -> u64 {
        self.records
    }

    /// Returns how many of the values are empty: a null pointer or Reference, an empty text or a
    /// null GUID. They are values of their path but add to no number, distinct value, minimum or
    /// maximum.
    #[must_use]
    pub const fn empty(&self) -> u64 {
        self.empty
    }

    /// Returns the kinds of the values with how many each has, in the order of the data types.
    /// One kind unless Records of several types share the path.
    #[must_use]
    pub fn kinds(&self) -> &[(FieldKind, u64)] {
        &self.kinds
    }

    fn holds(&self, data_type: DataType) -> bool {
        self.kinds
            .iter()
            .any(|(kind, _)| kind.data_type() == data_type)
    }

    fn holds_array(&self) -> bool {
        self.kinds.iter().any(|(kind, _)| kind.is_array())
    }

    /// Returns how many distinct non-empty values the path has, at most [`MAX_DISTINCT_VALUES`].
    /// The values of a Reference are the GUIDs it links to; strong and weak pointers and inline
    /// structs have none, so the count is 0.
    ///
    /// Values are told apart by a 64-bit fingerprint, so the count is exact up to a collision of
    /// two fingerprints, which has a chance of about 2^-64 per pair of values.
    #[must_use]
    pub const fn distinct(&self) -> usize {
        self.distinct
    }

    /// Returns whether the path has more than [`MAX_DISTINCT_VALUES`] distinct values.
    #[must_use]
    pub const fn distinct_capped(&self) -> bool {
        self.distinct_capped
    }

    /// Returns the smallest number, or `None` for a path without numbers. `-0.0` is `0.0` and
    /// NaN is no number.
    #[must_use]
    pub const fn min(&self) -> Option<Number> {
        self.min
    }

    /// Returns the largest number, or `None` for a path without numbers.
    #[must_use]
    pub const fn max(&self) -> Option<Number> {
        self.max
    }
}

/// The statistics of the field paths of some Records, sorted by path.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FieldStatistics {
    fields: Vec<FieldStat>,
    records: u64,
    unreadable_records: u64,
    unreadable_values: u64,
}

impl FieldStatistics {
    /// Computes the statistics of every Record of `forge`, in parallel.
    ///
    /// # Errors
    ///
    /// Returns [`FieldStatisticsError::TooManyPaths`] when the Records have more than
    /// [`MAX_FIELD_PATHS`] distinct paths.
    pub fn of_all<'f>(forge: &'f DataForge<'f>) -> Result<Self, FieldStatisticsError> {
        Self::of_records(forge, forge.path_order())
    }

    /// Computes the statistics of the Records at `records`, the indexes
    /// [`QueryRun::indexes`](crate::query::QueryRun::indexes) gives, in parallel; an index that is
    /// no Record counts as unreadable.
    ///
    /// # Errors
    ///
    /// Returns the error of [`of_all`](Self::of_all).
    pub fn of_records<'f>(
        forge: &'f DataForge<'f>,
        records: &[u32],
    ) -> Result<Self, FieldStatisticsError> {
        Ok(Self::of_records_while(forge, records, || true)?.unwrap_or_default())
    }

    /// Computes the statistics as [`of_records`](Self::of_records) does, asking `proceed` before
    /// every Record; returns `Ok(None)` as soon as it has answered `false`.
    ///
    /// # Errors
    ///
    /// Returns the error of [`of_all`](Self::of_all).
    pub fn of_records_while<'f>(
        forge: &'f DataForge<'f>,
        records: &[u32],
        proceed: impl Fn() -> bool + Sync,
    ) -> Result<Option<Self>, FieldStatisticsError> {
        let stopped = AtomicBool::new(false);
        let layouts = SharedLayouts::new(forge);
        let part = records
            .par_iter()
            .with_min_len(min_part_len(records.len(), rayon::current_num_threads()))
            .fold(
                || Ok(Part::new(&layouts)),
                |part: Result<Part<'_, 'f>, FieldStatisticsError>, &record| {
                    let mut part = part?;
                    if stopped.load(Atomic::Relaxed) {
                        return Ok(part);
                    }
                    if !proceed() {
                        stopped.store(true, Atomic::Relaxed);
                        return Ok(part);
                    }
                    part.add(forge, record)?;
                    Ok(part)
                },
            )
            .reduce(
                || Ok(Part::new(&layouts)),
                |left, right| left.and_then(|left| left.merge(right?)),
            )?;
        Ok((!stopped.load(Atomic::Relaxed)).then(|| Self::from_part(part)))
    }

    fn from_part(part: Part<'_, '_>) -> Self {
        let Part {
            trie,
            examined,
            unreadable_records,
            unreadable_values,
            ..
        } = part;
        let mut paths: Vec<String> = Vec::with_capacity(trie.nodes.len());
        for node in &trie.nodes {
            let parent = paths.get(node.parent as usize).map_or("", String::as_str);
            let mut path = String::with_capacity(parent.len() + 1 + node.segment.len());
            if node.parent != ROOT {
                path.push_str(parent);
                path.push('/');
            }
            path.push_str(node.segment);
            paths.push(path);
        }
        let mut fields: Vec<FieldStat> = trie
            .nodes
            .into_iter()
            .zip(paths)
            .skip(1)
            .filter(|(node, _)| node.occurrences > 0)
            .map(|(node, path)| {
                let mut kinds = node.kinds;
                kinds.sort_by_key(|(kind, _)| kind.rank());
                let extremes = node.extremes;
                FieldStat {
                    path,
                    occurrences: node.occurrences,
                    records: node.records,
                    empty: node.empty,
                    kinds: kinds.into_boxed_slice(),
                    distinct: node
                        .distinct
                        .as_ref()
                        .map_or(0, |distinct| distinct.count()),
                    distinct_capped: node
                        .distinct
                        .as_ref()
                        .is_some_and(|distinct| distinct.is_capped()),
                    min: extremes.as_ref().map(|extremes| extremes.min),
                    max: extremes.as_ref().map(|extremes| extremes.max),
                }
            })
            .collect();
        fields.par_sort_by(|a, b| order(&a.path, &b.path));
        Self {
            fields,
            records: examined,
            unreadable_records,
            unreadable_values,
        }
    }

    /// Returns the paths with their statistics, sorted by path: segment by segment ignoring ASCII
    /// case, so a path is followed by the paths below it.
    #[must_use]
    pub fn fields(&self) -> &[FieldStat] {
        &self.fields
    }

    /// Returns the statistics of `path`, if any Record has it.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&FieldStat> {
        self.fields
            .binary_search_by(|field| order(&field.path, path))
            .ok()
            .and_then(|at| self.fields.get(at))
    }

    /// Returns how many Records were examined.
    #[must_use]
    pub const fn records(&self) -> u64 {
        self.records
    }

    /// Returns how many requested Record positions or root instances/layouts could not be read.
    #[must_use]
    pub const fn unreadable_records(&self) -> u64 {
        self.unreadable_records
    }

    /// Returns the number of unreadable scalar occurrences and rejected nested source edges.
    ///
    /// Bad arrays, missing instances, unreadable layouts, repeated owners and inline cycles count.
    #[must_use]
    pub const fn unreadable_values(&self) -> u64 {
        self.unreadable_values
    }
}

/// Returns how many Records each fold part takes at least, so that `threads` parts at most are
/// made and each keeps its own trie.
const fn min_part_len(records: usize, threads: usize) -> usize {
    let len = records.div_ceil(if threads == 0 { 1 } else { threads });
    if len == 0 { 1 } else { len }
}

/// Orders paths segment by segment, ignoring ASCII case and then by bytes.
fn order(a: &str, b: &str) -> Ordering {
    let mut left = a.split('/');
    let mut right = b.split('/');
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let by_case = x
                    .bytes()
                    .map(|byte| byte.to_ascii_lowercase())
                    .cmp(y.bytes().map(|byte| byte.to_ascii_lowercase()))
                    .then_with(|| x.cmp(y));
                if by_case != Ordering::Equal {
                    return by_case;
                }
            }
        }
    }
}
