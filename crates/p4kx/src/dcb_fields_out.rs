// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_extraction::{CSV_BYTE_ORDER_MARK, CsvCellMode, FieldStat, write_csv_cell_with_mode};

use crate::dcb_fields::Listing;
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

/// Writes the listing as CSV using the selected character and spreadsheet policy.
pub(crate) fn csv(
    out: &mut impl Write,
    listing: &Listing<'_>,
    mode: CsvCellMode,
) -> io::Result<()> {
    if mode == CsvCellMode::Spreadsheet {
        out.write_all(CSV_BYTE_ORDER_MARK)?;
    }
    write_csv_row(out, &HEADER.map(str::to_owned), mode)?;
    for field in &listing.fields {
        write_csv_row(out, &cells(field), mode)?;
    }
    Ok(())
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
    mode: CsvCellMode,
) -> io::Result<()> {
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            out.write_all(b",")?;
        }
        let field_mode =
            if mode == CsvCellMode::Spreadsheet && !matches!(index, PATH_COLUMN | KINDS_COLUMN) {
                CsvCellMode::Raw
            } else {
                mode
            };
        write_csv_cell_with_mode(out, cell, field_mode)?;
    }
    out.write_all(b"\r\n")
}
