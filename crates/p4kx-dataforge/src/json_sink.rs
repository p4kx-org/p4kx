// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::positioned_record::ValueSlot;
use crate::{Annotations, RecordTree, Reference, Resolution, StructValue, Value};

use super::observation::{Observation, ReferenceMember};
use super::source::RecordJsonRecordField;
use crate::{json_text, texts, value_json};

pub(super) struct Out<'writer, 'source, Writer> {
    pub(super) sink: &'writer mut Writer,
    pub(super) pretty: bool,
    pub(super) annotations: Annotations<'writer>,
    pub(super) observation: Option<Observation<'writer, 'source>>,
}

impl<Writer: Write> Out<'_, '_, Writer> {
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
        if let Some(observation) = &mut self.observation {
            observation.key(depth, key);
        }
        self.space()
    }

    pub(super) fn close(&mut self, depth: usize, bracket: &[u8]) -> io::Result<()> {
        if self.pretty {
            json_text::new_line(self.sink, depth)?;
        }
        self.sink.write_all(bracket)
    }
}

pub(super) fn header<'source, Writer: Write>(
    out: &mut Out<'_, 'source, Writer>,
    record: Option<&RecordTree<'source>>,
    structure: &StructValue<'source>,
    slot: Option<ValueSlot<'source>>,
    depth: usize,
) -> io::Result<()> {
    out.sink.write_all(b"{")?;
    out.key(true, depth + 1, "$type")?;
    value_json::write_json_string(out.sink, structure.type_name())?;
    if let Some(observation) = &mut out.observation {
        observation.structure_name(
            structure.struct_index(),
            record.map(RecordTree::record_index),
            slot,
        )?;
    }
    if let Some(tree) = record {
        record_header(out, tree, depth)?;
    }
    Ok(())
}

fn record_header<Writer: Write>(
    out: &mut Out<'_, '_, Writer>,
    tree: &RecordTree<'_>,
    depth: usize,
) -> io::Result<()> {
    out.key(false, depth + 1, "$guid")?;
    write!(out.sink, "\"{}\"", tree.guid())?;
    observe_header(out, tree, RecordJsonRecordField::Guid, None)?;
    out.key(false, depth + 1, "$path")?;
    value_json::write_json_string(out.sink, tree.record_path())?;
    observe_header(
        out,
        tree,
        RecordJsonRecordField::FileName,
        Some(tree.metadata().file_name()),
    )?;
    out.key(false, depth + 1, "$name")?;
    value_json::write_json_string(out.sink, tree.name())?;
    observe_header(out, tree, RecordJsonRecordField::Name, Some(tree.name()))?;
    if let Some(tag) = tree.tag() {
        out.key(false, depth + 1, "$tag")?;
        value_json::write_json_string(out.sink, tag)?;
        observe_header(out, tree, RecordJsonRecordField::Tag, Some(tag))?;
    }
    Ok(())
}

fn observe_header<Writer: Write>(
    out: &mut Out<'_, '_, Writer>,
    tree: &RecordTree<'_>,
    field: RecordJsonRecordField,
    text: Option<&str>,
) -> io::Result<()> {
    if let Some(observation) = &mut out.observation {
        observation.header(tree.record_index(), field, text)?;
    }
    Ok(())
}

pub(super) fn leaf<'source, Writer: Write>(
    out: &mut Out<'_, 'source, Writer>,
    value: &Value<'source>,
    slot: Option<ValueSlot<'source>>,
    depth: usize,
) -> io::Result<()> {
    match value {
        Value::Reference(value) => reference(out, value, slot, depth),
        Value::Null => out.sink.write_all(b"null"),
        Value::Error(error) => {
            out.sink.write_all(b"{")?;
            out.key(true, depth + 1, "error")?;
            value_json::write_json_string(out.sink, &error.to_string())?;
            if let Some(observation) = &mut out.observation {
                observation.value(slot, None)?;
            }
            if let Some(guid) = error.dangling_guid() {
                out.key(false, depth + 1, "guid")?;
                write!(out.sink, "\"{guid}\"")?;
                if let Some(observation) = &mut out.observation {
                    observation.value(slot, None)?;
                }
            }
            out.close(depth, b"}")
        }
        Value::Locale(key) => locale(out, key, depth),
        scalar => value_json::write_json_value(out.sink, scalar),
    }
}

fn locale<Writer: Write>(out: &mut Out<'_, '_, Writer>, key: &str, depth: usize) -> io::Result<()> {
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

fn reference<'source, Writer: Write>(
    out: &mut Out<'_, 'source, Writer>,
    reference: &Reference<'source>,
    slot: Option<ValueSlot<'source>>,
    depth: usize,
) -> io::Result<()> {
    out.sink.write_all(b"{")?;
    out.key(true, depth + 1, "guid")?;
    write!(out.sink, "\"{}\"", reference.guid())?;
    observe_link(out, reference, slot, ReferenceMember::Guid)?;
    out.key(false, depth + 1, "path")?;
    value_json::write_json_string(out.sink, reference.record_path())?;
    observe_link(out, reference, slot, ReferenceMember::Path)?;
    out.key(false, depth + 1, "type")?;
    value_json::write_json_string(out.sink, reference.type_name())?;
    observe_link(out, reference, slot, ReferenceMember::Type)?;
    if let Some(location) = reference.location() {
        out.key(false, depth + 1, "at")?;
        value_json::write_json_string(out.sink, location.as_str())?;
        observe_link(out, reference, slot, ReferenceMember::InstancePath)?;
    }
    if let Some(path) = out.annotations.tag_path(reference) {
        out.key(false, depth + 1, "tag")?;
        value_json::write_json_string(out.sink, path.as_ref())?;
    }
    out.close(depth, b"}")
}

fn observe_link<'source, Writer: Write>(
    out: &mut Out<'_, 'source, Writer>,
    reference: &Reference<'source>,
    slot: Option<ValueSlot<'source>>,
    member: ReferenceMember,
) -> io::Result<()> {
    if let Some(observation) = &mut out.observation {
        observation.reference(slot, reference, member)?;
    }
    Ok(())
}
