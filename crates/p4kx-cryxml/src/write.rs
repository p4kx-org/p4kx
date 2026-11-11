// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::document::Document;
use crate::element::{Children, Element};
use crate::escape::{Context, write_escaped};
use crate::names::{AttributeNames, sanitized};
use crate::xml_write_field::{FieldSite, Tracked, XmlFieldKind, XmlWriteField};

const DECLARATION: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";
const MAX_INDENT_LEVELS: usize = 64;
const INDENT: [u8; 2 * MAX_INDENT_LEVELS] = [b' '; 2 * MAX_INDENT_LEVELS];

/// Writes `document` as indented XML without recursion; indentation stops growing at 64 levels
/// so that deep input cannot blow up the output quadratically.
pub(crate) fn write<W: Write>(
    document: &Document<'_>,
    out: W,
    observe: impl FnMut(XmlWriteField) -> io::Result<()>,
) -> io::Result<()> {
    let mut writer = Writer {
        out: Tracked::new(out),
        names: AttributeNames::default(),
        observe,
    };
    writer.out.write_all(DECLARATION)?;
    let mut open: Vec<(Element<'_>, Children<'_>)> = Vec::new();
    let root = document.root();
    if writer.start(root, 0)? {
        open.push((root, root.children()));
    }
    while let Some((element, children)) = open.last_mut() {
        if let Some(child) = children.next() {
            if writer.start(child, open.len())? {
                open.push((child, child.children()));
            }
        } else {
            let element = *element;
            open.pop();
            writer.indent(open.len())?;
            writer.end_tag(element)?;
        }
    }
    writer.out.flush()
}

struct Writer<'d, W, F> {
    out: Tracked<W>,
    names: AttributeNames<'d>,
    observe: F,
}

impl<'d, W: Write, F: FnMut(XmlWriteField) -> io::Result<()>> Writer<'d, W, F> {
    /// Writes the start of `element`, or all of it when it has no children; returns whether its
    /// children and end tag remain to be written.
    fn start(&mut self, element: Element<'d>, depth: usize) -> io::Result<bool> {
        self.indent(depth)?;
        self.out.write_all(b"<")?;
        self.write_node_name(element, XmlFieldKind::ElementName)?;
        self.attributes(element)?;
        let has_children = element.has_children();
        match element.content() {
            None if !has_children => self.out.write_all(b"/>\n")?,
            None => self.out.write_all(b">\n")?,
            Some(content) => {
                self.out.write_all(b">")?;
                self.out.field(
                    FieldSite::node(element.index(), XmlFieldKind::Text),
                    |out| write_escaped(out, content, Context::Text),
                    &mut self.observe,
                )?;
                if has_children {
                    self.out.write_all(b"\n")?;
                } else {
                    self.end_tag(element)?;
                }
            }
        }
        Ok(has_children)
    }

    fn attributes(&mut self, element: Element<'d>) -> io::Result<()> {
        let first = element.first_attribute_index();
        let names = self.names.resolve(element.attributes());
        for ((name, attribute), index) in names.iter().zip(element.attributes()).zip(first..) {
            self.out.write_all(b" ")?;
            let mut site =
                FieldSite::attribute(element.index(), index, XmlFieldKind::AttributeName);
            if name.disambiguated {
                site.preceding_names = Some(first..index);
            }
            self.out.field(
                site,
                |out| out.write_all(name.name.as_bytes()),
                &mut self.observe,
            )?;
            self.out.write_all(b"=\"")?;
            self.out.field(
                FieldSite::attribute(element.index(), index, XmlFieldKind::AttributeValue),
                |out| write_escaped(out, attribute.value(), Context::Attribute),
                &mut self.observe,
            )?;
            self.out.write_all(b"\"")?;
        }
        Ok(())
    }

    fn write_node_name(&mut self, element: Element<'_>, kind: XmlFieldKind) -> io::Result<()> {
        self.out.field(
            FieldSite::node(element.index(), kind),
            |out| out.write_all(sanitized(element.name()).as_bytes()),
            &mut self.observe,
        )
    }

    fn end_tag(&mut self, element: Element<'_>) -> io::Result<()> {
        self.out.write_all(b"</")?;
        self.write_node_name(element, XmlFieldKind::ClosingElementName)?;
        self.out.write_all(b">\n")
    }

    fn indent(&mut self, depth: usize) -> io::Result<()> {
        let width = 2 * depth.min(MAX_INDENT_LEVELS);
        self.out.write_all(INDENT.get(..width).unwrap_or_default())
    }
}
