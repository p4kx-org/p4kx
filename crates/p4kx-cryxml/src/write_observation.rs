// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use crate::document::Document;
use crate::element::Element;
use crate::xml_render_span::RawSpans;
use crate::xml_write_field::FieldSite;
use crate::{XmlFieldKind, XmlRenderKind, XmlRenderSpan, XmlWriteField};

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
            positioned(source_span(
                self.input,
                &observed.site,
                observed.range.clone(),
                None,
            ))?;
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

pub(crate) fn conditional_child(input: &Document<'_>, element: Element<'_>) -> (RawSpans, bool) {
    let edge = element
        .children()
        .next()
        .and_then(|child| input.source.parent(child.index()));
    let complete = edge.is_some();
    ([edge, None, None, None], complete)
}

pub(crate) fn empty_content(input: &Document<'_>, element: Element<'_>) -> (RawSpans, bool) {
    let text = FieldSite::node(element.index(), XmlFieldKind::Text);
    let ([edge, _, _, _], child_known) = conditional_child(input, element);
    if let Some([offset, _, prefix, terminator]) = input.source.field(input, &text, None) {
        ([offset, edge, prefix, terminator], child_known)
    } else {
        ([edge, None, None, None], false)
    }
}

fn source_span(
    input: &Document<'_>,
    source: &FieldSite,
    emitted: Range<u64>,
    section: Option<Range<usize>>,
) -> XmlRenderSpan {
    let observed = input.source.field(input, source, section);
    let complete = observed.is_some();
    let sources = observed.unwrap_or([None, None, None, None]);
    XmlRenderSpan::new(
        XmlRenderKind::Field(source.kind),
        emitted,
        sources,
        complete,
    )
}
