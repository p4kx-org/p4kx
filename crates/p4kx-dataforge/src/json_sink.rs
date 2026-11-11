// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::{Annotations, RecordTree, Reference, Resolution, Value};
use crate::{json_text, texts, value_json};

pub(super) struct Out<'writer, Writer> {
    pub(super) sink: &'writer mut Writer,
    pub(super) pretty: bool,
    pub(super) annotations: Annotations<'writer>,
}

impl<Writer: Write> Out<'_, Writer> {
    pub(super) fn separator(&mut self, first: bool, depth: usize) -> io::Result<()> {
        if !first {
            self.sink.write_all(b",")?;
        }
        if self.pretty {
            json_text::new_line(self.sink, depth)?;
        }
        Ok(())
    }

    pub(super) fn space(&mut self) -> io::Result<()> {
        if self.pretty {
            self.sink.write_all(b" ")?;
        }
        Ok(())
    }

    pub(super) fn key(&mut self, first: bool, depth: usize, key: &str) -> io::Result<()> {
        self.separator(first, depth)?;
        value_json::write_json_string(self.sink, key)?;
        self.sink.write_all(b":")?;
        self.space()
    }

    pub(super) fn close(&mut self, depth: usize, bracket: &[u8]) -> io::Result<()> {
        if self.pretty {
            json_text::new_line(self.sink, depth)?;
        }
        self.sink.write_all(bracket)
    }
}

pub(super) fn header<Writer: Write>(
    out: &mut Out<'_, Writer>,
    record: Option<&RecordTree<'_>>,
    type_name: &str,
    depth: usize,
) -> io::Result<()> {
    out.sink.write_all(b"{")?;
    out.key(true, depth + 1, "$type")?;
    value_json::write_json_string(out.sink, type_name)?;
    if let Some(tree) = record {
        record_header(out, tree, depth)?;
    }
    Ok(())
}

fn record_header<Writer: Write>(
    out: &mut Out<'_, Writer>,
    tree: &RecordTree<'_>,
    depth: usize,
) -> io::Result<()> {
    out.key(false, depth + 1, "$guid")?;
    write!(out.sink, "\"{}\"", tree.guid())?;
    out.key(false, depth + 1, "$path")?;
    value_json::write_json_string(out.sink, tree.record_path())?;
    out.key(false, depth + 1, "$name")?;
    value_json::write_json_string(out.sink, tree.name())?;
    if let Some(tag) = tree.tag() {
        out.key(false, depth + 1, "$tag")?;
        value_json::write_json_string(out.sink, tag)?;
    }
    Ok(())
}

pub(super) fn leaf<Writer: Write>(
    out: &mut Out<'_, Writer>,
    value: &Value<'_>,
    depth: usize,
) -> io::Result<()> {
    match value {
        Value::Reference(value) => reference(out, value, depth),
        Value::Null => out.sink.write_all(b"null"),
        Value::Error(error) => {
            out.sink.write_all(b"{")?;
            out.key(true, depth + 1, "error")?;
            value_json::write_json_string(out.sink, &error.to_string())?;
            if let Some(guid) = error.dangling_guid() {
                out.key(false, depth + 1, "guid")?;
                write!(out.sink, "\"{guid}\"")?;
            }
            out.close(depth, b"}")
        }
        Value::Locale(key) => locale(out, key, depth),
        scalar => value_json::write_json_value(out.sink, scalar),
    }
}

fn locale<Writer: Write>(out: &mut Out<'_, Writer>, key: &str, depth: usize) -> io::Result<()> {
    let Some(resolution) = texts::annotation(&out.annotations, key) else {
        return value_json::write_json_string(out.sink, key);
    };
    out.sink.write_all(b"{")?;
    out.key(true, depth + 1, "key")?;
    value_json::write_json_string(out.sink, key)?;
    if let Resolution::Text(text) = resolution {
        out.key(false, depth + 1, "text")?;
        value_json::write_json_string(out.sink, &text)?;
    } else {
        out.key(false, depth + 1, "missing")?;
        out.sink.write_all(b"true")?;
    }
    out.close(depth, b"}")
}

fn reference<Writer: Write>(
    out: &mut Out<'_, Writer>,
    reference: &Reference<'_>,
    depth: usize,
) -> io::Result<()> {
    out.sink.write_all(b"{")?;
    out.key(true, depth + 1, "guid")?;
    write!(out.sink, "\"{}\"", reference.guid())?;
    out.key(false, depth + 1, "path")?;
    value_json::write_json_string(out.sink, reference.record_path())?;
    out.key(false, depth + 1, "type")?;
    value_json::write_json_string(out.sink, reference.type_name())?;
    if let Some(location) = reference.location() {
        out.key(false, depth + 1, "at")?;
        value_json::write_json_string(out.sink, location.as_str())?;
    }
    if let Some(path) = out.annotations.tag_path(reference) {
        out.key(false, depth + 1, "tag")?;
        value_json::write_json_string(out.sink, path.as_ref())?;
    }
    out.close(depth, b"}")
}
