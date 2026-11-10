// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::iter::FusedIterator;
use std::slice;

use crate::document::{Document, Node};
use crate::strings::Span;

/// One attribute of an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attribute<'d> {
    name: &'d str,
    value: &'d str,
}

impl<'d> Attribute<'d> {
    /// Returns the attribute's name as stored in the file.
    #[must_use]
    pub const fn name(self) -> &'d str {
        self.name
    }

    /// Returns the attribute's value.
    #[must_use]
    pub const fn value(self) -> &'d str {
        self.value
    }
}

/// One element of a [`Document`].
#[derive(Debug, Clone, Copy)]
pub struct Element<'d> {
    document: &'d Document<'d>,
    index: u32,
}

impl<'d> Element<'d> {
    pub(crate) const fn new(document: &'d Document<'d>, index: u32) -> Self {
        Self { document, index }
    }

    /// Returns the original node-table position in this element's owning document.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Returns whether this element borrows the given actual parsed document.
    #[must_use]
    pub fn is_from(self, document: &Document<'_>) -> bool {
        std::ptr::eq(self.document, document)
    }

    fn node(self) -> Option<&'d Node> {
        self.document.nodes.get(self.index as usize)
    }

    /// Returns the element's name as stored in the file.
    #[must_use]
    pub fn name(self) -> &'d str {
        self.node()
            .map_or("", |node| node.name.get(&self.document.text))
    }

    /// Returns the element's text content, or `None` when it has none.
    #[must_use]
    pub fn content(self) -> Option<&'d str> {
        self.node()
            .filter(|node| !node.content.is_empty())
            .map(|node| node.content.get(&self.document.text))
    }

    /// Returns the element's attributes in file order.
    #[must_use]
    pub fn attributes(self) -> Attributes<'d> {
        let spans = self
            .node()
            .and_then(|node| {
                let first = node.first_attribute as usize;
                self.document
                    .attributes
                    .get(first..first + usize::from(node.attribute_count))
            })
            .unwrap_or_default();
        Attributes {
            text: &self.document.text,
            spans: spans.iter(),
        }
    }

    /// Returns the value of the first attribute named `name`.
    #[must_use]
    pub fn attribute(self, name: &str) -> Option<&'d str> {
        self.attributes()
            .find(|attribute| attribute.name() == name)
            .map(Attribute::value)
    }

    /// Returns the element's children in file order.
    #[must_use]
    pub fn children(self) -> Children<'d> {
        let index = self.index as usize;
        let starts = &self.document.child_starts;
        let ids = starts
            .get(index)
            .zip(starts.get(index + 1))
            .and_then(|(&start, &end)| self.document.children.get(start as usize..end as usize))
            .unwrap_or_default();
        Children {
            document: self.document,
            ids: ids.iter(),
        }
    }

    /// Returns whether the element has children.
    #[must_use]
    pub fn has_children(self) -> bool {
        self.children().len() > 0
    }
}

/// The attributes of an [`Element`].
#[derive(Debug, Clone)]
pub struct Attributes<'d> {
    text: &'d str,
    spans: slice::Iter<'d, (Span, Span)>,
}

impl<'d> Iterator for Attributes<'d> {
    type Item = Attribute<'d>;

    fn next(&mut self) -> Option<Self::Item> {
        self.spans.next().map(|&(name, value)| Attribute {
            name: name.get(self.text),
            value: value.get(self.text),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.spans.size_hint()
    }
}

impl ExactSizeIterator for Attributes<'_> {}

impl FusedIterator for Attributes<'_> {}

/// The children of an [`Element`].
#[derive(Debug, Clone)]
pub struct Children<'d> {
    document: &'d Document<'d>,
    ids: slice::Iter<'d, u32>,
}

impl<'d> Iterator for Children<'d> {
    type Item = Element<'d>;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids
            .next()
            .map(|&index| Element::new(self.document, index))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.ids.size_hint()
    }
}

impl DoubleEndedIterator for Children<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.ids
            .next_back()
            .map(|&index| Element::new(self.document, index))
    }
}

impl ExactSizeIterator for Children<'_> {}

impl FusedIterator for Children<'_> {}
