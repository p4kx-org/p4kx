// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::ops::Range;

use crate::Value;
use crate::flat_cells::{self, CSV_BYTE_ORDER_MARK, CsvCellMode};
use crate::value_json::{write_json_string, write_json_value};

/// The heading of the first column of every Query result, the record path.
pub const RECORD_PATH_COLUMN: &str = "record path";

/// A text format of a Query result, defined in `docs/query.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResultFormat {
    /// CSV for spreadsheets: after RFC 4180 with a header row, CRLF after every row and fields
    /// quoted where needed, starting with a UTF-8 byte order mark; a text cell that a spreadsheet
    /// would run as a formula starts with `'`. Numbers are never changed.
    Csv,
    /// CSV with the exact bytes of the values: no byte order mark and no `'`.
    CsvRaw,
    /// One line of JSON: an array with one object per row, keyed by column.
    Json,
    /// CSV with RFC 4180 ASCII fields, CRLF rows and no byte order mark.
    CsvStrict,
}

impl ResultFormat {
    const fn csv_mode(self) -> Option<CsvCellMode> {
        match self {
            Self::Csv => Some(CsvCellMode::Spreadsheet),
            Self::CsvRaw => Some(CsvCellMode::Raw),
            Self::CsvStrict => Some(CsvCellMode::Strict),
            Self::Json => None,
        }
    }
}

/// Writes a Query result row by row in a [`ResultFormat`], holding no more than one row.
#[derive(Debug)]
pub struct ResultWriter<W> {
    out: W,
    format: ResultFormat,
    columns: usize,
    keys: Vec<Option<Vec<u8>>>,
    rows: u64,
    text: String,
}

impl<W: Write> ResultWriter<W> {
    /// Starts a result whose columns after the record path have the headings `columns`, and
    /// writes the CSV header row or the opening bracket of the JSON array.
    ///
    /// # Errors
    ///
    /// Returns an I/O error or [`io::ErrorKind::InvalidData`] for unsupported strict CSV headings.
    pub fn new(out: W, format: ResultFormat, columns: &[String]) -> io::Result<Self> {
        let mut writer = Self {
            out,
            format,
            columns: columns.len(),
            keys: Vec::with_capacity(columns.len()),
            rows: 0,
            text: String::new(),
        };
        if let Some(mode) = format.csv_mode() {
            if mode == CsvCellMode::Spreadsheet {
                writer.out.write_all(CSV_BYTE_ORDER_MARK)?;
            }
            flat_cells::write_csv_cell_with_mode(&mut writer.out, RECORD_PATH_COLUMN, mode)?;
            for column in columns {
                writer.out.write_all(b",")?;
                flat_cells::write_csv_cell_with_mode(&mut writer.out, column, mode)?;
            }
            writer.out.write_all(b"\r\n")?;
        } else {
            let mut seen = HashSet::from([RECORD_PATH_COLUMN]);
            for column in columns {
                let key = if seen.insert(column.as_str()) {
                    let mut key = Vec::new();
                    write_json_string(&mut key, column)?;
                    key.push(b':');
                    Some(key)
                } else {
                    None
                };
                writer.keys.push(key);
            }
            writer.out.write_all(b"[")?;
        }
        Ok(writer)
    }

    /// Writes one row: its record path and one cell per column, `None` for an empty cell.
    ///
    /// A missing cell is written as empty and cells beyond the columns are left out.
    ///
    /// # Errors
    ///
    /// Returns an I/O error or [`io::ErrorKind::InvalidData`] for unsupported strict CSV data.
    pub fn write_row(&mut self, record_path: &str, cells: &[Option<Value<'_>>]) -> io::Result<()> {
        if let Some(mode) = self.format.csv_mode() {
            self.csv_row(record_path, cells, mode, |_, _| {})?;
        } else {
            self.json_row(record_path, cells)?;
        }
        self.rows = self.rows.saturating_add(1);
        Ok(())
    }

    /// Writes a CSV row and reports the exact byte interval for each field in column order.
    ///
    /// # Errors
    /// Returns a write error or invalid data when the selected format is not CSV.
    pub fn write_csv_row_observed(
        &mut self,
        record_path: &str,
        cells: &[Option<Value<'_>>],
        position: impl Fn(&W) -> u64,
        mut field: impl FnMut(Range<u64>),
    ) -> io::Result<()> {
        let mode = self.format.csv_mode().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "observed CSV row requires CSV format",
            )
        })?;
        let mut start = position(&self.out);
        self.csv_row(record_path, cells, mode, |start_of_field, writer| {
            if start_of_field {
                start = position(writer);
            } else {
                field(start..position(writer));
            }
        })?;
        self.rows = self.rows.saturating_add(1);
        Ok(())
    }

    /// Returns the column positions the JSON row writer retains.
    pub fn json_columns(&self) -> impl Iterator<Item = usize> + '_ {
        self.keys
            .iter()
            .enumerate()
            .filter_map(|(index, key)| key.as_ref().map(|_| index))
    }

    /// Returns the number of rows written.
    #[must_use]
    pub const fn rows(&self) -> u64 {
        self.rows
    }

    /// Returns the writer the result goes to.
    #[must_use]
    pub const fn get_ref(&self) -> &W {
        &self.out
    }

    /// Ends the result, flushes it and returns the writer it went to.
    ///
    /// # Errors
    ///
    /// Returns the [`io::Error`] of writing to `out`.
    pub fn finish(mut self) -> io::Result<W> {
        if self.format == ResultFormat::Json {
            self.out.write_all(b"]\n")?;
        }
        self.out.flush()?;
        Ok(self.out)
    }

    fn csv_row(
        &mut self,
        record_path: &str,
        cells: &[Option<Value<'_>>],
        mode: CsvCellMode,
        mut boundary: impl FnMut(bool, &W),
    ) -> io::Result<()> {
        boundary(true, &self.out);
        flat_cells::write_csv_cell_with_mode(&mut self.out, record_path, mode)?;
        boundary(false, &self.out);
        for index in 0..self.columns {
            self.out.write_all(b",")?;
            boundary(true, &self.out);
            if let Some(Some(value)) = cells.get(index) {
                let mut text = std::mem::take(&mut self.text);
                text.clear();
                let formatted = if matches!(value, Value::Array(_)) {
                    let mut json = Vec::new();
                    write_json_value(&mut json, value).and_then(|()| {
                        text.push_str(
                            std::str::from_utf8(&json)
                                .map_err(|_| io::Error::other("the JSON of a list is not UTF-8"))?,
                        );
                        Ok(())
                    })
                } else {
                    write!(text, "{value}")
                        .map_err(|_| io::Error::other("a value cannot be formatted"))
                };
                let cell_mode = match value {
                    Value::String(_) | Value::Locale(_) | Value::Enum(_) => mode,
                    _ => mode.verbatim(),
                };
                let written = formatted.and_then(|()| {
                    flat_cells::write_csv_cell_with_mode(&mut self.out, &text, cell_mode)
                });
                self.text = text;
                written?;
            } else {
                flat_cells::write_csv_cell_with_mode(&mut self.out, "", mode)?;
            }
            boundary(false, &self.out);
        }
        self.out.write_all(b"\r\n")
    }

    fn json_row(&mut self, record_path: &str, cells: &[Option<Value<'_>>]) -> io::Result<()> {
        if self.rows > 0 {
            self.out.write_all(b",")?;
        }
        self.out.write_all(b"{")?;
        write_json_string(&mut self.out, RECORD_PATH_COLUMN)?;
        self.out.write_all(b":")?;
        write_json_string(&mut self.out, record_path)?;
        for (index, key) in self.keys.iter().enumerate() {
            let Some(key) = key else {
                continue;
            };
            self.out.write_all(b",")?;
            self.out.write_all(key)?;
            match cells.get(index) {
                Some(Some(value)) => write_json_value(&mut self.out, value)?,
                _ => self.out.write_all(b"null")?,
            }
        }
        self.out.write_all(b"}")
    }
}
