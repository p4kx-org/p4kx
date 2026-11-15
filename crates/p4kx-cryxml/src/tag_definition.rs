// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original Mannequin tag and fragment definition declarations.

use crate::SourceElement;

/// A physical Mannequin tag or fragment definition document.
#[derive(Debug, Clone, Copy)]
pub struct TagDefinition<'document>(SourceElement<'document>);

impl<'document> TagDefinition<'document> {
    /// Admits an unqualified original definition root.
    #[must_use]
    pub fn from_root(root: SourceElement<'document>) -> Option<Self> {
        (root.namespace().is_none() && root.name() == "TagDefinition").then_some(Self(root))
    }

    /// Returns the original import and tag sections in authored order.
    pub fn sections(self) -> impl Iterator<Item = Section<'document>> {
        self.0
            .elements()
            .filter_map(|element| match (element.namespace(), element.name()) {
                (None, "Imports") => Some(Section::Imports(Imports(element))),
                (None, "Tags") => Some(Section::Tags(Tags(element))),
                _ => None,
            })
    }
}

/// One original recognized section of a tag or fragment definition.
#[derive(Debug, Clone, Copy)]
pub enum Section<'document> {
    /// Imports of other definition documents.
    Imports(Imports<'document>),
    /// Direct tags and named groups of tags.
    Tags(Tags<'document>),
}

/// Original definition import section.
#[derive(Debug, Clone, Copy)]
pub struct Imports<'document>(SourceElement<'document>);

impl<'document> Imports<'document> {
    /// Returns the original ordered import elements.
    pub fn elements(self) -> impl Iterator<Item = SourceElement<'document>> {
        self.0
            .elements()
            .filter(|node| node.namespace().is_none() && node.name() == "Import")
    }
}

/// Original tag section.
#[derive(Debug, Clone, Copy)]
pub struct Tags<'document>(SourceElement<'document>);

impl<'document> Tags<'document> {
    /// Returns the source element containing the direct members.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.0
    }

    /// Returns recognized direct members in authored order.
    pub fn members(self) -> impl Iterator<Item = Member<'document>> {
        self.0
            .elements()
            .filter_map(|element| match (element.namespace(), element.name()) {
                (None, "Tag") => Some(Member::Tag(element)),
                (None, "Group") => Some(Member::Group(Group(element))),
                _ => None,
            })
    }
}

/// One original tag or one ordered group of tags.
#[derive(Debug, Clone, Copy)]
pub enum Member<'document> {
    /// Unqualified direct Tag declaration.
    Tag(SourceElement<'document>),
    /// Group element with direct tag children.
    Group(Group<'document>),
}

/// One original group of tags.
#[derive(Debug, Clone, Copy)]
pub struct Group<'document>(SourceElement<'document>);

impl<'document> Group<'document> {
    /// Returns the original group element.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.0
    }

    /// Returns the group's direct unqualified Tag children.
    pub fn tags(self) -> impl Iterator<Item = SourceElement<'document>> {
        self.0
            .elements()
            .filter(|node| node.namespace().is_none() && node.name() == "Tag")
    }
}

#[cfg(test)]
#[path = "tag_definition_tests.rs"]
mod tests;
