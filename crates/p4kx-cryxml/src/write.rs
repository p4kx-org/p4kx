// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use crate::document::Document;
use crate::element::{Children, Element};
use crate::escape::{Context, write_escaped};
use crate::names::{AttributeNames, DEFAULT_NAMESPACE_ATTRIBUTE, sanitized};
use crate::write_observation::{ChildMarkup, Observation, no_sources};
use crate::xml_render_span::RawSpans;
use crate::xml_write_field::{FieldSite, NamespaceChoice, Tracked, XmlFieldKind};
use crate::{XmlRenderKind, XmlRenderSpan, XmlWriteField};

const DECLARATION: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";
const MAX_INDENT_LEVELS: usize = 64;
const SPACES_PER_LEVEL: usize = 2;
const INDENT: [u8; SPACES_PER_LEVEL * MAX_INDENT_LEVELS] =
    [b' '; SPACES_PER_LEVEL * MAX_INDENT_LEVELS];

type OpenElement<'document> = (Element<'document>, Children<'document>);

/// Writes the original node order iteratively, with bounded indentation.
pub(crate) fn write<W: Write>(
    document: &Document<'_>,
    out: W,
    observe: impl FnMut(XmlWriteField) -> io::Result<()>,
    positioned: Option<impl FnMut(XmlRenderSpan) -> io::Result<()>>,
) -> io::Result<()> {
    let mut writer = Writer {
        out: Tracked::new(out),
        names: AttributeNames::default(),
        observation: Observation::new(document, observe, positioned),
    };
    writer.markup(DECLARATION, XmlRenderKind::Declaration, |_| no_sources())?;
    let mut open: Vec<OpenElement<'_>> = Vec::new();
    let root = document.root();
    if writer.start(root, &open)? {
        open.push((root, root.children()));
    }
    while let Some((element, children)) = open.last_mut() {
        if let Some(child) = children.next() {
            if writer.start(child, &open)? {
                open.push((child, child.children()));
            }
        } else {
            let element = *element;
            open.pop();
            writer.indent(element, &open)?;
            writer.end_tag(element)?;
        }
    }
    writer.out.flush()
}

struct Writer<'document, W, FieldObserver, SpanObserver> {
    out: Tracked<W>,
    names: AttributeNames<'document>,
    observation: Observation<'document, FieldObserver, SpanObserver>,
}

impl<'document, W, FieldObserver, SpanObserver> Writer<'document, W, FieldObserver, SpanObserver>
where
    W: Write,
    FieldObserver: FnMut(XmlWriteField) -> io::Result<()>,
    SpanObserver: FnMut(XmlRenderSpan) -> io::Result<()>,
{
    fn start(
        &mut self,
        element: Element<'document>,
        open: &[OpenElement<'document>],
    ) -> io::Result<bool> {
        self.indent(element, open)?;
        self.markup(b"<", XmlRenderKind::Structure, |input| {
            let [count, first, _, _] = input.source.attribute_group(element.index());
            let edge = input.source.parent(element.index());
            let complete = count.is_some() && first.is_some() && edge.is_some();
            ([count, first, edge, None], complete)
        })?;
        self.write_node_name(element, XmlFieldKind::ElementName)?;
        self.attributes(element)?;
        let has_children = element.has_children();
        match element.content() {
            None => {
                let close: &[u8] = if has_children { b">\n" } else { b"/>\n" };
                self.child_markup(close, element, ChildMarkup::EmptyText)?;
            }
            Some(content) => {
                self.literal(b">")?;
                self.text(element, content)?;
                if has_children {
                    self.child_markup(b"\n", element, ChildMarkup::Children)?;
                } else {
                    self.end_tag(element)?;
                }
            }
        }
        Ok(has_children)
    }

    fn attributes(&mut self, element: Element<'document>) -> io::Result<()> {
        let first = element.first_attribute_index();
        let names = self.names.resolve(element.attributes());
        let out = &mut self.out;
        let observation = &mut self.observation;
        for ((name, attribute), index) in names.iter().zip(element.attributes()).zip(first..) {
            literal(out, observation, b" ")?;
            let mut site =
                FieldSite::attribute(element.index(), index, XmlFieldKind::AttributeName);
            if name.disambiguated {
                site.preceding_names = Some(first..index);
            }
            if name.uses_namespace_alias() {
                let count = u32::try_from(names.len()).map_err(io::Error::other)?;
                let end = first
                    .checked_add(count)
                    .ok_or_else(|| io::Error::other("XML attribute index exceeds u32"))?;
                site.namespace = Some(NamespaceChoice::Alias(first..end));
            } else if name.name == DEFAULT_NAMESPACE_ATTRIBUTE {
                site.namespace = Some(NamespaceChoice::Default);
            }
            out.field(
                site,
                |out| out.write_all(name.name.as_bytes()),
                &mut |field| observation.field(field),
            )?;
            literal(out, observation, b"=\"")?;
            out.field(
                FieldSite::attribute(element.index(), index, XmlFieldKind::AttributeValue),
                |out| write_escaped(out, attribute.value(), Context::Attribute),
                &mut |field| observation.field(field),
            )?;
            literal(out, observation, b"\"")?;
        }
        Ok(())
    }

    fn text(&mut self, element: Element<'_>, content: &str) -> io::Result<()> {
        let site = FieldSite::node(element.index(), XmlFieldKind::Text);
        let observation = &mut self.observation;
        if !observation.is_positioned() {
            return self.out.field(
                site,
                |out| write_escaped(out, content, Context::Text),
                &mut |field| observation.field(field),
            );
        }
        let mut offset = 0;
        for section in content.split_inclusive('\n') {
            let start = self.out.offset();
            write_escaped(&mut self.out, section, Context::Text)?;
            let end = offset + section.len();
            observation.text(&site, start..self.out.offset(), offset..end)?;
            offset = end;
        }
        Ok(())
    }

    fn write_node_name(&mut self, element: Element<'_>, kind: XmlFieldKind) -> io::Result<()> {
        self.out.field(
            FieldSite::node(element.index(), kind),
            |out| out.write_all(sanitized(element.name()).as_bytes()),
            &mut |field| self.observation.field(field),
        )
    }

    fn end_tag(&mut self, element: Element<'_>) -> io::Result<()> {
        self.child_markup(b"</", element, ChildMarkup::Children)?;
        self.write_node_name(element, XmlFieldKind::ClosingElementName)?;
        self.literal(b">\n")
    }

    fn indent(&mut self, element: Element<'_>, open: &[OpenElement<'_>]) -> io::Result<()> {
        let levels = open.len().min(MAX_INDENT_LEVELS);
        let width = levels * SPACES_PER_LEVEL;
        let spaces = INDENT.get(..width).unwrap_or_default();
        if spaces.is_empty() {
            return Ok(());
        }
        let start = self.out.offset();
        self.out.write_all(spaces)?;
        let nodes = std::iter::once(element.index())
            .chain(open.iter().rev().map(|(parent, _)| parent.index()))
            .take(levels);
        self.observation
            .indentation(start..self.out.offset(), levels, nodes)
    }

    fn child_markup(
        &mut self,
        bytes: &[u8],
        element: Element<'_>,
        kind: ChildMarkup,
    ) -> io::Result<()> {
        let start = self.out.offset();
        self.out.write_all(bytes)?;
        self.observation
            .child_markup(start..self.out.offset(), element, kind)
    }

    fn literal(&mut self, bytes: &[u8]) -> io::Result<()> {
        literal(&mut self.out, &mut self.observation, bytes)
    }

    fn markup(
        &mut self,
        bytes: &[u8],
        kind: XmlRenderKind,
        facts: impl FnOnce(&Document<'_>) -> (RawSpans, bool),
    ) -> io::Result<()> {
        let start = self.out.offset();
        self.out.write_all(bytes)?;
        self.observation
            .markup(kind, start..self.out.offset(), facts)
    }
}

fn literal<W, FieldObserver, SpanObserver>(
    out: &mut Tracked<W>,
    observation: &mut Observation<'_, FieldObserver, SpanObserver>,
    bytes: &[u8],
) -> io::Result<()>
where
    W: Write,
    FieldObserver: FnMut(XmlWriteField) -> io::Result<()>,
    SpanObserver: FnMut(XmlRenderSpan) -> io::Result<()>,
{
    let start = out.offset();
    out.write_all(bytes)?;
    observation.markup(XmlRenderKind::Structure, start..out.offset(), |_| {
        no_sources()
    })
}
