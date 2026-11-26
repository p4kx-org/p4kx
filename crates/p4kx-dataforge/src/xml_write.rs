// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use super::observation::{Output, RecordXmlSite as Site, RecordXmlValueMember as Member};
use crate::texts::annotation;
use crate::xml_text::{write_display, write_escaped, write_float};
use crate::{Annotations, Field, Reference, Resolution, Value, ValueError};

const INDENT: &[u8; 64] = b"                                                                ";

pub(super) fn indent(out: &mut impl Write, depth: usize) -> io::Result<()> {
    let mut spaces = depth.saturating_mul(2);
    while spaces > 0 {
        let chunk = spaces.min(INDENT.len());
        out.write_all(INDENT.get(..chunk).unwrap_or_default())?;
        spaces -= chunk;
    }
    Ok(())
}

pub(super) fn attribute<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    name: &str,
    value: &str,
    origin: Option<Site<'tree, 'source>>,
) -> io::Result<()> {
    attribute_with(out, name, origin, |out| write_escaped(out, value))
}

pub(super) fn attribute_with<'out, 'tree, 'source, W: Write>(
    out: &mut Output<'out, 'tree, 'source, W>,
    name: &str,
    origin: Option<Site<'tree, 'source>>,
    write: impl FnOnce(&mut Output<'out, 'tree, 'source, W>) -> io::Result<()>,
) -> io::Result<()> {
    out.write_all(b" ")?;
    out.write_all(name.as_bytes())?;
    attribute_value(out, origin, write)
}

fn attribute_value<'out, 'tree, 'source, W: Write>(
    out: &mut Output<'out, 'tree, 'source, W>,
    origin: Option<Site<'tree, 'source>>,
    write: impl FnOnce(&mut Output<'out, 'tree, 'source, W>) -> io::Result<()>,
) -> io::Result<()> {
    out.write_all(b"=\"")?;
    out.fragment(origin, write)?;
    out.write_all(b"\"")
}

pub(super) fn value_attribute<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    name: &str,
    value: &'tree Value<'source>,
) -> io::Result<()> {
    attribute_with(out, name, Some(Site::member(value, Member::Value)), |out| {
        write_scalar(out, value)
    })
}

pub(super) fn write_reference<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    value: &'tree Value<'source>,
    reference: &Reference<'_>,
    annotations: Annotations<'_>,
) -> io::Result<()> {
    let site = |member| Some(Site::member(value, member));
    out.write_all(b"<p4kx:reference")?;
    attribute_with(out, "guid", site(Member::Guid), |out| {
        write!(out, "{}", reference.guid())
    })?;
    attribute(out, "path", reference.record_path(), site(Member::Path))?;
    attribute(out, "type", reference.type_name(), site(Member::Type))?;
    if let Some(location) = reference.location() {
        attribute(out, "at", location.as_str(), site(Member::InstancePath))?;
    }
    if let Some(path) = annotations.tag_path(reference) {
        attribute(out, "tag", path.as_ref(), site(Member::Tag))?;
    }
    out.write_all(b"/>\n")
}

pub(super) fn write_error_end<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    value: &'tree Value<'source>,
    error: &ValueError,
) -> io::Result<()> {
    attribute_with(
        out,
        "reason",
        Some(Site::member(value, Member::Value)),
        |out| write_display(out, error),
    )?;
    if let Some(guid) = error.dangling_guid() {
        attribute_with(
            out,
            "guid",
            Some(Site::member(value, Member::Guid)),
            |out| write!(out, "{guid}"),
        )?;
    }
    out.write_all(b"/>\n")
}

pub(super) fn write_attribute_field<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    field: &'tree Field<'source>,
    annotations: Annotations<'_>,
) -> io::Result<()> {
    let value = field.value();
    out.write_all(b" ")?;
    out.fragment(Some(Site::member(value, Member::Name)), |out| {
        out.write_all(field.name().as_bytes())
    })?;
    attribute_value(out, Some(Site::member(value, Member::Value)), |out| {
        write_scalar(out, value)
    })?;
    localized(out, value, Some(field.name()), annotations)
}

pub(super) fn write_annotation<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    value: &'tree Value<'source>,
    annotations: Annotations<'_>,
    end: &[u8],
) -> io::Result<()> {
    localized(out, value, None, annotations)?;
    out.write_all(end)
}

fn localized<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    value: &'tree Value<'source>,
    field: Option<&str>,
    annotations: Annotations<'_>,
) -> io::Result<()> {
    let Value::Locale(key) = value else {
        return Ok(());
    };
    let Some(resolved) = annotation(&annotations, key) else {
        return Ok(());
    };
    let name: &[u8] = if matches!(resolved, Resolution::Text(_)) {
        b"text"
    } else {
        b"missing"
    };
    out.write_all(b" ")?;
    if let Some(field) = field {
        out.write_all(b"p4kx:")?;
        out.write_all(name)?;
        out.write_all(b"-")?;
        out.fragment(Some(Site::member(value, Member::Name)), |out| {
            out.write_all(field.as_bytes())
        })?;
    } else {
        out.write_all(name)?;
    }
    attribute_value(
        out,
        Some(Site::member(value, Member::Translation)),
        |out| match resolved {
            Resolution::Text(text) => write_escaped(out, &text),
            _ => out.write_all(b"true"),
        },
    )
}

/// Writes the text of a scalar; other values write nothing.
fn write_scalar(out: &mut impl Write, value: &Value<'_>) -> io::Result<()> {
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
