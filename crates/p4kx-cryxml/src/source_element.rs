// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use roxmltree::{Children as TextChildren, Node as TextNode};

use crate::element::{Children, Element};
use crate::source_attribute::{SourceAttribute, SourceAttributes};
use crate::source_position::{self, SourceElementPosition};
use crate::source_syntax::{SourceSpanError, SourceSyntax};

/// A borrowed element of a parsed XML source.
#[derive(Debug, Clone, Copy)]
pub struct SourceElement<'document> {
    view: ElementView<'document>,
}

#[derive(Debug, Clone, Copy)]
enum ElementView<'document> {
    Text {
        node: TextNode<'document, 'document>,
        prefix_bytes: usize,
    },
    CryXml(Element<'document>),
}

impl<'document> SourceElement<'document> {
    pub(crate) const fn text(node: TextNode<'document, 'document>, prefix_bytes: usize) -> Self {
        Self {
            view: ElementView::Text { node, prefix_bytes },
        }
    }

    pub(crate) const fn cryxml(element: Element<'document>) -> Self {
        Self {
            view: ElementView::CryXml(element),
        }
    }

    /// Returns the text element's local name or the stored CryXml name.
    #[must_use]
    pub fn name(self) -> &'document str {
        match self.view {
            ElementView::Text { node, .. } => node.tag_name().name(),
            ElementView::CryXml(element) => element.name(),
        }
    }

    /// Returns the text element's expanded namespace URI when present.
    #[must_use]
    pub fn namespace(self) -> Option<&'document str> {
        match self.view {
            ElementView::Text { node, .. } => node.tag_name().namespace(),
            ElementView::CryXml(_) => None,
        }
    }

    /// Returns the element's original coordinate within this input.
    #[must_use]
    pub fn position(self) -> SourceElementPosition {
        match self.view {
            ElementView::Text { node, prefix_bytes } => SourceElementPosition::Text(
                source_position::original_range(node.range(), prefix_bytes),
            ),
            ElementView::CryXml(element) => SourceElementPosition::CryXml(element.index()),
        }
    }

    /// Reads the authored syntax of this element.
    ///
    /// # Errors
    /// Returns an error if the original text opener has no valid source span.
    pub fn syntax(self) -> Result<SourceSyntax<'document>, SourceSpanError> {
        match self.view {
            ElementView::Text { node, prefix_bytes } => SourceSyntax::text(node, prefix_bytes),
            ElementView::CryXml(element) => Ok(SourceSyntax::from_cryxml_element(element)),
        }
    }

    /// Returns the first matching local text name or stored CryXml attribute name.
    #[must_use]
    pub fn attribute(self, name: &str) -> Option<&'document str> {
        match self.view {
            ElementView::Text { node, .. } => node.attribute(name),
            ElementView::CryXml(element) => element.attribute(name),
        }
    }

    /// Returns original attributes in order, with text namespace declarations in the element span.
    pub fn attributes(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        match self.view {
            ElementView::Text { node, prefix_bytes } => SourceAttributes::Text {
                attributes: node.attributes(),
                prefix_bytes,
            },
            ElementView::CryXml(element) => SourceAttributes::CryXml {
                attributes: element.attributes(),
                index: element.first_attribute_index(),
                node_index: element.index(),
            },
        }
    }

    /// Returns direct child elements in the order supplied by the source parser.
    pub fn elements(self) -> impl Iterator<Item = Self> {
        match self.view {
            ElementView::Text { node, prefix_bytes } => SourceElements::Text {
                children: node.children(),
                prefix_bytes,
            },
            ElementView::CryXml(element) => SourceElements::CryXml(element.children()),
        }
    }
}

enum SourceElements<'document> {
    Text {
        children: TextChildren<'document, 'document>,
        prefix_bytes: usize,
    },
    CryXml(Children<'document>),
}

impl<'document> Iterator for SourceElements<'document> {
    type Item = SourceElement<'document>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Text {
                children,
                prefix_bytes,
            } => children
                .find(TextNode::is_element)
                .map(|node| SourceElement::text(node, *prefix_bytes)),
            Self::CryXml(children) => children.next().map(SourceElement::cryxml),
        }
    }
}
