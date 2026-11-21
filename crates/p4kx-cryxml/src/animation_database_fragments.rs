// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::source_attribute::SourceAttribute;
use crate::source_element::SourceElement;

use super::{attributes, children};

/// One fragment alternative in its original list and group.
#[derive(Debug, Clone, Copy)]
pub struct FragmentOption<'document> {
    list_ordinal: usize,
    group_ordinal: usize,
    option_ordinal: usize,
    group: SourceElement<'document>,
    element: SourceElement<'document>,
}

impl<'document> FragmentOption<'document> {
    /// Returns the list, group and option positions among their original siblings.
    #[must_use]
    pub const fn ordinals(self) -> (usize, usize, usize) {
        (self.list_ordinal, self.group_ordinal, self.option_ordinal)
    }

    /// Returns the original group element naming the fragment.
    #[must_use]
    pub const fn group(self) -> SourceElement<'document> {
        self.group
    }

    /// Returns the original option element.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.element
    }

    /// Returns the option's authored `BlendOutDuration` occurrences.
    pub fn blend_out_durations(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        attributes(self.element, "BlendOutDuration")
    }

    /// Returns direct animation layers with their original sibling ordinals.
    pub fn layers(self) -> impl Iterator<Item = AnimationLayer<'document>> {
        children(self.element, "AnimLayer")
            .map(|(ordinal, element)| AnimationLayer { ordinal, element })
    }
}

/// A group naming a fragment and declaring its ordered alternatives.
#[derive(Debug, Clone, Copy)]
pub struct FragmentGroup<'document> {
    list_ordinal: usize,
    group_ordinal: usize,
    element: SourceElement<'document>,
}

impl<'document> FragmentGroup<'document> {
    /// Returns the original group element.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.element
    }

    /// Returns direct alternatives in original sibling order.
    pub fn options(self) -> impl Iterator<Item = FragmentOption<'document>> {
        children(self.element, "Fragment").map(move |(option_ordinal, element)| FragmentOption {
            list_ordinal: self.list_ordinal,
            group_ordinal: self.group_ordinal,
            option_ordinal,
            group: self.element,
            element,
        })
    }
}

pub(super) fn groups(root: SourceElement<'_>) -> impl Iterator<Item = FragmentGroup<'_>> {
    children(root, "FragmentList").flat_map(|(list_ordinal, list)| {
        list.elements()
            .enumerate()
            .filter(|(_, element)| element.namespace().is_none())
            .map(move |(group_ordinal, element)| FragmentGroup {
                list_ordinal,
                group_ordinal,
                element,
            })
    })
}

/// A direct authored animation layer.
#[derive(Debug, Clone, Copy)]
pub struct AnimationLayer<'document> {
    ordinal: usize,
    element: SourceElement<'document>,
}

impl<'document> AnimationLayer<'document> {
    /// Returns this layer's position among the option's original child elements.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }

    /// Returns the original layer element.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.element
    }

    /// Returns the original layer members in interleaved order.
    pub fn items(self) -> impl Iterator<Item = (usize, LayerItem<'document>)> {
        self.element
            .elements()
            .enumerate()
            .map(|(ordinal, element)| {
                let item = match (element.namespace(), element.name()) {
                    (None, "Animation") => LayerItem::Animation(Animation(element)),
                    (None, "Blend") => LayerItem::Blend(Blend(element)),
                    _ => LayerItem::Other(element),
                };
                (ordinal, item)
            })
    }
}

/// A direct member of an authored animation layer.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum LayerItem<'document> {
    /// A clip-alias declaration.
    Animation(Animation<'document>),
    /// A timing declaration in its original position.
    Blend(Blend<'document>),
    /// Another original element preserved at its source position.
    Other(SourceElement<'document>),
}

/// An original clip-alias declaration with per-occurrence attributes.
#[derive(Debug, Clone, Copy)]
pub struct Animation<'document>(SourceElement<'document>);

impl<'document> Animation<'document> {
    /// Returns the original animation element.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.0
    }

    /// Returns the authored `speed` occurrences.
    pub fn speeds(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        attributes(self.0, "speed")
    }
}

/// An original blend declaration with per-occurrence attributes.
#[derive(Debug, Clone, Copy)]
pub struct Blend<'document>(SourceElement<'document>);

impl<'document> Blend<'document> {
    /// Returns the original blend element.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.0
    }

    /// Returns authored `Duration` occurrences.
    pub fn durations(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        attributes(self.0, "Duration")
    }

    /// Returns authored `StartTime` occurrences.
    pub fn start_times(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        attributes(self.0, "StartTime")
    }

    /// Returns authored `ExitTime` occurrences.
    pub fn exit_times(self) -> impl Iterator<Item = SourceAttribute<'document>> {
        attributes(self.0, "ExitTime")
    }
}
