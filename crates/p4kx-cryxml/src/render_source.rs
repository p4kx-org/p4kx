// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::document::Document;
use crate::header::{ATTRIBUTE_LEN, Header, NODE_LEN, Reader};
use crate::strings::Span;
use crate::xml_render_span::RawSpans;
use crate::xml_write_field::FieldSite;
use crate::{ByteOrder, XmlChildAbsence, XmlFieldKind, XmlParentWords, XmlRawKind, XmlRawSpan};

const NODE_NAME: usize = 0;
const NODE_TEXT: usize = 4;
const NODE_ATTRIBUTE_COUNT: usize = 8;
const NODE_PARENT: usize = 12;
const NODE_FIRST_ATTRIBUTE: usize = 16;
const ATTRIBUTE_NAME: usize = 0;
const ATTRIBUTE_VALUE: usize = 4;
const WORD: usize = size_of::<u32>();

#[derive(Debug, Clone)]
pub(crate) struct RenderSource<'input> {
    nodes: Table<'input>,
    node_directory: Range<u64>,
    attributes: Table<'input>,
    strings: Table<'input>,
}

impl<'input> RenderSource<'input> {
    pub(crate) fn of(header: &Header<'input>) -> Self {
        let [nodes, attributes, strings] = header.render_starts();
        Self {
            nodes: Table::new(header.nodes, nodes),
            node_directory: header.node_directory.clone(),
            attributes: Table::new(header.attributes, attributes),
            strings: Table::new(header.strings, strings),
        }
    }

    pub(crate) fn field(
        &self,
        document: &Document<'_>,
        site: &FieldSite,
        section: Option<Range<usize>>,
    ) -> Option<RawSpans> {
        let (field, string) = self.word_and_string(document, site)?;
        let offset = u32::try_from(field.range().start.checked_sub(match site.kind {
            XmlFieldKind::AttributeName | XmlFieldKind::AttributeValue => self.attributes.base,
            _ => self.nodes.base,
        })?)
        .ok()?;
        let table = if site.attribute.is_some() {
            &self.attributes
        } else {
            &self.nodes
        };
        let raw = table.word(offset as usize, document.byte_order())?;
        let start = string.start() as usize;
        let end = string.end() as usize;
        let section = section.unwrap_or(0..end.checked_sub(start)?);
        let shown = start.checked_add(section.start)?..start.checked_add(section.end)?;
        if shown.start > shown.end || shown.end > end || raw > string.start() {
            return None;
        }
        let text = if shown.is_empty() {
            None
        } else {
            Some(self.strings.site(shown, XmlRawKind::StringBytes)?)
        };
        let prefix = if raw < string.start() {
            Some(
                self.strings
                    .site(raw as usize..start, XmlRawKind::StringBoundary)?,
            )
        } else {
            None
        };
        let terminator = self
            .strings
            .site(end..end.checked_add(1)?, XmlRawKind::StringTerminator)?;
        Some([Some(field), text, prefix, Some(terminator)])
    }

    pub(crate) fn attribute_group(&self, node: u32) -> RawSpans {
        [
            self.node(
                node,
                NODE_ATTRIBUTE_COUNT,
                size_of::<u16>(),
                XmlRawKind::AttributeCount,
            ),
            self.node(node, NODE_FIRST_ATTRIBUTE, WORD, XmlRawKind::FirstAttribute),
            None,
            None,
        ]
    }

    pub(crate) fn parent(&self, node: u32) -> Option<XmlRawSpan> {
        self.node(node, NODE_PARENT, WORD, XmlRawKind::NodeParent)
    }

    pub(crate) fn child_absence(&self, node: u32, order: ByteOrder) -> Option<XmlChildAbsence> {
        let first = node.checked_add(1)?;
        let count = u32::try_from(self.nodes.bytes.len() / NODE_LEN).ok()?;
        let remaining = count.checked_sub(first)?;
        let parents = if remaining == 0 {
            None
        } else {
            let start = self.parent(first)?.range().start;
            Some(XmlParentWords::new(start, remaining))
        };
        Some(XmlChildAbsence::new(
            node,
            order,
            self.node_directory.clone(),
            parents,
        ))
    }

    fn word_and_string(
        &self,
        input: &Document<'_>,
        site: &FieldSite,
    ) -> Option<(XmlRawSpan, Span)> {
        let (word, string) = match site.kind {
            XmlFieldKind::ElementName | XmlFieldKind::ClosingElementName => (
                self.node(site.node, NODE_NAME, WORD, XmlRawKind::NodeNameOffset),
                input.nodes.get(site.node as usize)?.name,
            ),
            XmlFieldKind::Text => (
                self.node(site.node, NODE_TEXT, WORD, XmlRawKind::NodeTextOffset),
                input.nodes.get(site.node as usize)?.content,
            ),
            XmlFieldKind::AttributeName => {
                let attribute = site.attribute? as usize;
                (
                    self.attributes.row(
                        attribute,
                        ATTRIBUTE_LEN,
                        ATTRIBUTE_NAME,
                        WORD,
                        XmlRawKind::AttributeNameOffset,
                    ),
                    input.attributes.get(attribute)?.0,
                )
            }
            XmlFieldKind::AttributeValue => {
                let attribute = site.attribute? as usize;
                (
                    self.attributes.row(
                        attribute,
                        ATTRIBUTE_LEN,
                        ATTRIBUTE_VALUE,
                        WORD,
                        XmlRawKind::AttributeValueOffset,
                    ),
                    input.attributes.get(attribute)?.1,
                )
            }
        };
        Some((word?, string))
    }

    fn node(
        &self,
        index: u32,
        column: usize,
        width: usize,
        kind: XmlRawKind,
    ) -> Option<XmlRawSpan> {
        self.nodes
            .row(index as usize, NODE_LEN, column, width, kind)
    }
}

#[derive(Debug, Clone)]
struct Table<'input> {
    bytes: &'input [u8],
    base: u64,
}

impl<'input> Table<'input> {
    const fn new(bytes: &'input [u8], base: usize) -> Self {
        Self {
            bytes,
            base: base as u64,
        }
    }

    fn row(
        &self,
        index: usize,
        stride: usize,
        column: usize,
        width: usize,
        kind: XmlRawKind,
    ) -> Option<XmlRawSpan> {
        let start = index.checked_mul(stride)?.checked_add(column)?;
        self.site(start..start.checked_add(width)?, kind)
    }

    fn site(&self, local: Range<usize>, kind: XmlRawKind) -> Option<XmlRawSpan> {
        self.bytes.get(local.clone())?;
        let start = self.base.checked_add(local.start as u64)?;
        let end = self.base.checked_add(local.end as u64)?;
        Some(XmlRawSpan::new(kind, start..end))
    }

    fn word(&self, offset: usize, order: ByteOrder) -> Option<u32> {
        let bytes = self.bytes.get(offset..offset.checked_add(WORD)?)?;
        Reader::new(bytes, order).u32().ok()
    }
}
