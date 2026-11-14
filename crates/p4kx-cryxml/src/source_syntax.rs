// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use roxmltree::Node as TextNode;
use thiserror::Error;

use crate::element::Element;
use crate::source_attribute::SourceAttributes;
use crate::source_content::{SourceContents, SourceText};
use crate::source_declaration::{SourceDeclaration, SourceDeclarations};
use crate::source_start_tag::StartTag;

/// The original spelling cannot be matched to a parsed XML coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid original XML source span")]
pub struct SourceSpanError;

/// The authored syntax of a validated XML element.
#[derive(Debug, Clone)]
pub struct SourceSyntax<'document> {
    view: SyntaxView<'document>,
}

#[derive(Debug, Clone)]
enum SyntaxView<'document> {
    Text {
        node: TextNode<'document, 'document>,
        tag: StartTag<'document>,
        prefix_bytes: usize,
    },
    CryXml(Element<'document>),
}

impl<'document> SourceSyntax<'document> {
    /// Reads the syntax of an element in a parsed text XML document.
    ///
    /// # Errors
    /// Returns an error if the input is not an element or its opener has no valid source span.
    pub fn from_text_element(
        node: TextNode<'document, 'document>,
    ) -> Result<Self, SourceSpanError> {
        if !node.is_element() {
            return Err(SourceSpanError);
        }
        Self::text(node, 0)
    }

    pub(crate) fn text(
        node: TextNode<'document, 'document>,
        prefix_bytes: usize,
    ) -> Result<Self, SourceSpanError> {
        let tag = StartTag::new(node.document().input_text(), node.range(), prefix_bytes)?;
        Ok(Self {
            view: SyntaxView::Text {
                node,
                tag,
                prefix_bytes,
            },
        })
    }

    /// Borrows the syntax of a checked CryXml element.
    #[must_use]
    pub const fn from_cryxml_element(element: Element<'document>) -> Self {
        Self {
            view: SyntaxView::CryXml(element),
        }
    }

    /// Returns the authored `QName` or stored CryXml name.
    #[must_use]
    pub fn qualified_name(&self) -> &'document str {
        match &self.view {
            SyntaxView::Text { tag, .. } => tag.name,
            SyntaxView::CryXml(element) => element.name(),
        }
    }

    /// Returns lexical attributes and namespace declarations in their authored order.
    pub fn declarations(
        &self,
    ) -> impl Iterator<Item = Result<SourceDeclaration<'document>, SourceSpanError>> {
        match &self.view {
            SyntaxView::Text { node, tag, .. } => SourceDeclarations::Text {
                node: *node,
                tokens: tag.attributes.clone(),
                attributes: node.attributes(),
            },
            SyntaxView::CryXml(element) => SourceDeclarations::CryXml(SourceAttributes::CryXml {
                attributes: element.attributes(),
                index: element.first_attribute_index(),
                node_index: element.index(),
            }),
        }
    }

    /// Returns direct content in the parser's original order.
    ///
    /// # Errors
    /// Returns an error if the original text closing tag has no valid source span.
    pub fn content(&self) -> Result<SourceContents<'document>, SourceSpanError> {
        match &self.view {
            SyntaxView::Text {
                node,
                tag,
                prefix_bytes,
            } => SourceContents::text_element(*node, tag.name, *prefix_bytes),
            SyntaxView::CryXml(element) => Ok(SourceContents::cryxml_element(*element)),
        }
    }

    /// Returns the CryXml node's independent content slot when present.
    #[must_use]
    pub fn binary_text(&self) -> Option<SourceText<'document>> {
        match self.view {
            SyntaxView::Text { .. } => None,
            SyntaxView::CryXml(element) => SourceText::cryxml(element),
        }
    }
}
