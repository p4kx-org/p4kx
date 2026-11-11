// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::record_xml::RECORD_XML_NAMESPACE;
use super::tasks::Head;
use crate::{Annotations, Field, Value};
use crate::{xml_text, xml_write};
use std::io::{self, Write};

pub(super) fn open<'source>(
    out: &mut impl Write,
    head: Head<'_, 'source>,
    fields: &[Field<'source>],
    flags: &[bool],
    depth: usize,
    annotations: Annotations<'_>,
) -> io::Result<Option<&'source str>> {
    let (type_name, fallback) = match head {
        Head::Record(tree, _) => (tree.type_name(), "p4kx:record"),
        Head::Nested(type_name) => (type_name, "p4kx:struct"),
    };
    let plain = xml_text::is_plain_name(type_name);
    let name = if plain { type_name } else { fallback };
    xml_write::indent(out, depth)?;
    out.write_all(b"<")?;
    out.write_all(name.as_bytes())?;
    if let Head::Record(_, true) = head {
        xml_write::attribute(out, "xmlns:p4kx", RECORD_XML_NAMESPACE)?;
    }
    if !plain {
        xml_write::attribute(out, "type", type_name)?;
    }
    record_header(out, head)?;
    for (field, _) in fields
        .iter()
        .zip(flags)
        .filter(|(_, attribute)| **attribute)
    {
        xml_write::write_attribute_field(out, field, annotations)?;
    }
    if flags.iter().all(|attribute| *attribute) {
        out.write_all(b"/>\n")?;
        Ok(None)
    } else {
        out.write_all(b">\n")?;
        Ok(Some(name))
    }
}

fn record_header(out: &mut impl Write, head: Head<'_, '_>) -> io::Result<()> {
    if let Head::Record(tree, _) = head {
        out.write_all(b" guid=\"")?;
        write!(out, "{}", tree.guid())?;
        out.write_all(b"\"")?;
        xml_write::attribute(out, "path", tree.record_path())?;
        xml_write::attribute(out, "p4kx:name", tree.name())?;
        if let Some(tag) = tree.tag() {
            xml_write::attribute(out, "p4kx:tag", tag)?;
        }
    }
    Ok(())
}

pub(super) fn open_field<'source>(
    out: &mut impl Write,
    field: &'source str,
    empty: bool,
    depth: usize,
) -> io::Result<&'source str> {
    xml_write::indent(out, depth)?;
    let name = if xml_text::is_plain_name(field) {
        out.write_all(b"<")?;
        out.write_all(field.as_bytes())?;
        field
    } else {
        out.write_all(b"<p4kx:field name=\"")?;
        xml_text::write_escaped(out, field)?;
        out.write_all(b"\"")?;
        "p4kx:field"
    };
    out.write_all(if empty { b"/>\n" } else { b">\n" })?;
    Ok(name)
}

pub(super) fn close(out: &mut impl Write, name: &str, depth: usize) -> io::Result<()> {
    xml_write::indent(out, depth)?;
    out.write_all(b"</")?;
    out.write_all(name.as_bytes())?;
    out.write_all(b">\n")
}

pub(super) fn field_leaf(
    out: &mut impl Write,
    field: &Field<'_>,
    depth: usize,
    annotations: Annotations<'_>,
) -> io::Result<()> {
    xml_write::indent(out, depth)?;
    if let Value::Error(error) = field.value() {
        out.write_all(b"<p4kx:error field=\"")?;
        xml_text::write_escaped(out, field.name())?;
        out.write_all(b"\"")?;
        return xml_write::write_error_end(out, error);
    }
    out.write_all(b"<p4kx:field name=\"")?;
    xml_text::write_escaped(out, field.name())?;
    out.write_all(b"\" value=\"")?;
    xml_write::write_scalar(out, field.value())?;
    out.write_all(b"\"")?;
    xml_write::write_annotation(out, field.value(), annotations, b"/>\n")
}

pub(super) fn value_leaf(
    out: &mut impl Write,
    value: &Value<'_>,
    depth: usize,
    annotations: Annotations<'_>,
) -> io::Result<()> {
    xml_write::indent(out, depth)?;
    match value {
        Value::Reference(reference) => xml_write::write_reference(out, reference, annotations),
        Value::Null => out.write_all(b"<p4kx:null/>\n"),
        Value::Error(error) => {
            out.write_all(b"<p4kx:error")?;
            xml_write::write_error_end(out, error)
        }
        scalar => {
            out.write_all(b"<p4kx:value value=\"")?;
            xml_write::write_scalar(out, scalar)?;
            out.write_all(b"\"")?;
            xml_write::write_annotation(out, scalar, annotations, b"/>\n")
        }
    }
}
