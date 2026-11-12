// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use p4kx_extraction::{
    CSV_BYTE_ORDER_MARK, FieldStat, MAX_DISTINCT_VALUES, Number, write_csv_cell,
};
use serde::Serialize;

use crate::dcb_fields::Listing;
use crate::json;
use crate::query_table::one_line;

const GAP: &str = "  ";
const COLUMNS: usize = 8;
const PATH_COLUMN: usize = 0;
const KINDS_COLUMN: usize = 1;
const HEADER: [&str; COLUMNS] = [
    "path", "kinds", "values", "records", "empty", "distinct", "min", "max",
];

/// Writes the listing as an aligned table ending with the number of paths and Records.
pub(crate) fn table(out: &mut impl Write, listing: &Listing<'_>) -> io::Result<()> {
    let rows: Vec<[String; COLUMNS]> = listing.fields.iter().map(|field| cells(field)).collect();
    let mut widths = HEADER.map(|text| text.chars().count());
    for row in &rows {
        for (width, text) in widths.iter_mut().zip(row) {
            *width = (*width).max(text.chars().count());
        }
    }
    line(out, &widths, &HEADER.map(str::to_owned))?;
    line(out, &widths, &widths.map(|width| "-".repeat(width)))?;
    for row in &rows {
        line(out, &widths, row)?;
    }
    let paths = listing.fields.len();
    let records = listing.statistics.records();
    writeln!(
        out,
        "{paths} {} in {records} {}",
        if paths == 1 { "path" } else { "paths" },
        if records == 1 { "Record" } else { "Records" },
    )
}

/// Writes the listing as CSV after RFC 4180 with a header row; with `neutralise`, it starts with
/// a byte order mark and a path or kind that a spreadsheet would run as a formula starts with `'`.
pub(crate) fn csv(out: &mut impl Write, listing: &Listing<'_>, neutralise: bool) -> io::Result<()> {
    if neutralise {
        out.write_all(CSV_BYTE_ORDER_MARK)?;
    }
    write_csv_row(out, &HEADER.map(str::to_owned), neutralise)?;
    for field in &listing.fields {
        write_csv_row(out, &cells(field), neutralise)?;
    }
    Ok(())
}

/// Writes the listing as one line of JSON, defined in docs/cli-json.md.
pub(crate) fn json(out: &mut impl Write, archive: &Path, listing: &Listing<'_>) -> io::Result<()> {
    let result = ResultJson {
        archive: json::path(archive),
        records: listing.statistics.records(),
        unreadable_records: listing.statistics.unreadable_records(),
        unreadable_values: listing.statistics.unreadable_values(),
        max_distinct: MAX_DISTINCT_VALUES,
        field_count: listing.fields.len(),
        fields: listing
            .fields
            .iter()
            .map(|field| FieldJson::of(field))
            .collect(),
    };
    serde_json::to_writer(&mut *out, &result)?;
    writeln!(out)
}

#[derive(Serialize)]
struct ResultJson<'a> {
    archive: String,
    records: u64,
    unreadable_records: u64,
    unreadable_values: u64,
    max_distinct: usize,
    field_count: usize,
    fields: Vec<FieldJson<'a>>,
}

#[derive(Serialize)]
struct FieldJson<'a> {
    path: &'a str,
    occurrences: u64,
    records: u64,
    empty: u64,
    kinds: Vec<KindJson>,
    distinct: usize,
    distinct_capped: bool,
    min: Option<NumberJson>,
    max: Option<NumberJson>,
}

#[derive(Serialize)]
struct KindJson {
    kind: String,
    count: u64,
}

#[derive(Serialize)]
#[serde(untagged)]
enum NumberJson {
    Signed(i64),
    Unsigned(u64),
    Float(f64),
}

impl<'a> FieldJson<'a> {
    fn of(field: &'a FieldStat) -> Self {
        Self {
            path: field.path(),
            occurrences: field.occurrences(),
            records: field.records(),
            empty: field.empty(),
            kinds: field
                .kinds()
                .iter()
                .map(|(kind, count)| KindJson {
                    kind: kind.to_string(),
                    count: *count,
                })
                .collect(),
            distinct: field.distinct(),
            distinct_capped: field.distinct_capped(),
            min: field.min().and_then(NumberJson::of),
            max: field.max().and_then(NumberJson::of),
        }
    }
}

impl NumberJson {
    /// Returns the number as JSON, or `None` for an infinite float, which JSON cannot hold.
    fn of(number: Number) -> Option<Self> {
        match number {
            Number::Integer(value) => i64::try_from(value)
                .map(Self::Signed)
                .or_else(|_| u64::try_from(value).map(Self::Unsigned))
                .ok(),
            other => other
                .to_string()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .map(Self::Float),
        }
    }
}

fn cells(field: &FieldStat) -> [String; COLUMNS] {
    let kinds: Vec<String> = field
        .kinds()
        .iter()
        .map(|(kind, _)| kind.to_string())
        .collect();
    let distinct = if field.distinct_capped() {
        format!("{}+", field.distinct())
    } else {
        field.distinct().to_string()
    };
    [
        one_line(field.path()),
        kinds.join(", "),
        field.occurrences().to_string(),
        field.records().to_string(),
        field.empty().to_string(),
        distinct,
        field.min().map_or_default(|number| number.to_string()),
        field.max().map_or_default(|number| number.to_string()),
    ]
}

fn line(
    out: &mut impl Write,
    widths: &[usize; COLUMNS],
    cells: &[String; COLUMNS],
) -> io::Result<()> {
    let mut text = String::new();
    for (index, (width, cell)) in widths.iter().zip(cells).enumerate() {
        if index > 0 {
            text.push_str(GAP);
        }
        text.push_str(cell);
        text.extend(std::iter::repeat_n(
            ' ',
            width.saturating_sub(cell.chars().count()),
        ));
    }
    writeln!(out, "{}", text.trim_end())
}

fn write_csv_row(
    out: &mut impl Write,
    cells: &[String; COLUMNS],
    neutralise: bool,
) -> io::Result<()> {
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            out.write_all(b",")?;
        }
        let text = matches!(index, PATH_COLUMN | KINDS_COLUMN);
        write_csv_cell(out, cell, neutralise && text)?;
    }
    out.write_all(b"\r\n")
}
