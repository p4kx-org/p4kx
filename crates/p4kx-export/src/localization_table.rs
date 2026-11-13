// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::ops::Range;

use p4kx_extraction::{CSV_BYTE_ORDER_MARK, Language, TextSet, decode_line_breaks, write_csv_cell};

/// The file format of a localisation table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TableFormat {
    /// One JSON object with language codes and one object per key.
    Json,
    /// CSV with a UTF-8 BOM, CRLF and formula neutralisation.
    Csv,
    /// CSV with a UTF-8 BOM, CRLF and original keys and texts.
    CsvRaw,
}

/// An addressed datum emitted by the localisation table writer.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum LocalizationTableDatum<'text> {
    /// The tool-defined CSV column title.
    KeyHeader,
    /// A language code in the table header.
    Language {
        /// Zero-based position among the requested languages.
        column: usize,
        /// The language of the column.
        language: Language,
    },
    /// The original key at the specified row.
    Key {
        /// Zero-based data row position.
        row: usize,
        /// Borrowed original key spelling.
        key: &'text str,
    },
    /// A translation cell, including an explicit absence.
    Text {
        /// Zero-based data row position.
        row: usize,
        /// Borrowed lookup key spelling.
        key: &'text str,
        /// The language used for the lookup.
        language: Language,
        /// The original stored text, if present.
        stored: Option<&'text str>,
    },
}

/// Writes the sorted localisation table, retaining the difference between empty and missing JSON cells.
///
/// # Errors
/// Returns the first output error.
pub fn write_localization_table(
    texts: &TextSet<'_>,
    languages: &[Language],
    format: TableFormat,
    out: &mut impl Write,
) -> io::Result<usize> {
    write_observed_localization_table(texts, languages, format, out, |_, _| Ok(()))
}

/// Writes the table and reports each emitted datum with its half-open output byte range.
///
/// # Errors
/// Returns the first output or observation error.
pub fn write_observed_localization_table(
    texts: &TextSet<'_>,
    languages: &[Language],
    format: TableFormat,
    out: &mut dyn Write,
    observe: impl FnMut(LocalizationTableDatum<'_>, Range<u64>) -> io::Result<()>,
) -> io::Result<usize> {
    let rows = texts.keys().into_iter().filter(|key| {
        languages
            .iter()
            .any(|&language| texts.text(key, language).is_some())
    });
    let mut table = TableWriter {
        out,
        position: 0,
        observe,
    };
    match format {
        TableFormat::Json => write_json(texts, languages, rows, &mut table),
        TableFormat::Csv => write_csv(texts, languages, rows, &mut table, true),
        TableFormat::CsvRaw => write_csv(texts, languages, rows, &mut table, false),
    }
}

fn write_json<'a>(
    texts: &TextSet<'_>,
    languages: &[Language],
    rows: impl Iterator<Item = &'a str>,
    table: &mut TableWriter<
        '_,
        impl FnMut(LocalizationTableDatum<'_>, Range<u64>) -> io::Result<()>,
    >,
) -> io::Result<usize> {
    table.write_all(b"{\"languages\":[")?;
    for (column, &language) in languages.iter().enumerate() {
        if column > 0 {
            table.write_all(b",")?;
        }
        table.datum(
            LocalizationTableDatum::Language { column, language },
            |out| Ok(serde_json::to_writer(out, language.code())?),
        )?;
    }
    table.write_all(b"],\"entries\":[")?;
    let mut count = 0;
    for (row, key) in rows.enumerate() {
        table.write_all(if row == 0 { b"\n" } else { b",\n" })?;
        table.write_all(b"{\"key\":")?;
        table.datum(LocalizationTableDatum::Key { row, key }, |out| {
            Ok(serde_json::to_writer(out, key)?)
        })?;
        table.write_all(b",\"texts\":{")?;
        for (column, &language) in languages.iter().enumerate() {
            if column > 0 {
                table.write_all(b",")?;
            }
            serde_json::to_writer(&mut *table, language.code())?;
            table.write_all(b":")?;
            let stored = texts.text(key, language);
            table.datum(
                LocalizationTableDatum::Text {
                    row,
                    key,
                    language,
                    stored,
                },
                |out| match stored {
                    Some(text) => Ok(serde_json::to_writer(out, &decode_line_breaks(text))?),
                    None => out.write_all(b"null"),
                },
            )?;
        }
        table.write_all(b"}}")?;
        count += 1;
    }
    table.write_all(if count == 0 { b"]}\n" } else { b"\n]}\n" })?;
    Ok(count)
}

fn write_csv<'a>(
    texts: &TextSet<'_>,
    languages: &[Language],
    rows: impl Iterator<Item = &'a str>,
    table: &mut TableWriter<
        '_,
        impl FnMut(LocalizationTableDatum<'_>, Range<u64>) -> io::Result<()>,
    >,
    neutralise: bool,
) -> io::Result<usize> {
    table.write_all(CSV_BYTE_ORDER_MARK)?;
    table.datum(LocalizationTableDatum::KeyHeader, |out| {
        out.write_all(b"key")
    })?;
    for (column, &language) in languages.iter().enumerate() {
        table.write_all(b",")?;
        table.datum(
            LocalizationTableDatum::Language { column, language },
            |out| out.write_all(language.code().as_bytes()),
        )?;
    }
    table.write_all(b"\r\n")?;
    let mut count = 0;
    for (row, key) in rows.enumerate() {
        table.datum(LocalizationTableDatum::Key { row, key }, |out| {
            write_csv_cell(out, key, neutralise)
        })?;
        for &language in languages {
            table.write_all(b",")?;
            let stored = texts.text(key, language);
            table.datum(
                LocalizationTableDatum::Text {
                    row,
                    key,
                    language,
                    stored,
                },
                |out| {
                    if let Some(text) = stored {
                        write_csv_cell(out, &decode_line_breaks(text), neutralise)?;
                    }
                    Ok(())
                },
            )?;
        }
        table.write_all(b"\r\n")?;
        count += 1;
    }
    Ok(count)
}

struct TableWriter<'out, Observe> {
    out: &'out mut dyn Write,
    position: u64,
    observe: Observe,
}

impl<Observe> TableWriter<'_, Observe>
where
    Observe: FnMut(LocalizationTableDatum<'_>, Range<u64>) -> io::Result<()>,
{
    fn datum(
        &mut self,
        datum: LocalizationTableDatum<'_>,
        write: impl FnOnce(&mut Self) -> io::Result<()>,
    ) -> io::Result<()> {
        let start = self.position;
        write(self)?;
        (self.observe)(datum, start..self.position)
    }
}

impl<Observe> Write for TableWriter<'_, Observe> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.out.write(buf)?;
        self.position = self
            .position
            .checked_add(written as u64)
            .ok_or_else(|| io::Error::other("localization output byte count exceeds u64"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}
