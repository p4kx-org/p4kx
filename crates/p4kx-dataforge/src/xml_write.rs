// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::texts::annotation;
use crate::xml_text::{write_display, write_escaped, write_float};
use crate::{Annotations, Field, Reference, Resolution, Value, ValueError};

const INDENT: &[u8; 64] = b"                                                                ";

pub(crate) fn indent(out: &mut impl Write, depth: usize) -> io::Result<()> {
    let mut spaces = depth.saturating_mul(2);
    while spaces > 0 {
        let chunk = spaces.min(INDENT.len());
        out.write_all(INDENT.get(..chunk).unwrap_or_default())?;
        spaces -= chunk;
    }
    Ok(())
}

pub(crate) fn attribute(out: &mut impl Write, name: &str, value: &str) -> io::Result<()> {
    out.write_all(b" ")?;
    out.write_all(name.as_bytes())?;
    out.write_all(b"=\"")?;
    write_escaped(out, value)?;
    out.write_all(b"\"")
}

pub(crate) fn write_reference(
    out: &mut impl Write,
    reference: &Reference<'_>,
    annotations: Annotations<'_>,
) -> io::Result<()> {
    out.write_all(b"<p4kx:reference guid=\"")?;
    write!(out, "{}", reference.guid())?;
    out.write_all(b"\"")?;
    attribute(out, "path", reference.record_path())?;
    attribute(out, "type", reference.type_name())?;
    if let Some(location) = reference.location() {
        attribute(out, "at", location.as_str())?;
    }
    if let Some(path) = annotations.tag_path(reference) {
        attribute(out, "tag", path.as_ref())?;
    }
    out.write_all(b"/>\n")
}

/// Writes the `reason` attribute of an error value and, for a dangling Reference, the `guid` of
/// the missing Record, then closes the element.
pub(crate) fn write_error_end(out: &mut impl Write, error: &ValueError) -> io::Result<()> {
    out.write_all(b" reason=\"")?;
    write_display(out, error)?;
    out.write_all(b"\"")?;
    if let Some(guid) = error.dangling_guid() {
        write!(out, " guid=\"{guid}\"")?;
    }
    out.write_all(b"/>\n")
}

/// Writes a scalar field as an attribute, followed by the `p4kx:text-` or `p4kx:missing-`
/// attribute of its localisation key.
pub(crate) fn write_attribute_field(
    out: &mut impl Write,
    field: &Field<'_>,
    annotations: Annotations<'_>,
) -> io::Result<()> {
    out.write_all(b" ")?;
    out.write_all(field.name().as_bytes())?;
    out.write_all(b"=\"")?;
    write_scalar(out, field.value())?;
    out.write_all(b"\"")?;
    if let Value::Locale(key) = field.value() {
        match annotation(&annotations, key) {
            Some(Resolution::Text(text)) => {
                out.write_all(b" p4kx:text-")?;
                out.write_all(field.name().as_bytes())?;
                out.write_all(b"=\"")?;
                write_escaped(out, &text)?;
                out.write_all(b"\"")?;
            }
            Some(_) => {
                out.write_all(b" p4kx:missing-")?;
                out.write_all(field.name().as_bytes())?;
                out.write_all(b"=\"true\"")?;
            }
            None => {}
        }
    }
    Ok(())
}

/// Writes the `text` or `missing` attribute of a localisation key and then `end`; any other
/// value, a kept key and default output write just `end`.
pub(crate) fn write_annotation(
    out: &mut impl Write,
    value: &Value<'_>,
    annotations: Annotations<'_>,
    end: &[u8],
) -> io::Result<()> {
    if let Value::Locale(key) = value {
        match annotation(&annotations, key) {
            Some(Resolution::Text(text)) => {
                out.write_all(b" text=\"")?;
                write_escaped(out, &text)?;
                out.write_all(b"\"")?;
            }
            Some(_) => out.write_all(b" missing=\"true\"")?,
            None => {}
        }
    }
    out.write_all(end)
}

/// Writes the text of a scalar; other values write nothing.
pub(crate) fn write_scalar(out: &mut impl Write, value: &Value<'_>) -> io::Result<()> {
    match value {
        Value::Bool(value) => out.write_all(if *value { b"true" } else { b"false" }),
        Value::Int8(value) => write!(out, "{value}"),
        Value::Int16(value) => write!(out, "{value}"),
        Value::Int32(value) => write!(out, "{value}"),
        Value::Int64(value) => write!(out, "{value}"),
        Value::UInt8(value) => write!(out, "{value}"),
        Value::UInt16(value) => write!(out, "{value}"),
        Value::UInt32(value) => write!(out, "{value}"),
        Value::UInt64(value) => write!(out, "{value}"),
        Value::Single(value) => write_float(out, *value),
        Value::Double(value) => write_float(out, *value),
        Value::String(text) | Value::Locale(text) | Value::Enum(text) => write_escaped(out, text),
        Value::Guid(guid) => write!(out, "{guid}"),
        Value::Struct(_)
        | Value::Array(_)
        | Value::Reference(_)
        | Value::Null
        | Value::Error(_) => Ok(()),
    }
}
