// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use p4kx_extraction::Value;
use p4kx_extraction::query::{QueryRun, RECORD_PATH_COLUMN, Row, write_short_text};

use crate::query_rows::{Counts, RowMode, for_each_row};
use crate::query_text_position::Positioned;

/// The gap between two columns of the table.
const GAP: &str = "  ";

type RowObserver<'a> = dyn FnMut(&Row<'_>, &[Range<u64>]) -> io::Result<()> + 'a;

/// Writes the result as a table whose columns fit every row: a first pass over the pages
/// measures the columns, a second one writes the rows; with `count_line`, the table ends with
/// the number of Records.
pub(crate) fn write(
    columns: &[String],
    run: &QueryRun<'_, '_>,
    out: impl Write,
    count_line: bool,
    counts: &mut Counts,
    mut observe: Option<&mut RowObserver<'_>>,
) -> io::Result<()> {
    let mut out = Positioned::new(out);
    let mut header = vec![one_line(RECORD_PATH_COLUMN)];
    header.extend(columns.iter().map(|column| one_line(column)));
    let widths = measure(run, &header);
    line(&mut out, &widths, &header, None)?;
    let rule: Vec<String> = widths.iter().map(|width| "-".repeat(*width)).collect();
    line(&mut out, &widths, &rule, None)?;
    let mode = if observe.is_some() {
        RowMode::Observed
    } else {
        RowMode::Raw
    };
    let mut spans = Vec::new();
    for_each_row(run, counts, mode, |row, cells| {
        let mut texts = Vec::with_capacity(cells.len().saturating_add(1));
        texts.push(one_line(row.record_path()));
        texts.extend(
            cells
                .iter()
                .map(|cell| cell.as_ref().map_or_else(String::new, cell_text)),
        );
        spans.clear();
        line(
            &mut out,
            &widths,
            &texts,
            observe.as_ref().map(|_| &mut spans),
        )?;
        if let Some(observer) = observe.as_mut() {
            observer(row, &spans)?;
        }
        Ok(())
    })?;
    if count_line {
        writeln!(out, "{}", counts.line(""))?;
    }
    out.flush()
}

/// Returns the width of each column: the widest of its heading and its texts in every row.
fn measure(run: &QueryRun<'_, '_>, header: &[String]) -> Vec<usize> {
    let mut widths: Vec<usize> = header.iter().map(|text| text.chars().count()).collect();
    for page in run.pages(p4kx_extraction::QUERY_PAGE_SIZE) {
        for row in &page {
            let Ok(cells) = row.cells() else {
                continue;
            };
            widen(&mut widths, 0, line_width(row.record_path()));
            for (index, cell) in cells.iter().enumerate() {
                if let Some(value) = cell {
                    let width = cell_text(value).chars().count();
                    widen(&mut widths, index.saturating_add(1), width);
                }
            }
        }
    }
    widths
}

fn widen(widths: &mut [usize], index: usize, width: usize) {
    if let Some(current) = widths.get_mut(index) {
        *current = (*current).max(width);
    }
}

/// Returns the short text of a cell on one line, a long list cut as in `docs/query.md`.
fn cell_text(value: &Value<'_>) -> String {
    let mut text = String::new();
    let _infallible = write_short_text(&mut text, value);
    one_line(&text)
}
pub(crate) use p4kx_extraction::text_line::one_line;

/// Returns the number of characters [`one_line`] makes of `text`.
fn line_width(text: &str) -> usize {
    text.chars().fold(0, |width: usize, c| {
        width.saturating_add(if matches!(c, '\n' | '\r' | '\t') {
            2
        } else {
            1
        })
    })
}

fn line(
    out: &mut Positioned<impl Write>,
    widths: &[usize],
    cells: &[String],
    mut spans: Option<&mut Vec<Range<u64>>>,
) -> io::Result<()> {
    let origin = out.position();
    let mut line = String::new();
    for (index, (width, text)) in widths.iter().zip(cells).enumerate() {
        if index > 0 {
            line.push_str(GAP);
        }
        let start = line.len();
        line.push_str(text);
        if let Some(spans) = spans.as_mut() {
            spans.push(start as u64..line.len() as u64);
        }
        let padding = width.saturating_sub(text.chars().count());
        line.extend(std::iter::repeat_n(' ', padding));
    }
    let text = line.trim_end();
    let end = text.len() as u64;
    if let Some(spans) = spans {
        for span in spans {
            span.start = table_position(origin, span.start.min(end))?;
            span.end = table_position(origin, span.end.min(end))?;
        }
    }
    writeln!(out, "{text}")
}

fn table_position(origin: u64, offset: u64) -> io::Result<u64> {
    origin
        .checked_add(offset)
        .ok_or_else(|| io::Error::other("query table position exceeds u64"))
}
