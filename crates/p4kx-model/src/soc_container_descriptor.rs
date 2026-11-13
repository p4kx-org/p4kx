// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttribute, SourceDocument, SourceElement, SourceElementPosition};
use p4kx_cryxml::{SourceReadError, SourceReadOptions};
use thiserror::Error;

use crate::soc_source_node::{SocSourceNode, SocSourceValue};

/// A positioned source descriptor of an object container.
#[derive(Debug)]
pub struct SocContainerDescriptor<'input> {
    document: SourceDocument<'input>,
}

/// Failure to read an original object container descriptor.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SocContainerDescriptorError {
    /// The source XML cannot be read.
    #[error(transparent)]
    Xml(#[from] SourceReadError),
    /// The root is not an object container.
    #[error("the XML source has no ObjectContainer root")]
    Root,
}

impl<'input> SocContainerDescriptor<'input> {
    /// Reads the original descriptor while retaining its physical XML positions.
    ///
    /// # Errors
    /// Returns an error for unreadable XML or an unexpected root.
    pub fn parse(bytes: &'input [u8]) -> Result<Self, SocContainerDescriptorError> {
        let document = SourceDocument::parse(bytes, SourceReadOptions::default())?;
        if !named(document.root(), "ObjectContainer") {
            return Err(SocContainerDescriptorError::Root);
        }
        Ok(Self { document })
    }

    /// Returns the original descriptor XML.
    #[must_use]
    pub const fn document(&self) -> &SourceDocument<'_> {
        &self.document
    }

    /// Visits physical Child declarations in descriptor order.
    pub fn children(&self) -> impl Iterator<Item = SocContainerDeclaration<'_>> {
        sections(self.document.root(), "ChildObjectContainers").flat_map(|section| {
            sections(section.element(), "Child")
                .map(move |node| SocContainerDeclaration { section, node })
        })
    }

    /// Visits physical stream declarations in descriptor order, parents before nested streams.
    pub fn streams(&self) -> impl Iterator<Item = SocStreamDeclaration<'_>> {
        let mut pending = Vec::new();
        push_streams(self.document.root(), None, &mut pending);
        std::iter::from_fn(move || {
            let stream = pending.pop()?;
            push_streams(
                stream.declaration.element(),
                Some(stream.declaration.element()),
                &mut pending,
            );
            Some(stream)
        })
    }
}

fn push_streams<'document>(
    owner: SourceElement<'document>,
    parent: Option<SourceElement<'document>>,
    pending: &mut Vec<SocStreamDeclaration<'document>>,
) {
    let start = pending.len();
    for section in sections(owner, "StreamingOCs") {
        pending.extend(
            sections(section.element(), "OC").map(|node| SocStreamDeclaration {
                declaration: SocContainerDeclaration { section, node },
                parent,
            }),
        );
    }
    if let Some(new) = pending.get_mut(start..) {
        new.reverse();
    }
}

fn sections<'document>(
    owner: SourceElement<'document>,
    name: &'static str,
) -> impl Iterator<Item = SocSourceNode<'document>> {
    SocSourceNode::direct_children(owner).filter(move |node| named(node.element(), name))
}

fn named(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name().eq_ignore_ascii_case(name)
}

/// One physical Child or stream declaration with its source coordinates.
#[derive(Debug, Clone, Copy)]
pub struct SocContainerDeclaration<'document> {
    section: SocSourceNode<'document>,
    node: SocSourceNode<'document>,
}

impl<'document> SocContainerDeclaration<'document> {
    /// Returns the original section slot and the declaration slot within that section.
    #[must_use]
    pub const fn ordinal(self) -> (usize, usize) {
        (self.section.ordinal(), self.node.ordinal())
    }

    /// Returns the original declaration element and its authored content.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Returns the original section element and its authored content.
    #[must_use]
    pub const fn section(self) -> SourceElement<'document> {
        self.section.element()
    }

    /// Returns original `name` attributes using the descriptor's case-insensitive spelling.
    pub fn name_attributes(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        self.node
            .attributes_where(|attribute| attribute.eq_ignore_ascii_case("name"))
    }

    /// Reads the unique authored name, including an explicit empty string.
    #[must_use]
    pub fn name(self) -> SocSourceValue<&'document str> {
        self.node
            .attribute_where(|attribute| attribute.eq_ignore_ascii_case("name"))
            .read(Some)
    }
}

/// One physical stream declaration and its optional original enclosing stream.
#[derive(Debug, Clone, Copy)]
pub struct SocStreamDeclaration<'document> {
    declaration: SocContainerDeclaration<'document>,
    parent: Option<SourceElement<'document>>,
}

impl<'document> SocStreamDeclaration<'document> {
    /// Returns the positioned stream declaration.
    #[must_use]
    pub const fn declaration(self) -> SocContainerDeclaration<'document> {
        self.declaration
    }

    /// Returns the original enclosing stream's XML coordinate when nested.
    #[must_use]
    pub fn parent_position(self) -> Option<SourceElementPosition> {
        self.parent.map(SourceElement::position)
    }
}
