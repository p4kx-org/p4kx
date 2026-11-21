// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::flat_cells::{CSV_BYTE_ORDER_MARK, write_csv_cell, write_plain};
use crate::texts::annotation;
use crate::value_json::{write_json_string, write_json_value};
use crate::{Annotations, Guid, RecordTree, Resolution, Value};

/// The first columns of a flat value export, in order; the text column follows when texts are on,
/// and the Record tag column is always the last.
pub const FLAT_COLUMNS: [&str; 8] = [
    "record_guid",
    "record_name",
    "record_path",
    "field_path",
    "array_indices",
    "kind",
    "value",
    "target_path",
];

/// The name of the last column, which holds the Record tag.
pub const FLAT_TAG_COLUMN: &str = "record_tag";

/// The name of the column that holds the resolved text of a localisation key.
pub const FLAT_TEXT_COLUMN: &str = "text";

/// How a flat value export is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlatFormat {
    /// CSV with a header row, a UTF-8 byte order mark and CRLF rows; string, localisation and enum
    /// values that a spreadsheet would run as a formula get a leading `'`.
    #[default]
    Csv,
    /// CSV like [`FlatFormat::Csv`] but without byte order mark and without any change to a value.
    CsvRaw,
    /// One compact JSON object per value, ended by a line feed.
    Ndjson,
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
pub(super) struct Out<'writer, 'annotations, W> {
    out: &'writer mut W,
    annotations: Annotations<'annotations>,
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

    /// Writes what starts the file: for [`FlatFormat::Csv`] the byte order mark, for both CSV
    /// formats the header row with the text column when `with_text`; nothing for NDJSON.
    ///
    /// # Errors
    ///
    /// Returns the I/O error of `out`.
    pub fn write_header<W: Write>(&self, with_text: bool, out: &mut W) -> io::Result<()> {
        if self.format == FlatFormat::Ndjson {
            return Ok(());
        }
        if self.format == FlatFormat::Csv {
            out.write_all(CSV_BYTE_ORDER_MARK)?;
        }
        out.write_all(FLAT_COLUMNS.join(",").as_bytes())?;
        if with_text {
            out.write_all(b",")?;
            out.write_all(FLAT_TEXT_COLUMN.as_bytes())?;
        }
        out.write_all(b",")?;
        out.write_all(FLAT_TAG_COLUMN.as_bytes())?;
        out.write_all(b"\r\n")
    }

    /// Writes the rows of `tree` to `out`: one per leaf value that is not empty. The texts of
    /// `annotations`, if any, are the text column of localisation keys.
    ///
    /// # Errors
    ///
    /// Returns the I/O error of `out`.
    pub fn write_annotated<W: Write>(
        &mut self,
        tree: &RecordTree<'_>,
        annotations: Annotations<'_>,
        out: &mut W,
    ) -> io::Result<()> {
        self.path.clear();
        self.indices.clear();
        self.escaped.clear();
        self.prefix.clear();
        self.tag.clear();
        if self.format == FlatFormat::Ndjson {
            write_json_prefix(&mut self.prefix, tree)?;
        } else {
            write_csv_prefix(&mut self.prefix, tree)?;
            if let Some(tag) = tree.tag() {
                write_csv_cell(&mut self.tag, tag, self.format == FlatFormat::Csv)?;
            }
        }
        let mut out = Out { out, annotations };
        self.walk_fields(&mut out, tree.fields())
    }

    pub(super) fn write_row<W: Write>(
        &self,
        out: &mut Out<'_, '_, W>,
        leaf: Leaf<'_, '_>,
    ) -> io::Result<()> {
        let text = match leaf {
            Leaf::Value(Value::Locale(key)) => match annotation(&out.annotations, key) {
                Some(Resolution::Text(text)) => Some(text),
                _ => None,
            },
            _ => None,
        };
        let target = match leaf {
            Leaf::Value(Value::Reference(reference)) => Some(reference.record_path()),
            _ => None,
        };
        if self.format == FlatFormat::Ndjson {
            self.write_json_row(out, leaf, target, text.as_deref())
        } else {
            self.write_csv_row(out, leaf, target, text.as_deref())
        }
    }

    fn write_csv_row<W: Write>(
        &self,
        out: &mut Out<'_, '_, W>,
        leaf: Leaf<'_, '_>,
        target: Option<&str>,
        text: Option<&str>,
    ) -> io::Result<()> {
        let neutralise = self.format == FlatFormat::Csv;
        let sink = &mut *out.out;
        sink.write_all(&self.prefix)?;
        write_csv_cell(sink, &self.path, false)?;
        sink.write_all(b",")?;
        write_indices(sink, &self.indices)?;
        write!(sink, ",{},", kind(leaf))?;
        match leaf {
            Leaf::Value(Value::String(text) | Value::Locale(text) | Value::Enum(text)) => {
                write_csv_cell(sink, text, neutralise)?;
            }
            Leaf::Value(Value::Reference(reference)) => write!(sink, "{}", reference.guid())?,
            Leaf::Dangling(guid) => write!(sink, "{guid}")?,
            Leaf::Value(scalar) => write_plain(sink, scalar)?,
        }
        sink.write_all(b",")?;
        if let Some(path) = target {
            write_csv_cell(sink, path, false)?;
        }
        if out.annotations.texts().is_some() {
            sink.write_all(b",")?;
            if let Some(text) = text {
                write_csv_cell(sink, text, neutralise)?;
            }
        }
        sink.write_all(b",")?;
        sink.write_all(&self.tag)?;
        sink.write_all(b"\r\n")
    }

    fn write_json_row<W: Write>(
        &self,
        out: &mut Out<'_, '_, W>,
        leaf: Leaf<'_, '_>,
        target: Option<&str>,
        text: Option<&str>,
    ) -> io::Result<()> {
        let sink = &mut *out.out;
        sink.write_all(&self.prefix)?;
        sink.write_all(b",\"field_path\":")?;
        write_json_string(sink, &self.path)?;
        sink.write_all(b",\"array_indices\":\"")?;
        write_indices(sink, &self.indices)?;
        write!(sink, "\",\"kind\":\"{}\",\"value\":", kind(leaf))?;
        match leaf {
            Leaf::Value(Value::Reference(reference)) => {
                write!(sink, "\"{}\"", reference.guid())?;
            }
            Leaf::Dangling(guid) => write!(sink, "\"{guid}\"")?,
            Leaf::Value(scalar) => write_json_value(sink, scalar)?,
        }
        if let Some(path) = target {
            sink.write_all(b",\"target_path\":")?;
            write_json_string(sink, path)?;
        }
        if let Some(text) = text {
            sink.write_all(b",\"text\":")?;
            write_json_string(sink, text)?;
        }
        sink.write_all(b"}\n")
    }
}

fn write_csv_prefix(prefix: &mut Vec<u8>, tree: &RecordTree<'_>) -> io::Result<()> {
    write!(prefix, "{},", tree.guid())?;
    write_csv_cell(prefix, tree.name(), false)?;
    prefix.push(b',');
    write_csv_cell(prefix, tree.record_path(), false)?;
    prefix.push(b',');
    Ok(())
}

fn write_json_prefix(prefix: &mut Vec<u8>, tree: &RecordTree<'_>) -> io::Result<()> {
    write!(
        prefix,
        "{{\"record_guid\":\"{}\",\"record_name\":",
        tree.guid()
    )?;
    write_json_string(prefix, tree.name())?;
    prefix.extend_from_slice(b",\"record_path\":");
    write_json_string(prefix, tree.record_path())?;
    if let Some(tag) = tree.tag() {
        prefix.extend_from_slice(b",\"record_tag\":");
        write_json_string(prefix, tag)?;
    }
    Ok(())
}

/// Returns the `kind` column of a leaf value.
const fn kind(leaf: Leaf<'_, '_>) -> &'static str {
    let Leaf::Value(value) = leaf else {
        return "reference";
    };
    match value {
        Value::Bool(_) => "bool",
        Value::Int8(_) | Value::Int16(_) | Value::Int32(_) | Value::Int64(_) => "int",
        Value::UInt8(_) | Value::UInt16(_) | Value::UInt32(_) | Value::UInt64(_) => "uint",
        Value::Single(_) => "single",
        Value::Double(_) => "double",
        Value::String(_) => "string",
        Value::Locale(_) => "locale",
        Value::Enum(_) => "enum",
        Value::Guid(_) => "guid",
        Value::Reference(_) => "reference",
        Value::Struct(_) | Value::Array(_) | Value::Null | Value::Error(_) => "",
    }
}

fn write_indices(out: &mut impl Write, indices: &[u32]) -> io::Result<()> {
    for (position, index) in indices.iter().enumerate() {
        if position > 0 {
            out.write_all(b".")?;
        }
        write!(out, "{index}")?;
    }
    Ok(())
}
