// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use crate::document::Document;
use crate::element::Element;
use crate::xml_render_span::RawSpans;
use crate::xml_write_field::FieldSite;
use crate::{XmlFieldKind, XmlRenderKind, XmlRenderSpan, XmlWriteField};

#[derive(Clone, Copy)]
pub(crate) enum ChildMarkup {
    Children,
    EmptyText,
}

pub(crate) struct Observation<'document, FieldObserver, SpanObserver> {
    input: &'document Document<'document>,
    legacy: FieldObserver,
    positioned: Option<SpanObserver>,
}

impl<'document, FieldObserver, SpanObserver> Observation<'document, FieldObserver, SpanObserver>
where
    FieldObserver: FnMut(XmlWriteField) -> io::Result<()>,
    SpanObserver: FnMut(XmlRenderSpan) -> io::Result<()>,
{
    pub(crate) const fn new(
        input: &'document Document<'document>,
        legacy: FieldObserver,
        positioned: Option<SpanObserver>,
    ) -> Self {
        Self {
            input,
            legacy,
            positioned,
        }
    }

    pub(crate) const fn is_positioned(&self) -> bool {
        self.positioned.is_some()
    }

    pub(crate) fn field(&mut self, observed: XmlWriteField) -> io::Result<()> {
        if let Some(positioned) = &mut self.positioned {
            let span = source_span(self.input, &observed.site, observed.range.clone(), None);
            positioned(span.with_attribute_selection(&observed))?;
        }
        (self.legacy)(observed)
    }

    pub(crate) fn text(
        &mut self,
        site: &FieldSite,
        emitted: Range<u64>,
        section: Range<usize>,
    ) -> io::Result<()> {
        if let Some(positioned) = &mut self.positioned {
            positioned(source_span(self.input, site, emitted, Some(section)))?;
        }
        Ok(())
    }

    pub(crate) fn indentation(
        &mut self,
        emitted: Range<u64>,
        levels: usize,
        nodes: impl Iterator<Item = u32>,
    ) -> io::Result<()> {
        if let Some(positioned) = &mut self.positioned {
            let mut sources = Vec::with_capacity(levels);
            sources.extend(nodes.filter_map(|node| self.input.source.parent(node)));
            let complete = sources.len() == levels;
            positioned(XmlRenderSpan::indentation(emitted, sources, complete))?;
        }
        Ok(())
    }

    pub(crate) fn child_markup(
        &mut self,
        emitted: Range<u64>,
        element: Element<'_>,
        kind: ChildMarkup,
    ) -> io::Result<()> {
        if let Some(positioned) = &mut self.positioned {
            positioned(child_span(self.input, emitted, element, kind))?;
        }
        Ok(())
    }

    pub(crate) fn markup(
        &mut self,
        kind: XmlRenderKind,
        emitted: Range<u64>,
        facts: impl FnOnce(&Document<'_>) -> (RawSpans, bool),
    ) -> io::Result<()> {
        if let Some(positioned) = &mut self.positioned {
            let (sources, complete) = facts(self.input);
            positioned(XmlRenderSpan::new(kind, emitted, sources, complete))?;
        }
        Ok(())
    }
}

pub(crate) const fn no_sources() -> (RawSpans, bool) {
    ([None, None, None, None], true)
}

fn child_span(
    input: &Document<'_>,
    emitted: Range<u64>,
    element: Element<'_>,
    kind: ChildMarkup,
) -> XmlRenderSpan {
    let child = element.children().next();
    let edge = child.and_then(|child| input.source.parent(child.index()));
    let child_known = edge.is_some();
    let (sources, text_known) = match kind {
        ChildMarkup::Children => ([edge, None, None, None], true),
        ChildMarkup::EmptyText => {
            let site = FieldSite::node(element.index(), XmlFieldKind::Text);
            match input.source.field(input, &site, None) {
                Some([offset, _, prefix, terminator]) => ([offset, edge, prefix, terminator], true),
                None => ([edge, None, None, None], false),
            }
        }
    };
    if child.is_none()
        && text_known
        && let Some(proof) = input
            .source
            .child_absence(element.index(), input.byte_order())
    {
        return XmlRenderSpan::without_children(emitted, sources, proof);
    }
    XmlRenderSpan::new(
        XmlRenderKind::Structure,
        emitted,
        sources,
        child_known && text_known,
    )
}

fn source_span(
    input: &Document<'_>,
    source: &FieldSite,
    emitted: Range<u64>,
    section: Option<Range<usize>>,
) -> XmlRenderSpan {
    let observed = input.source.field(input, source, section);
    if source.namespace.is_some()
        && let Some(attribute) = source.attribute
    {
        let value = FieldSite::attribute(source.node, attribute, XmlFieldKind::AttributeValue);
        let value = input.source.field(input, &value, None);
        return XmlRenderSpan::namespace_name(emitted, observed, value);
    }
    let complete = observed.is_some();
    let sources = observed.unwrap_or([None, None, None, None]);
    XmlRenderSpan::new(
        XmlRenderKind::Field(source.kind),
        emitted,
        sources,
        complete,
    )
}
