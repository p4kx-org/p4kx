// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::Peekable;
use std::ops::Range;

use roxmltree::{Children as TextChildren, Node as TextNode, NodeType};

use crate::element::{Children, Element};
use crate::source_element::SourceElement;
use crate::source_position;
use crate::source_syntax::SourceSpanError;

/// The original coordinate of text or comment content.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceContentPosition {
    /// The original half-open byte range in text XML.
    Text(Range<usize>),
    /// The owning CryXml node table index for its content slot.
    CryXml(u32),
}

/// An original content occurrence among an element's children.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SourceContent<'document> {
    /// A child element at its original coordinate.
    Element(SourceElement<'document>),
    /// Decoded text and its original source extent.
    Text(SourceText<'document>),
    /// An XML comment at its original source extent.
    Comment(SourceText<'document>),
    /// An XML processing instruction.
    ProcessingInstruction {
        /// Authored target name.
        target: &'document str,
        /// Authored body when present.
        value: Option<&'document str>,
        /// Original half-open byte range of the instruction.
        position: Range<usize>,
    },
}

/// A borrowed text value and its original lexical evidence.
#[derive(Debug, Clone)]
pub struct SourceText<'document> {
    value: &'document str,
    lexeme: Option<&'document str>,
    position: SourceContentPosition,
}

impl<'document> SourceText<'document> {
    pub(crate) fn cryxml(element: Element<'document>) -> Option<Self> {
        Some(Self {
            value: element.content()?,
            lexeme: None,
            position: SourceContentPosition::CryXml(element.index()),
        })
    }

    fn text(
        value: &'document str,
        source: &'document str,
        range: Range<usize>,
        prefix_bytes: usize,
    ) -> Result<Self, SourceSpanError> {
        let lexeme = source.get(range.clone()).ok_or(SourceSpanError)?;
        Ok(Self {
            value,
            lexeme: Some(lexeme),
            position: SourceContentPosition::Text(source_position::original_range(
                range,
                prefix_bytes,
            )),
        })
    }

    /// Returns the decoded or stored text value.
    #[must_use]
    pub const fn value(&self) -> &'document str {
        self.value
    }

    /// Returns the exact original lexical text or comment in text XML.
    #[must_use]
    pub const fn lexeme(&self) -> Option<&'document str> {
        self.lexeme
    }

    /// Returns the original text extent or CryXml content slot.
    #[must_use]
    pub const fn position(&self) -> &SourceContentPosition {
        &self.position
    }
}

/// A borrowed iterator over original direct content occurrences.
pub struct SourceContents<'document> {
    view: ContentView<'document>,
}

enum ContentView<'document> {
    Text(TextContents<'document>),
    CryXml(Children<'document>),
    Root(Option<SourceElement<'document>>),
}

impl<'document> SourceContents<'document> {
    pub(crate) fn text_element(
        node: TextNode<'document, 'document>,
        qualified_name: &str,
        prefix_bytes: usize,
    ) -> Result<Self, SourceSpanError> {
        let source = node.document().input_text();
        let range = node.range();
        let original = source.get(range.clone()).ok_or(SourceSpanError)?;
        let closing = closing_tag_start(original, qualified_name)? + range.start;
        Ok(Self::text(node, closing, prefix_bytes))
    }

    pub(crate) fn text_document(node: TextNode<'document, 'document>, prefix_bytes: usize) -> Self {
        Self::text(node, node.document().input_text().len(), prefix_bytes)
    }

    fn text(node: TextNode<'document, 'document>, closing: usize, prefix_bytes: usize) -> Self {
        Self {
            view: ContentView::Text(TextContents {
                children: node.children().peekable(),
                source: node.document().input_text(),
                closing,
                prefix_bytes,
            }),
        }
    }

    pub(crate) fn cryxml_element(element: Element<'document>) -> Self {
        Self {
            view: ContentView::CryXml(element.children()),
        }
    }

    pub(crate) const fn cryxml_document(element: Element<'document>) -> Self {
        Self {
            view: ContentView::Root(Some(SourceElement::cryxml(element))),
        }
    }
}

struct TextContents<'document> {
    children: Peekable<TextChildren<'document, 'document>>,
    source: &'document str,
    closing: usize,
    prefix_bytes: usize,
}

impl<'document> TextContents<'document> {
    fn next_content(&mut self) -> Option<Result<SourceContent<'document>, SourceSpanError>> {
        let node = self.children.next()?;
        Some(match node.node_type() {
            NodeType::Element => Ok(SourceContent::Element(SourceElement::text(
                node,
                self.prefix_bytes,
            ))),
            NodeType::Text => {
                let end = self
                    .children
                    .peek()
                    .map_or(self.closing, |next| next.range().start);
                self.text(node, node.range().start..end)
                    .map(SourceContent::Text)
            }
            NodeType::Comment => self.text(node, node.range()).map(SourceContent::Comment),
            NodeType::PI => node
                .pi()
                .map(|instruction| SourceContent::ProcessingInstruction {
                    target: instruction.target,
                    value: instruction.value,
                    position: source_position::original_range(node.range(), self.prefix_bytes),
                })
                .ok_or(SourceSpanError),
            NodeType::Root => Err(SourceSpanError),
        })
    }

    fn text(
        &self,
        node: TextNode<'document, 'document>,
        range: Range<usize>,
    ) -> Result<SourceText<'document>, SourceSpanError> {
        SourceText::text(
            node.text().ok_or(SourceSpanError)?,
            self.source,
            range,
            self.prefix_bytes,
        )
    }
}

impl<'document> Iterator for SourceContents<'document> {
    type Item = Result<SourceContent<'document>, SourceSpanError>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.view {
            ContentView::Text(children) => children.next_content(),
            ContentView::CryXml(children) => children
                .next()
                .map(|element| Ok(SourceContent::Element(SourceElement::cryxml(element)))),
            ContentView::Root(root) => root
                .take()
                .map(|element| Ok(SourceContent::Element(element))),
        }
    }
}

fn closing_tag_start(source: &str, qualified_name: &str) -> Result<usize, SourceSpanError> {
    if source.ends_with("/>") {
        return Ok(source.len());
    }
    let end = source.strip_suffix('>').ok_or(SourceSpanError)?;
    let end = end.trim_end_matches(|character: char| character.is_ascii_whitespace());
    let before_name = end.strip_suffix(qualified_name).ok_or(SourceSpanError)?;
    let before_close = before_name.strip_suffix("</").ok_or(SourceSpanError)?;
    Ok(before_close.len())
}
