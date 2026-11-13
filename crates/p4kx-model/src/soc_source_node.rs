// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttribute, SourceElement};

#[derive(Debug, Clone, Copy)]
pub(crate) struct SocSourceNode<'document> {
    ordinal: usize,
    element: SourceElement<'document>,
}

impl<'document> SocSourceNode<'document> {
    pub(crate) const fn new(ordinal: usize, element: SourceElement<'document>) -> Self {
        Self { ordinal, element }
    }

    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub(crate) const fn element(self) -> SourceElement<'document> {
        self.element
    }

    pub(crate) fn children(
        parent: SourceElement<'document>,
        name: &'static str,
    ) -> impl Iterator<Item = Self> {
        Self::direct_children(parent).filter(move |node| named(node.element, name))
    }

    pub(crate) fn direct_children(parent: SourceElement<'document>) -> impl Iterator<Item = Self> {
        parent
            .elements()
            .enumerate()
            .map(|(ordinal, element)| Self::new(ordinal, element))
    }

    pub(crate) fn attributes(
        self,
        name: &'static str,
    ) -> impl Iterator<Item = SourceAttribute<'document>> {
        self.attributes_where(move |attribute| attribute == name)
    }

    pub(crate) fn attributes_where(
        self,
        matches: impl Fn(&str) -> bool,
    ) -> impl Iterator<Item = SourceAttribute<'document>> {
        self.element
            .attributes()
            .filter(move |attribute| attribute.namespace().is_none() && matches(attribute.name()))
    }

    pub(crate) fn read<T>(
        self,
        name: &str,
        parse: impl FnOnce(&'document str) -> Option<T>,
    ) -> SocSourceValue<T> {
        self.attribute(name).read(parse)
    }

    pub(crate) fn attribute(self, name: &str) -> Attribute<'document> {
        self.attribute_where(|attribute| attribute == name)
    }

    pub(crate) fn attribute_where(self, matches: impl Fn(&str) -> bool) -> Attribute<'document> {
        let mut matches = self.attributes_where(matches);
        match (matches.next(), matches.next()) {
            (None, _) => Attribute::Absent,
            (Some(attribute), None) => Attribute::Value(attribute.value()),
            _ => Attribute::Repeated,
        }
    }
}

pub(crate) fn named(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name() == name
}

pub(crate) enum Attribute<'document> {
    Absent,
    Repeated,
    Value(&'document str),
}

impl<'document> Attribute<'document> {
    pub(crate) fn read<T>(
        self,
        parse: impl FnOnce(&'document str) -> Option<T>,
    ) -> SocSourceValue<T> {
        match self {
            Self::Absent => SocSourceValue::Absent,
            Self::Repeated => SocSourceValue::Invalid,
            Self::Value(text) => {
                parse(text).map_or(SocSourceValue::Invalid, SocSourceValue::Present)
            }
        }
    }
}

/// Availability of one typed value whose raw XML remains accessible.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum SocSourceValue<T> {
    /// The attribute is absent.
    Absent,
    /// The attribute is repeated or cannot be interpreted.
    Invalid,
    /// The authored value was interpreted without changing its order.
    Present(T),
}
