// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::num::NonZeroUsize;
use std::ops::Range;
use std::slice::Chunks;

use rayon::prelude::*;

use crate::dataforge::out_of_range;
use crate::query::compile::Plan;
use crate::query::shared::Recordings;
use crate::query::source::{CellTrace, QueryCellSources};
use crate::query::state::WalkState;
use crate::query::walk::Place;
use crate::query::{CompiledQuery, MAX_PATH_VALUES};
use crate::{DataForgeError, Record, Value};

/// The Records a compiled Query selects, ordered by record path and then by definition order;
/// their rows are built page by page.
#[derive(Debug)]
pub struct QueryRun<'q, 'f> {
    query: &'q CompiledQuery<'f>,
    records: Vec<u32>,
}

impl<'f> CompiledQuery<'f> {
    /// Returns the run that selected `records`, the indexes [`QueryRun::indexes`] gave for this
    /// Query over the same DataForge; an index that is no Record gives no row.
    #[must_use]
    pub const fn resume(&self, records: Vec<u32>) -> QueryRun<'_, 'f> {
        QueryRun {
            query: self,
            records,
        }
    }

    pub(super) fn plan_of(&self, record: u32) -> Option<&Plan> {
        let def = self.forge.record_def(record)?;
        self.plans
            .get(usize::try_from(def.struct_index).ok()?)?
            .as_ref()
    }
}

impl<'f> QueryRun<'_, 'f> {
    /// Returns the number of selected Records, one row each.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns the indexes of the selected Records in row order, which [`CompiledQuery::resume`]
    /// turns into the run again.
    #[must_use]
    pub fn indexes(&self) -> &[u32] {
        &self.records
    }

    /// Returns whether no Record is selected.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns the record paths of the selected Records in row order.
    pub fn record_paths(&self) -> impl Iterator<Item = &'f str> + '_ {
        let forge = self.query.forge;
        self.records
            .iter()
            .filter_map(move |&record| forge.path_of(record))
    }

    /// Returns the rows at `range` of the row order, built in parallel; the part of `range` past
    /// the last row is left out.
    #[must_use]
    pub fn rows(&self, range: Range<usize>) -> Vec<Row<'f>> {
        let end = range.end.min(self.records.len());
        let start = range.start.min(end);
        self.query
            .rows_of(self.records.get(start..end).unwrap_or_default(), false)
    }

    /// Returns rows with the original inputs used by each selected cell.
    #[must_use]
    pub fn rows_observed(&self, range: Range<usize>) -> Vec<Row<'f>> {
        let end = range.end.min(self.records.len());
        let start = range.start.min(end);
        self.query
            .rows_of(self.records.get(start..end).unwrap_or_default(), true)
    }

    /// Returns the rows in pages of at most `page_size`; each page is built in parallel when it
    /// is reached.
    #[must_use]
    pub fn pages(&self, page_size: NonZeroUsize) -> Pages<'_, 'f> {
        Pages {
            query: self.query,
            chunks: self.records.chunks(page_size.get()),
        }
    }
}

/// The pages of a [`QueryRun`], in order.
#[derive(Debug)]
pub struct Pages<'r, 'f> {
    query: &'r CompiledQuery<'f>,
    chunks: Chunks<'r, u32>,
}

impl<'f> Iterator for Pages<'_, 'f> {
    type Item = Vec<Row<'f>>;

    fn next(&mut self) -> Option<Self::Item> {
        let chunk = self.chunks.next()?;
        Some(self.query.rows_of(chunk, false))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chunks.size_hint()
    }
}

/// One row of a Query result: a selected Record and its values, one per column.
#[derive(Debug, Clone)]
pub struct Row<'f> {
    record: Record<'f>,
    record_path: &'f str,
    cells: Result<Vec<Option<Value<'f>>>, DataForgeError>,
    unresolved: usize,
    visited_elements: usize,
    sources: Option<Vec<QueryCellSources>>,
}

impl<'f> Row<'f> {
    /// Returns the selected Record.
    #[must_use]
    pub const fn record(&self) -> Record<'f> {
        self.record
    }

    /// Returns the record path, the first column.
    #[must_use]
    pub const fn record_path(&self) -> &'f str {
        self.record_path
    }

    /// Returns the values of the columns after the record path; `None` where the Record's type
    /// has no such field.
    ///
    /// # Errors
    ///
    /// Returns the [`DataForgeError`] [`Record::tree`] would return when the Record's layout or
    /// instance cannot be read.
    pub fn cells(&self) -> Result<&[Option<Value<'f>>], &DataForgeError> {
        self.cells.as_deref()
    }

    /// Returns evidence by selected column position when built with original inputs.
    #[must_use]
    pub fn cell_sources(&self) -> Option<&[QueryCellSources]> {
        self.sources.as_deref()
    }

    /// Returns how many array elements the selected columns admitted during this row read.
    #[must_use]
    pub const fn visited_elements(&self) -> usize {
        self.visited_elements
    }

    /// Returns how many distinct links the columns followed with `->` to no Record or instance:
    /// null or dangling References and null weak pointers, each counted once however many
    /// columns reach it.
    #[must_use]
    pub const fn unresolved_references(&self) -> usize {
        self.unresolved
    }
}

/// What one worker keeps from row to row: the walk state and the buffers of the prefix
/// recordings.
struct Scratch<'f> {
    state: WalkState,
    recordings: Recordings<'f>,
}

impl Scratch<'_> {
    const fn new(limit: usize) -> Self {
        Self {
            state: WalkState::new(limit),
            recordings: Vec::new(),
        }
    }
}

impl<'f> CompiledQuery<'f> {
    fn rows_of(&self, records: &[u32], observe: bool) -> Vec<Row<'f>> {
        records
            .par_iter()
            .map_init(
                || Scratch::new(MAX_PATH_VALUES),
                |scratch, &record| {
                    let record_path = self.forge.path_of(record)?;
                    self.row(record, record_path, scratch, observe)
                },
            )
            .flatten()
            .collect()
    }

    /// Reads one explicitly selected physical Record with source coordinates.
    ///
    /// The caller supplies the row traversal budget. Missing filename metadata does not
    /// select away an otherwise readable physical Record.
    #[must_use]
    pub fn physical_row_observed(&self, index: u32, path_values: usize) -> Option<Row<'f>> {
        self.row(
            index,
            self.forge.path_of(index).unwrap_or_default(),
            &mut Scratch::new(path_values),
            true,
        )
    }

    fn row(
        &self,
        index: u32,
        record_path: &'f str,
        scratch: &mut Scratch<'f>,
        observe: bool,
    ) -> Option<Row<'f>> {
        let record = self.forge.record(index)?;
        let Scratch { state, recordings } = scratch;
        state.reset();
        let mut sources = observe.then(Vec::new);
        let cells = match self.plan_of(index)? {
            _ if self.columns.is_empty() && self.predicate.is_none() => Ok(Vec::new()),
            Plan::Broken(error) => Err(error.clone()),
            Plan::Read {
                columns, shared, ..
            } => self.instance_of(index).map(|place| {
                let groups = self.share_prefixes.then_some(shared);
                if let Some(groups) = groups {
                    groups.reset(recordings, observe);
                }
                columns
                    .iter()
                    .enumerate()
                    .map(|(at, column)| {
                        let mut trace = observe.then(CellTrace::default);
                        let value = column.as_ref().map(|reader| {
                            let prefix = groups.and_then(|groups| {
                                groups.prefix_of(at, reader, self.forge, place, recordings)
                            });
                            reader.cell(self.forge, place, state, prefix, trace.as_mut())
                        });
                        if let Some((sources, trace)) = sources.as_mut().zip(trace) {
                            sources.push(trace.finish());
                        }
                        value
                    })
                    .collect()
            }),
        };
        Some(Row {
            record,
            record_path,
            cells,
            unresolved: state.unresolved(),
            visited_elements: state.visited(),
            sources,
        })
    }

    /// Returns the Record's instance, which the Record tree reads its values from.
    pub(super) fn instance_of(&self, index: u32) -> Result<Place<'f>, DataForgeError> {
        let forge = self.forge;
        let def = forge
            .record_def(index)
            .ok_or_else(|| out_of_range("Record", index as usize, "Record", index.into()))?;
        let instance = u32::from(def.instance_index);
        let bytes = forge
            .instance(def.struct_index, instance)
            .ok_or_else(|| out_of_range("Record", index as usize, "instance", instance.into()))?;
        Ok(Place::Instance {
            struct_index: def.struct_index,
            bytes,
        })
    }
}
