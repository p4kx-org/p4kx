// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::positioned_record::ValueSlot;
use crate::positioned_reference_members::DanglingReferenceMember;
use crate::{
    Annotations, PositionedReferenceSources, RecordJsonReferenceMember, RecordTree, Reference,
    Resolution, StructValue, Value, ValueError,
};

use super::observation::Observation;
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
        Value::Error(error) => error_value(out, error, slot, depth),
        Value::Locale(key) => locale(out, key, slot, depth),
        scalar => value_json::write_json_value(out.sink, scalar),
    }
}

fn error_value<'source, Writer: Write>(
    out: &mut Out<'_, 'source, Writer>,
    error: &ValueError,
    slot: Option<ValueSlot<'source>>,
    depth: usize,
) -> io::Result<()> {
    let dangling = error.dangling_guid();
    out.sink.write_all(b"{")?;
    out.key(true, depth + 1, "error")?;
    value_json::write_json_string(out.sink, &error.to_string())?;
    if let Some(observation) = &mut out.observation {
        if dangling.is_some() {
            observation.dangling_reference(slot, DanglingReferenceMember::Error)?;
        } else {
            observation.value(slot, None)?;
        }
    }
    if let Some(guid) = dangling {
        out.key(false, depth + 1, "guid")?;
        write!(out.sink, "\"{guid}\"")?;
        if let Some(observation) = &mut out.observation {
            observation.dangling_reference(slot, DanglingReferenceMember::Guid)?;
        }
    }
    out.close(depth, b"}")
}

fn locale<'source, Writer: Write>(
    out: &mut Out<'_, 'source, Writer>,
    key: &str,
    slot: Option<ValueSlot<'source>>,
    depth: usize,
) -> io::Result<()> {
    let Some(resolution) = texts::annotation(&out.annotations, key) else {
        observe_key(out, slot, key)?;
        return value_json::write_json_string(out.sink, key);
    };
    out.sink.write_all(b"{")?;
    out.key(true, depth + 1, "key")?;
    observe_key(out, slot, key)?;
    value_json::write_json_string(out.sink, key)?;
    if let Resolution::Text(text) = &resolution {
        out.key(false, depth + 1, "text")?;
        value_json::write_json_string(out.sink, text)?;
    } else {
        out.key(false, depth + 1, "missing")?;
        out.sink.write_all(b"true")?;
    }
    if let Some(observation) = &mut out.observation {
        observation.localization(slot, key, &resolution)?;
    }
    out.close(depth, b"}")
}

fn observe_key<'source, Writer: Write>(
    out: &mut Out<'_, 'source, Writer>,
    slot: Option<ValueSlot<'source>>,
    key: &str,
) -> io::Result<()> {
    if let Some(observation) = &mut out.observation {
        observation.value(slot, Some(&Value::Locale(key)))?;
    }
    Ok(())
}

fn reference<'source, Writer: Write>(
    out: &mut Out<'_, 'source, Writer>,
    reference: &Reference<'source>,
    slot: Option<ValueSlot<'source>>,
    depth: usize,
) -> io::Result<()> {
    let sources = out
        .observation
        .as_ref()
        .and_then(|observation| observation.reference_sources(slot, reference));
    let sources = sources.as_ref();
    out.sink.write_all(b"{")?;
    out.key(true, depth + 1, "guid")?;
    write!(out.sink, "\"{}\"", reference.guid())?;
    observe_link(out, sources, RecordJsonReferenceMember::Guid)?;
    out.key(false, depth + 1, "path")?;
    value_json::write_json_string(out.sink, reference.record_path())?;
    observe_link(out, sources, RecordJsonReferenceMember::Path)?;
    out.key(false, depth + 1, "type")?;
    value_json::write_json_string(out.sink, reference.type_name())?;
    observe_link(out, sources, RecordJsonReferenceMember::Type)?;
    if let Some(location) = reference.location() {
        out.key(false, depth + 1, "at")?;
        value_json::write_json_string(out.sink, location.as_str())?;
        observe_link(out, sources, RecordJsonReferenceMember::InstancePath)?;
    }
    if let Some(path) = out.annotations.tag_path(reference) {
        out.key(false, depth + 1, "tag")?;
        value_json::write_json_string(out.sink, path.as_ref())?;
        if let Some(observation) = &mut out.observation {
            observation.tag(sources, reference.guid(), path.as_ref())?;
        }
    }
    out.close(depth, b"}")
}

fn observe_link<Writer: Write>(
    out: &mut Out<'_, '_, Writer>,
    sources: Option<&PositionedReferenceSources>,
    member: RecordJsonReferenceMember,
) -> io::Result<()> {
    if let Some(observation) = &mut out.observation {
        observation.reference(sources, member)?;
    }
    Ok(())
}
