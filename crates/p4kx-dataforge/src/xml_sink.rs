// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use super::observation::{
    Output, RecordXmlHeaderMember as Header, RecordXmlSite as Site, RecordXmlValueMember as Member,
};
use super::record_xml::RECORD_XML_NAMESPACE;
use super::tasks::{Head, Name};
use super::writer as xml_write;
use crate::xml_text;
use crate::{Annotations, Field, Value};

pub(super) fn open<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    head: Head<'tree, 'source>,
    fields: &'tree [Field<'source>],
    flags: &[bool],
    depth: usize,
    annotations: Annotations<'_>,
) -> io::Result<Option<Name<'tree, 'source>>> {
    let (type_name, fallback, original) = match head {
        Head::Record(tree, _) => (tree.type_name(), "p4kx:record", Site::Header(Header::Type)),
        Head::Nested { type_name, value } => {
            (type_name, "p4kx:struct", Site::member(value, Member::Value))
        }
    };
    let plain = xml_text::is_plain_name(type_name);
    let name = Name {
        text: if plain { type_name } else { fallback },
        origin: plain.then_some(original),
    };
    xml_write::indent(out, depth)?;
    out.write_all(b"<")?;
    write_name(out, name)?;
    if let Head::Record(_, true) = head {
        xml_write::attribute(out, "xmlns:p4kx", RECORD_XML_NAMESPACE, None)?;
    }
    if !plain {
        xml_write::attribute(out, "type", type_name, Some(original))?;
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

fn record_header<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    head: Head<'tree, 'source>,
) -> io::Result<()> {
    if let Head::Record(tree, _) = head {
        xml_write::attribute_with(out, "guid", Some(Site::Header(Header::Guid)), |out| {
            write!(out, "{}", tree.guid())
        })?;
        xml_write::attribute(
            out,
            "path",
            tree.record_path(),
            Some(Site::Header(Header::Path)),
        )?;
        xml_write::attribute(
            out,
            "p4kx:name",
            tree.name(),
            Some(Site::Header(Header::Name)),
        )?;
        if let Some(tag) = tree.tag() {
            xml_write::attribute(out, "p4kx:tag", tag, Some(Site::Header(Header::Tag)))?;
        }
    }
    Ok(())
}

pub(super) fn open_field<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    field: &'tree Field<'source>,
    empty: bool,
    depth: usize,
) -> io::Result<Name<'tree, 'source>> {
    xml_write::indent(out, depth)?;
    let original = Some(Site::member(field.value(), Member::Name));
    let name = if xml_text::is_plain_name(field.name()) {
        let name = Name {
            text: field.name(),
            origin: original,
        };
        out.write_all(b"<")?;
        write_name(out, name)?;
        name
    } else {
        out.write_all(b"<p4kx:field")?;
        xml_write::attribute(out, "name", field.name(), original)?;
        Name {
            text: "p4kx:field",
            origin: None,
        }
    };
    out.write_all(if empty { b"/>\n" } else { b">\n" })?;
    Ok(name)
}

pub(super) fn close<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    name: Name<'tree, 'source>,
    depth: usize,
) -> io::Result<()> {
    xml_write::indent(out, depth)?;
    out.write_all(b"</")?;
    write_name(out, name)?;
    out.write_all(b">\n")
}

fn write_name<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    name: Name<'tree, 'source>,
) -> io::Result<()> {
    out.fragment(name.origin, |out| out.write_all(name.text.as_bytes()))
}

pub(super) fn field_leaf<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    field: &'tree Field<'source>,
    depth: usize,
    annotations: Annotations<'_>,
) -> io::Result<()> {
    xml_write::indent(out, depth)?;
    let value = field.value();
    let name = Some(Site::member(value, Member::Name));
    if let Value::Error(error) = value {
        out.write_all(b"<p4kx:error")?;
        xml_write::attribute(out, "field", field.name(), name)?;
        return xml_write::write_error_end(out, value, error);
    }
    out.write_all(b"<p4kx:field")?;
    xml_write::attribute(out, "name", field.name(), name)?;
    xml_write::value_attribute(out, "value", value)?;
    xml_write::write_annotation(out, value, annotations, b"/>\n")
}

pub(super) fn value_leaf<'tree, 'source>(
    out: &mut Output<'_, 'tree, 'source, impl Write>,
    value: &'tree Value<'source>,
    depth: usize,
    annotations: Annotations<'_>,
) -> io::Result<()> {
    xml_write::indent(out, depth)?;
    match value {
        Value::Reference(reference) => {
            xml_write::write_reference(out, value, reference, annotations)
        }
        Value::Null => {
            out.write_all(b"<")?;
            out.fragment(Some(Site::member(value, Member::Value)), |out| {
                out.write_all(b"p4kx:null")
            })?;
            out.write_all(b"/>\n")
        }
        Value::Error(error) => {
            out.write_all(b"<p4kx:error")?;
            xml_write::write_error_end(out, value, error)
        }
        scalar => {
            out.write_all(b"<p4kx:value")?;
            xml_write::value_attribute(out, "value", scalar)?;
            xml_write::write_annotation(out, scalar, annotations, b"/>\n")
        }
    }
}
