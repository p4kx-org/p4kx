// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use super::observation::FlatColumn;
use super::output::Cells;
use super::writer::{FlatValues, Leaf, Out};
use crate::flat_cells::{self, CsvCellMode};
use crate::value_json::{write_json_string, write_json_value};
use crate::{RecordTree, Value};

impl FlatValues {
    pub(super) fn write_csv_row<W: Write, const OBSERVED: bool>(
        &self,
        out: &mut Out<'_, '_, W, OBSERVED>,
        leaf: Leaf<'_, '_>,
        target: Option<&str>,
        text: Option<&str>,
        mode: CsvCellMode,
    ) -> io::Result<()> {
        let structural = mode.verbatim();
        let sink = &mut out.sink;
        sink.prefix(&self.prefix, &out.prefix_spans)?;
        sink.cell(FlatColumn::FieldPath, |sink| {
            flat_cells::write_csv_cell_with_mode(sink, &self.path, structural)
        })?;
        sink.write_all(b",")?;
        sink.cell(FlatColumn::ArrayIndices, |sink| {
            if self.indices.is_empty() {
                flat_cells::write_csv_cell_with_mode(sink, "", structural)
            } else {
                write_indices(sink, &self.indices)
            }
        })?;
        sink.write_all(b",")?;
        sink.cell(FlatColumn::Kind, |sink| {
            sink.write_all(kind(leaf).as_bytes())
        })?;
        sink.write_all(b",")?;
        sink.cell(FlatColumn::Value, |sink| write_csv_value(sink, leaf, mode))?;
        sink.write_all(b",")?;
        sink.cell(FlatColumn::TargetPath, |sink| {
            flat_cells::write_csv_cell_with_mode(sink, target.unwrap_or_default(), structural)
        })?;
        if out.annotations.texts().is_some() {
            sink.write_all(b",")?;
            sink.cell(FlatColumn::Text, |sink| {
                flat_cells::write_csv_cell_with_mode(sink, text.unwrap_or_default(), mode)
            })?;
        }
        sink.write_all(b",")?;
        sink.cell(FlatColumn::RecordTag, |sink| sink.write_all(&self.tag))?;
        sink.write_all(b"\r\n")
    }

    pub(super) fn write_json_row<W: Write, const OBSERVED: bool>(
        &self,
        out: &mut Out<'_, '_, W, OBSERVED>,
        leaf: Leaf<'_, '_>,
        target: Option<&str>,
        text: Option<&str>,
    ) -> io::Result<()> {
        let sink = &mut out.sink;
        sink.prefix(&self.prefix, &out.prefix_spans)?;
        sink.write_all(b",\"field_path\":")?;
        sink.cell(FlatColumn::FieldPath, |sink| {
            write_json_string(sink, &self.path)
        })?;
        sink.write_all(b",\"array_indices\":")?;
        sink.cell(FlatColumn::ArrayIndices, |sink| {
            sink.write_all(b"\"")?;
            write_indices(sink, &self.indices)?;
            sink.write_all(b"\"")
        })?;
        sink.write_all(b",\"kind\":")?;
        sink.cell(FlatColumn::Kind, |sink| write_json_string(sink, kind(leaf)))?;
        sink.write_all(b",\"value\":")?;
        sink.cell(FlatColumn::Value, |sink| write_json_leaf(sink, leaf))?;
        if let Some(path) = target {
            sink.write_all(b",\"target_path\":")?;
            sink.cell(FlatColumn::TargetPath, |sink| write_json_string(sink, path))?;
        }
        if let Some(text) = text {
            sink.write_all(b",\"text\":")?;
            sink.cell(FlatColumn::Text, |sink| write_json_string(sink, text))?;
        }
        sink.write_all(b"}\n")
    }
}

pub(super) fn write_csv_prefix<W: Write, const OBSERVED: bool>(
    prefix: &mut Cells<W, OBSERVED>,
    tree: &RecordTree<'_>,
    mode: CsvCellMode,
) -> io::Result<()> {
    prefix.cell(FlatColumn::RecordGuid, |sink| {
        write!(sink, "{}", tree.guid())
    })?;
    prefix.write_all(b",")?;
    prefix.cell(FlatColumn::RecordName, |sink| {
        flat_cells::write_csv_cell_with_mode(sink, tree.name(), mode)
    })?;
    prefix.write_all(b",")?;
    prefix.cell(FlatColumn::RecordPath, |sink| {
        flat_cells::write_csv_cell_with_mode(sink, tree.record_path(), mode)
    })?;
    prefix.write_all(b",")
}

pub(super) fn write_json_prefix<W: Write, const OBSERVED: bool>(
    prefix: &mut Cells<W, OBSERVED>,
    tree: &RecordTree<'_>,
) -> io::Result<()> {
    prefix.write_all(b"{\"record_guid\":")?;
    prefix.cell(FlatColumn::RecordGuid, |sink| {
        write!(sink, "\"{}\"", tree.guid())
    })?;
    prefix.write_all(b",\"record_name\":")?;
    prefix.cell(FlatColumn::RecordName, |sink| {
        write_json_string(sink, tree.name())
    })?;
    prefix.write_all(b",\"record_path\":")?;
    prefix.cell(FlatColumn::RecordPath, |sink| {
        write_json_string(sink, tree.record_path())
    })?;
    if let Some(tag) = tree.tag() {
        prefix.write_all(b",\"record_tag\":")?;
        prefix.cell(FlatColumn::RecordTag, |sink| write_json_string(sink, tag))?;
    }
    Ok(())
}

fn write_csv_value(sink: &mut impl Write, leaf: Leaf<'_, '_>, mode: CsvCellMode) -> io::Result<()> {
    match leaf {
        Leaf::Value(Value::String(text) | Value::Locale(text) | Value::Enum(text)) => {
            flat_cells::write_csv_cell_with_mode(sink, text, mode)
        }
        Leaf::Value(Value::Reference(reference)) => write!(sink, "{}", reference.guid()),
        Leaf::Dangling(guid) => write!(sink, "{guid}"),
        Leaf::Value(scalar) => flat_cells::write_plain(sink, scalar),
    }
}

fn write_json_leaf(sink: &mut impl Write, leaf: Leaf<'_, '_>) -> io::Result<()> {
    match leaf {
        Leaf::Value(Value::Reference(reference)) => write!(sink, "\"{}\"", reference.guid()),
        Leaf::Dangling(guid) => write!(sink, "\"{guid}\""),
        Leaf::Value(scalar) => write_json_value(sink, scalar),
    }
}

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
