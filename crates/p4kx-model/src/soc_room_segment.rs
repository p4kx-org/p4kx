// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::SourceElement;

use crate::soc_entities;
use crate::soc_source_node::{Attribute, SocSourceNode, SocSourceValue};

/// One original positioned segment in a SOC `AreaComponent`.
#[derive(Debug, Clone, Copy)]
pub struct SocRoomSegment<'document> {
    node: SocSourceNode<'document>,
}

impl<'document> SocRoomSegment<'document> {
    pub(crate) const fn new(node: SocSourceNode<'document>) -> Self {
        Self { node }
    }

    /// Returns the zero-based slot among the component's direct elements.
    #[must_use]
    pub const fn ordinal(self) -> usize {
        self.node.ordinal()
    }

    /// Returns the original segment including all attributes and uninterpreted children.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Interprets the authored local quaternion and translation without deriving a matrix.
    #[must_use]
    pub fn local_transform(self) -> SocSourceValue<SocSegmentTransform> {
        self.node.read("transform_ls", parse_transform)
    }

    /// Classifies the original segment declaration and retains absent or invalid values.
    #[must_use]
    pub fn geometry(self) -> SocSegmentGeometry<'document> {
        match self.node.attribute("Type") {
            Attribute::Value("Box") => SocSegmentGeometry::Box {
                min: self.node.read("BoxMin", vector),
                max: self.node.read("BoxMax", vector),
            },
            Attribute::Value("Shape") => SocSegmentGeometry::Outline {
                height: self.node.read("Height", finite),
            },
            Attribute::Value("Trimesh") => SocSegmentGeometry::TriangleSoup,
            label => SocSegmentGeometry::Other {
                label: label.read(Some),
            },
        }
    }

    /// Visits original outline points including malformed points and repeated point groups.
    pub fn outline_points(self) -> impl Iterator<Item = SocSegmentPoint<'document>> {
        SocSourceNode::children(self.element(), "Points").flat_map(|group| {
            SocSourceNode::children(group.element(), "Point").map(move |node| SocSegmentPoint {
                group_ordinal: Some(group.ordinal()),
                node,
                position_name: "Pos",
            })
        })
    }

    /// Visits the original direct point slots in triangle-soup storage order.
    pub fn triangle_points(self) -> impl Iterator<Item = SocSegmentPoint<'document>> {
        SocSourceNode::children(self.element(), "Point").map(|node| SocSegmentPoint {
            group_ordinal: None,
            node,
            position_name: "pos",
        })
    }
}

/// The position-bearing data of one segment without inferred containment rules.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum SocSegmentGeometry<'document> {
    /// Original box bounds in segment-local coordinates.
    Box {
        /// Authored `BoxMin`, without swapping bounds.
        min: SocSourceValue<[f64; 3]>,
        /// Authored `BoxMax`, without swapping bounds.
        max: SocSourceValue<[f64; 3]>,
    },
    /// An original outline with a separately authored extrusion height.
    Outline {
        /// Authored Height in source units.
        height: SocSourceValue<f64>,
    },
    /// A triangle soup stored as consecutive point slots.
    TriangleSoup,
    /// Another or unavailable shape discriminator whose XML remains accessible.
    Other {
        /// Original shape name when exactly one was declared.
        label: SocSourceValue<&'document str>,
    },
}

/// The original `transform_ls` quaternion and translation, without normalization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SocSegmentTransform {
    /// The stored quaternion in W, X, Y, Z order.
    pub rotation: [f64; 4],
    /// The stored segment-local translation vector.
    pub translation: [f64; 3],
}

/// One authored segment point at its original element position.
#[derive(Debug, Clone, Copy)]
pub struct SocSegmentPoint<'document> {
    group_ordinal: Option<usize>,
    node: SocSourceNode<'document>,
    position_name: &'static str,
}

impl<'document> SocSegmentPoint<'document> {
    /// Returns the optional outline group slot and point slot in its parent.
    #[must_use]
    pub const fn ordinal(self) -> (Option<usize>, usize) {
        (self.group_ordinal, self.node.ordinal())
    }

    /// Returns the original point element and its attribute coordinates.
    #[must_use]
    pub const fn element(self) -> SourceElement<'document> {
        self.node.element()
    }

    /// Reads the original segment-local point without modifying its slot.
    #[must_use]
    pub fn position(self) -> SocSourceValue<[f64; 3]> {
        self.node.read(self.position_name, vector)
    }
}

fn vector<const N: usize>(text: &str) -> Option<[f64; N]> {
    soc_entities::numbers(Some(text))
}

fn finite(text: &str) -> Option<f64> {
    let value: f64 = text.trim().parse().ok()?;
    value.is_finite().then_some(value)
}

fn parse_transform(text: &str) -> Option<SocSegmentTransform> {
    let mut rotation = None;
    let mut translation = None;
    for field in text.split('|') {
        let (name, value) = field.split_once(':')?;
        match name.trim() {
            "Q" if rotation.is_none() => rotation = Some(vector(value)?),
            "T" if translation.is_none() => translation = Some(vector(value)?),
            _ => return None,
        }
    }
    Some(SocSegmentTransform {
        rotation: rotation?,
        translation: translation?,
    })
}
