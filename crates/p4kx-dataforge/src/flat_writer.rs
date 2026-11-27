// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use super::observation::{FlatColumn, FlatColumnSpan, FlatRowObservation};
use super::output::Cells;
use super::render;
use crate::flat_cells::{self, CSV_BYTE_ORDER_MARK, CsvCellMode};
use crate::{
    Annotations, Guid, ObservedResolution, PositionedRecord, RecordTree, Resolution, Value,
};

/// The first columns of a flat value export, in order; the text column follows when texts are on,
/// and the Record tag column is always the last.
pub const FLAT_COLUMNS: [&str; 8] = [
    FlatColumn::RecordGuid.name(),
    FlatColumn::RecordName.name(),
    FlatColumn::RecordPath.name(),
    FlatColumn::FieldPath.name(),
    FlatColumn::ArrayIndices.name(),
    FlatColumn::Kind.name(),
    FlatColumn::Value.name(),
    FlatColumn::TargetPath.name(),
];

/// The name of the last column, which holds the Record tag.
pub const FLAT_TAG_COLUMN: &str = FlatColumn::RecordTag.name();

/// The name of the column that holds the resolved text of a localisation key.
pub const FLAT_TEXT_COLUMN: &str = FlatColumn::Text.name();

/// How a flat value export is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum FlatFormat {
    /// CSV with a header row, a UTF-8 byte order mark and CRLF rows; string, localisation and enum
    /// values that a spreadsheet would run as a formula get a leading `'`.
    #[default]
    Csv,
    /// CSV like [`FlatFormat::Csv`] but without byte order mark and without any change to a value.
    CsvRaw,
    /// One compact JSON object per value, ended by a line feed.
    Ndjson,
    /// CSV with RFC 4180 ASCII fields, CRLF rows and no byte order mark.
    CsvStrict,
}

impl FlatFormat {
    const fn csv_mode(self) -> Option<CsvCellMode> {
        match self {
            Self::Csv => Some(CsvCellMode::Spreadsheet),
            Self::CsvRaw => Some(CsvCellMode::Raw),
            Self::CsvStrict => Some(CsvCellMode::Strict),
            Self::Ndjson => None,
        }
    }
}

/// Writes Record trees as one row per leaf value, the format `docs/record-flat.md` defines.
///
/// One writer can be reused for many Records; it keeps its scratch space between them. The same
/// tree always gives the same bytes.
#[derive(Debug)]
pub struct FlatValues {
    pub(super) format: FlatFormat,
    pub(super) prefix: Vec<u8>,
    pub(super) tag: Vec<u8>,
    pub(super) path: String,
    pub(super) indices: Vec<u32>,
    pub(super) escaped: Vec<bool>,
    pub(super) order: Vec<usize>,
}

/// What a row holds: a leaf value, or a Reference to a Record that does not exist.
#[derive(Clone, Copy)]
pub(super) enum Leaf<'value, 'source> {
    Value(&'value Value<'source>),
    Dangling(Guid),
}

/// The output of one Record and what it is written with.
pub(super) struct Out<'writer, 'text, W, const OBSERVED: bool> {
    pub(super) sink: Cells<&'writer mut W, OBSERVED>,
    pub(super) prefix_spans: Vec<FlatColumnSpan>,
    pub(super) annotations: Annotations<'text>,
}

impl<W: Write, const OBSERVED: bool> Out<'_, '_, W, OBSERVED> {
    pub(super) const fn spans(&self) -> &[FlatColumnSpan] {
        self.sink.spans()
    }
}

impl FlatValues {
    /// Creates a writer for `format`.
    #[must_use]
    pub const fn new(format: FlatFormat) -> Self {
        Self {
            format,
            prefix: Vec::new(),
            tag: Vec::new(),
            path: String::new(),
            indices: Vec::new(),
            escaped: Vec::new(),
            order: Vec::new(),
        }
    }

    /// Returns the format the writer writes.
    #[must_use]
    pub const fn format(&self) -> FlatFormat {
        self.format
    }

    /// Writes the CSV headings with the optional text column, plus a UTF-8 byte order mark
    /// for [`FlatFormat::Csv`].
    ///
    /// # Errors
    ///
    /// Returns the I/O error of `out`.
    pub fn write_header<W: Write>(&self, with_text: bool, out: &mut W) -> io::Result<()> {
        let Some(mode) = self.format.csv_mode() else {
            return Ok(());
        };
        if mode == CsvCellMode::Spreadsheet {
            out.write_all(CSV_BYTE_ORDER_MARK)?;
        }
        let mode = mode.verbatim();
        for column in FLAT_COLUMNS {
            flat_cells::write_csv_cell_with_mode(out, column, mode)?;
            out.write_all(b",")?;
        }
        if with_text {
            flat_cells::write_csv_cell_with_mode(out, FLAT_TEXT_COLUMN, mode)?;
            out.write_all(b",")?;
        }
        flat_cells::write_csv_cell_with_mode(out, FLAT_TAG_COLUMN, mode)?;
        out.write_all(b"\r\n")
    }

    /// Writes the rows of `tree` to `out`: one per leaf value that is not empty. The texts of
    /// `annotations`, if any, are the text column of localisation keys.
    ///
    /// # Errors
    ///
    /// Returns an I/O error or [`io::ErrorKind::InvalidData`] for unsupported strict CSV data.
    pub fn write_annotated<W: Write>(
        &mut self,
        tree: &RecordTree<'_>,
        annotations: Annotations<'_>,
        out: &mut W,
    ) -> io::Result<()> {
        let mut out = self.begin::<W, false>(tree, annotations, out)?;
        self.walk_fields(&mut out, tree.fields(), None, None)
    }

    /// Writes the same flat bytes while observing the positions used by each emitted row.
    /// Column spans are relative to this call and include the format's value quoting.
    ///
    /// # Errors
    /// Returns an output, DataForge position or observer error.
    pub fn write_positioned_observed<'record, 'source, 'text, W: Write>(
        &mut self,
        record: &'record PositionedRecord<'source>,
        annotations: Annotations<'text>,
        out: &mut W,
        mut observe: impl for<'row> FnMut(
            FlatRowObservation<'row, 'record, 'source, 'text>,
        ) -> io::Result<()>,
    ) -> io::Result<()> {
        let tree = record.tree();
        let positions = record.root_fields().map_err(io::Error::other)?;
        let mut out = self.begin::<W, true>(tree, annotations, out)?;
        self.walk_fields(&mut out, tree.fields(), Some(positions), Some(&mut observe))
    }

    fn begin<'writer, 'text, W: Write, const OBSERVED: bool>(
        &mut self,
        tree: &RecordTree<'_>,
        annotations: Annotations<'text>,
        out: &'writer mut W,
    ) -> io::Result<Out<'writer, 'text, W, OBSERVED>> {
        self.path.clear();
        self.indices.clear();
        self.escaped.clear();
        self.prefix.clear();
        self.tag.clear();
        let mut prefix = Cells::<_, OBSERVED>::new(&mut self.prefix);
        if let Some(mode) = self.format.csv_mode() {
            render::write_csv_prefix(&mut prefix, tree, mode.verbatim())?;
            flat_cells::write_csv_cell_with_mode(
                &mut self.tag,
                tree.tag().unwrap_or_default(),
                mode,
            )?;
        } else {
            render::write_json_prefix(&mut prefix, tree)?;
        }
        Ok(Out {
            sink: Cells::new(out),
            prefix_spans: prefix.into_spans(),
            annotations,
        })
    }

    pub(super) fn write_row<'text, W: Write, const OBSERVED: bool>(
        &self,
        out: &mut Out<'_, 'text, W, OBSERVED>,
        leaf: Leaf<'_, '_>,
    ) -> io::Result<Option<ObservedResolution<'text>>> {
        let localization = match leaf {
            Leaf::Value(Value::Locale(key)) => out
                .annotations
                .texts()
                .map(|texts| texts.resolve_observed(key)),
            _ => None,
        };
        let text = match localization.as_ref().map(ObservedResolution::resolution) {
            Some(Resolution::Text(text)) => Some(text.as_ref()),
            _ => None,
        };
        let target = match leaf {
            Leaf::Value(Value::Reference(reference)) => Some(reference.record_path()),
            _ => None,
        };
        if let Some(mode) = self.format.csv_mode() {
            self.write_csv_row(out, leaf, target, text, mode)?;
        } else {
            self.write_json_row(out, leaf, target, text)?;
        }
        Ok(localization)
    }
}
