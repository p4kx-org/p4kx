// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Authored Mannequin animation database declarations.

#[path = "animation_database_fragments.rs"]
mod fragments;

use crate::source_attribute::SourceAttribute;
use crate::source_element::SourceElement;

pub use fragments::{Animation, AnimationLayer, Blend, FragmentGroup, FragmentOption, LayerItem};

/// A borrowed view of a physical `AnimDB` document.
#[derive(Debug, Clone, Copy)]
pub struct AnimationDatabase<'document> {
    root: SourceElement<'document>,
}

impl<'document> AnimationDatabase<'document> {
    /// Opens an original, unnamespaced `AnimDB` root.
    #[must_use]
    pub fn from_root(root: SourceElement<'document>) -> Option<Self> {
        (root.namespace().is_none() && root.name() == "AnimDB").then_some(Self { root })
    }

    /// Returns the original database root element.
    #[must_use]
    pub const fn root(self) -> SourceElement<'document> {
        self.root
    }

    /// Returns the original fragment-definition attributes in source order.
    pub fn fragment_definition(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        attributes(self.root, "FragDef")
    }

    /// Returns the original global tag-definition attributes in source order.
    pub fn tag_definition(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        attributes(self.root, "TagDef")
    }

    /// Returns direct `SubADBs` sections in original sibling order.
    pub fn sections(self) -> impl Iterator<Item = SubDatabaseSection<'document>> {
        children(self.root, "SubADBs")
            .map(|(ordinal, element)| SubDatabaseSection { ordinal, element })
    }

    /// Returns direct sub-database declarations in original order.
    pub fn branches(self) -> impl Iterator<Item = SubDatabase<'document>> {
        self.sections().flat_map(SubDatabaseSection::branches)
    }

    /// Returns the original fragment groups in list and sibling order.
    pub fn groups(self) -> impl Iterator<Item = FragmentGroup<'document>> {
        fragments::groups(self.root)
    }

    /// Returns every direct authored fragment option in original list and group order.
    pub fn options(self) -> impl Iterator<Item = FragmentOption<'document>> {
        self.groups().flat_map(FragmentGroup::options)
    }

    /// Returns the original options whose unqualified group name matches the selector.
    pub fn options_named<'name>(
        self,
        name: &'name str,
    ) -> impl Iterator<Item = FragmentOption<'document>> + use<'document, 'name> {
        self.groups()
            .filter(move |group| group.element().name().eq_ignore_ascii_case(name))
            .flat_map(FragmentGroup::options)
    }
}

/// An original section grouping sub-database declarations.
#[derive(Debug, Clone, Copy)]
pub struct SubDatabaseSection<'document> {
    ordinal: usize,
    element: SourceElement<'document>,
}

impl<'document> SubDatabaseSection<'document> {
    /// Returns the original grouping element.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.element
    }

    /// Returns original branch declarations in their sibling order.
    pub fn branches(self) -> impl Iterator<Item = SubDatabase<'document>> {
        children(self.element, "SubADB").map(move |(ordinal, element)| SubDatabase {
            section_ordinal: self.ordinal,
            ordinal,
            element,
        })
    }
}

/// One original `SubADB` declaration with its owning section and sibling positions.
#[derive(Debug, Clone, Copy)]
pub struct SubDatabase<'document> {
    section_ordinal: usize,
    ordinal: usize,
    element: SourceElement<'document>,
}

impl<'document> SubDatabase<'document> {
    /// Returns the owning section and branch positions among their direct siblings.
    #[must_use]
    pub const fn ordinals(self) -> (usize, usize) {
        (self.section_ordinal, self.ordinal)
    }

    /// Returns the original branch element.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.element
    }

    /// Returns every authored `File` occurrence.
    pub fn files(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        attributes(self.element, "File")
    }

    /// Returns every authored branch `Tags` occurrence.
    pub fn tags(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        attributes(self.element, "Tags")
    }

    /// Returns ordered direct fragment selectors with their original sibling ordinals.
    pub fn fragments(self) -> impl Iterator<Item = (usize, SourceElement<'document>)> {
        children(self.element, "FragmentID")
    }
}

/// Returns matching unqualified attributes in original order.
pub fn attributes<'document, 'name>(
    node: SourceElement<'document>,
    name: &'name str,
) -> impl Iterator<Item = SourceAttribute<'document>> + use<'document, 'name> {
    node.attributes()
        .filter(move |attribute| attribute.namespace().is_none() && attribute.name() == name)
}

fn children<'document>(
    parent: SourceElement<'document>,
    name: &'static str,
) -> impl Iterator<Item = (usize, SourceElement<'document>)> {
    parent
        .elements()
        .enumerate()
        .filter(move |(_, node)| node.namespace().is_none() && node.name() == name)
}

#[cfg(test)]
#[path = "animation_database_tests.rs"]
mod tests;
