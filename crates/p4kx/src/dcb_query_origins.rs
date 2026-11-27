// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{self, Write};
use std::ops::Range;
use std::slice;

use p4kx_extraction::query::{
    CompiledQuery, QueryCellSources, QueryRun, RECORD_PATH_COLUMN, ResultFormat, ResultWriter, Row,
};
use p4kx_extraction::query_source_evidence::{
    QueryCellEvidence, QuerySourceSite, bind_cell_inputs, bind_row_root_inputs,
};
use p4kx_extraction::{
    DecimalU64, FieldPath, InputScope, OriginArtifactBinding, OriginCompanionSpool,
    OriginOutputAddress, SourceInput, SourceScope, SourceUse, SourceUseRole, StagedOriginCompanion,
    Value,
};

use crate::query_rows::{Counts, RowMode, for_each_row};
use crate::query_table;
use crate::query_text_position::Positioned;

#[derive(Clone, Copy)]
pub(super) enum Format {
    Table { count_line: bool },
    Flat(ResultFormat),
}

pub(super) struct Publication {
    companion: RefCell<StagedOriginCompanion>,
}

impl Publication {
    pub(super) fn write(
        &self,
        out: &mut dyn Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        self.companion.borrow_mut().write(out, binding)
    }
}

pub(super) fn render(
    query: &CompiledQuery<'_>,
    run: &QueryRun<'_, '_>,
    source: SourceInput<'_>,
    format: Format,
    out: &mut dyn Write,
    counts: &mut Counts,
) -> io::Result<Publication> {
    let mut capture = Capture::new(query, source)?;
    match format {
        Format::Table { count_line } => {
            let mut ordinal = 0;
            query_table::write(
                query.columns(),
                run,
                out,
                count_line,
                counts,
                Some(&mut |row, spans| {
                    capture.text_row(ordinal, row, spans)?;
                    ordinal += 1;
                    Ok(())
                }),
            )?;
        }
        Format::Flat(ResultFormat::Json) => {
            render_json(&mut capture, run, out, counts)?;
        }
        Format::Flat(csv) => {
            render_csv(&mut capture, run, csv, out, counts)?;
        }
    }
    capture.finish()
}

fn render_json(
    capture: &mut Capture<'_, '_>,
    run: &QueryRun<'_, '_>,
    out: &mut dyn Write,
    counts: &mut Counts,
) -> io::Result<()> {
    let mut writer = ResultWriter::new(out, ResultFormat::Json, capture.query.columns())?;
    let columns = writer.json_columns().collect::<Vec<_>>();
    for_each_row(run, counts, RowMode::Observed, |row, values| {
        let ordinal = usize::try_from(writer.rows()).map_err(io::Error::other)?;
        writer.write_row(row.record_path(), values)?;
        capture.json_row(ordinal, row, values, &columns)
    })?;
    writer.finish().map(drop)
}

fn render_csv(
    capture: &mut Capture<'_, '_>,
    run: &QueryRun<'_, '_>,
    format: ResultFormat,
    out: &mut dyn Write,
    counts: &mut Counts,
) -> io::Result<()> {
    let out = Positioned::new(out);
    let mut writer = ResultWriter::new(out, format, capture.query.columns())?;
    let mut spans = Vec::with_capacity(capture.query.columns().len().saturating_add(1));
    for_each_row(run, counts, RowMode::Observed, |row, values| {
        let ordinal = usize::try_from(writer.rows()).map_err(io::Error::other)?;
        spans.clear();
        writer.write_csv_row_observed(row.record_path(), values, Positioned::position, |span| {
            spans.push(span);
        })?;
        capture.text_row(ordinal, row, &spans)
    })?;
    writer.finish().map(drop)
}

struct Capture<'query, 'source> {
    query: &'query CompiledQuery<'source>,
    scope: SourceScope,
    scope_complete: bool,
    spool: OriginCompanionSpool,
}

impl<'query, 'source> Capture<'query, 'source> {
    fn new(query: &'query CompiledQuery<'source>, source: SourceInput<'_>) -> io::Result<Self> {
        let spool = OriginCompanionSpool::new()?;
        let scope_complete = match &source {
            SourceInput::Archive(entry) => entry.ancestors().all(|step| step.is_some()),
            SourceInput::File(_) => true,
            _ => false,
        };
        let scope = spool
            .sources()
            .observe(source, InputScope::Payload)
            .map_err(io::Error::other)?;
        Ok(Self {
            query,
            scope,
            scope_complete,
            spool,
        })
    }

    fn finish(self) -> io::Result<Publication> {
        Ok(Publication {
            companion: RefCell::new(self.spool.finish()?),
        })
    }

    fn json_row(
        &mut self,
        ordinal: usize,
        row: &Row<'_>,
        values: &[Option<Value<'_>>],
        columns: &[usize],
    ) -> io::Result<()> {
        let root = self.row_root(row)?;
        let record_path = self.record_path(row)?;
        let mut path = FieldPath::root();
        path.push_index(ordinal).map_err(io::Error::other)?;
        path.push_field(RECORD_PATH_COLUMN)
            .map_err(io::Error::other)?;
        self.push_json(&path, slice::from_ref(&record_path), true)?;
        path.pop();
        let traces = row.cell_sources().unwrap_or_default();
        for &column in columns {
            let name = self.query.columns().get(column).ok_or_else(|| {
                io::Error::other("query writer retained a column outside the original query")
            })?;
            path.push_field(name).map_err(io::Error::other)?;
            self.json_cell(
                &mut path,
                values.get(column).and_then(Option::as_ref),
                traces.get(column),
                &root,
            )?;
            path.pop();
        }
        self.spool.finish_batch()
    }

    fn json_cell(
        &mut self,
        path: &mut FieldPath,
        value: Option<&Value<'_>>,
        cell: Option<&QueryCellSources>,
        root: &QueryCellEvidence,
    ) -> io::Result<()> {
        let bound = self.cell_inputs(cell, root)?;
        let routes = cell.map_or(&[][..], QueryCellSources::values);
        let matching = value.is_none_or(
            |value| !matches!(value, Value::Array(items) if routes.len() != items.len()),
        );
        self.push_json(path, bound.uses(), matching && bound.is_complete())?;
        match value {
            Some(Value::Array(items)) => {
                for (ordinal, item) in items.iter().enumerate() {
                    let route = if matching {
                        routes.get(ordinal).map_or_default(AsRef::as_ref)
                    } else {
                        &[]
                    };
                    let item_bound = bind_cell_inputs(
                        self.query.dataforge(),
                        &self.scope,
                        route.iter().copied(),
                    )
                    .map_err(io::Error::other)?
                    .with_row_root(root);
                    path.push_index(ordinal).map_err(io::Error::other)?;
                    self.push_json(path, item_bound.uses(), item_bound.is_complete())?;
                    self.json_error(path, item, &item_bound)?;
                    path.pop();
                }
            }
            Some(value) => self.json_error(path, value, &bound)?,
            None => {}
        }
        Ok(())
    }

    fn text_row(&mut self, ordinal: usize, row: &Row<'_>, spans: &[Range<u64>]) -> io::Result<()> {
        let root = self.row_root(row)?;
        let record_path = self.record_path(row)?;
        let mut path = FieldPath::root();
        path.push_index(ordinal).map_err(io::Error::other)?;
        let traces = row.cell_sources().unwrap_or_default();
        for (column, span) in spans.iter().enumerate() {
            path.push_index(column).map_err(io::Error::other)?;
            let address = OriginOutputAddress::TextBytes {
                field: path.clone(),
                start: DecimalU64::new(span.start),
                end_exclusive: DecimalU64::new(span.end),
            };
            if let Some(ordinal) = column.checked_sub(1) {
                let bound = self.cell_inputs(traces.get(ordinal), &root)?;
                self.push(&address, bound.uses(), bound.is_complete())?;
            } else {
                self.push(&address, slice::from_ref(&record_path), true)?;
            }
            path.pop();
        }
        self.spool.finish_batch()
    }

    fn row_root(&self, row: &Row<'_>) -> io::Result<QueryCellEvidence> {
        bind_row_root_inputs(self.query.dataforge(), &self.scope, row).map_err(io::Error::other)
    }

    fn cell_inputs(
        &self,
        cell: Option<&QueryCellSources>,
        root: &QueryCellEvidence,
    ) -> io::Result<QueryCellEvidence> {
        bind_cell_inputs(
            self.query.dataforge(),
            &self.scope,
            cell.into_iter().flat_map(QueryCellSources::traversed),
        )
        .map(|bound| bound.with_row_root(root))
        .map_err(io::Error::other)
    }

    fn record_path(&self, row: &Row<'_>) -> io::Result<SourceUse> {
        QuerySourceSite::record(row.record().index(), "fileName", SourceUseRole::Conversion)
            .bind(&self.scope)
            .map_err(io::Error::other)
    }

    fn json_error(
        &mut self,
        path: &mut FieldPath,
        item: &Value<'_>,
        bound: &QueryCellEvidence,
    ) -> io::Result<()> {
        if let Value::Error(error) = item {
            path.push_field("error").map_err(io::Error::other)?;
            self.push_json(path, bound.uses(), bound.is_complete())?;
            path.pop();
            if error.dangling_guid().is_some() {
                path.push_field("guid").map_err(io::Error::other)?;
                self.push_json(path, bound.uses(), bound.is_complete())?;
                path.pop();
            }
        }
        Ok(())
    }

    fn push_json(
        &mut self,
        path: &FieldPath,
        uses: &[SourceUse],
        complete: bool,
    ) -> io::Result<()> {
        let address = OriginOutputAddress::JsonField { path: path.clone() };
        self.push(&address, uses, complete)
    }

    fn push(
        &mut self,
        address: &OriginOutputAddress,
        uses: &[SourceUse],
        complete: bool,
    ) -> io::Result<()> {
        self.spool
            .push(address, uses, complete && self.scope_complete)
    }
}
