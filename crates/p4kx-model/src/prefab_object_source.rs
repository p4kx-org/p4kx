// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_cryxml::{SourceAttributePosition, SourceElementPosition};

use crate::mtl_xml::XmlElement;
use crate::prefab_entity_links::PrefabEntityLinksSource;
use crate::prefab_guid_remapping::PrefabGuidRemappingBlockSource;

/// Original transform operands of one Prefab object or its parent.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabTransformInputs {
    /// The owning object element, including the scope of defaulted operands.
    pub element: SourceElementPosition,
    /// The original `Pos` attribute when authored.
    pub position: Option<SourceAttributePosition>,
    /// The original `Rotate` attribute when authored.
    pub rotation: Option<SourceAttributePosition>,
    /// The original `Scale` attribute when authored.
    pub scale: Option<SourceAttributePosition>,
}

impl PrefabTransformInputs {
    pub(crate) fn new<'a>(element: impl XmlElement<'a>) -> Self {
        Self {
            element: element.position(),
            position: element.attribute_position("Pos"),
            rotation: element.attribute_position("Rotate"),
            scale: element.attribute_position("Scale"),
        }
    }
}

/// A containing object whose local transform contributes to a flattened object.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabTransformAncestor {
    /// This parent's local transform operands.
    pub transform: PrefabTransformInputs,
    /// The parent's `Id` used by an immediate child when comparing `Parent`.
    pub id: Option<SourceAttributePosition>,
    /// This parent's containing object, if present.
    pub parent: Option<Arc<Self>>,
}

/// The geometry selector selected for an object's emitted model path.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrefabModelSource {
    /// `Brush` objects select their `Prefab` attribute.
    Brush(Option<SourceAttributePosition>),
    /// The object's nonempty `Geometry` attribute.
    Geometry(Option<SourceAttributePosition>),
    /// Geometry selected through the entity resource chain.
    Resource {
        /// The direct `Geometry` selector that was absent or empty.
        geometry: Option<SourceAttributePosition>,
        /// The last inspected resource element and its selector.
        end: PrefabResourceEnd,
    },
}

/// The final observed step in an Entity's resource geometry lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrefabResourceEnd {
    /// The terminal Geometry element and its authored path, if present.
    Path {
        /// The terminal Geometry element.
        element: SourceElementPosition,
        /// The original `path` attribute when authored.
        position: Option<SourceAttributePosition>,
    },
    /// A required child was absent from the observed resource element.
    MissingChild {
        /// The element searched for the missing child.
        parent: SourceElementPosition,
        /// The child sought in that element.
        child: PrefabResourceChild,
    },
}

/// A child sought along the Entity resource geometry route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrefabResourceChild {
    /// The Entity's `PropertiesDataCore` child.
    PropertiesDataCore,
    /// The properties' `EntityGeometryResource` child.
    EntityGeometryResource,
    /// A child in the nested `Geometry` chain.
    Geometry,
}

impl PrefabResourceChild {
    /// Returns the original child tag consulted by the geometry lookup.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::PropertiesDataCore => "PropertiesDataCore",
            Self::EntityGeometryResource => "EntityGeometryResource",
            Self::Geometry => "Geometry",
        }
    }
}

/// Original selectors for an object that instances a prefab.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabReferenceSource {
    /// The object's `PrefabName` when authored.
    pub name: Option<SourceAttributePosition>,
    /// The object's `PrefabGUID` when authored.
    pub guid: Option<SourceAttributePosition>,
    /// Original blocks and pair positions matching the parsed remapping values.
    pub guid_remapping_blocks: Vec<PrefabGuidRemappingBlockSource>,
}

/// An attribute observed while evaluating a selector.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrefabAttributeObservation {
    /// The attribute was absent from its owning element.
    Absent,
    /// The attribute's original position.
    Present(SourceAttributePosition),
}

impl PrefabAttributeObservation {
    pub(crate) fn observed(position: Option<SourceAttributePosition>) -> Self {
        position.map_or(Self::Absent, Self::Present)
    }
}

/// Original selectors that determine an object's attachment state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabAttachmentSource {
    /// The object's `Parent` when authored.
    pub parent: Option<SourceAttributePosition>,
    /// Whether the authored parent text was compared against the immediate containing Object's Id.
    pub compared_group_id: bool,
    /// The `AttachmentTarget` observation if evaluated by the parser.
    pub attachment_target: Option<PrefabAttributeObservation>,
}

/// Original XML sites of a surviving flattened Prefab object.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabObjectSource {
    /// The object's `Name`, when authored.
    pub name: Option<SourceAttributePosition>,
    /// The object's `Type`, when authored.
    pub kind: Option<SourceAttributePosition>,
    /// The object's `Id`, when authored.
    pub id: Option<SourceAttributePosition>,
    /// The object's `EntityClass`, when authored.
    pub class: Option<SourceAttributePosition>,
    /// The selected geometry operand or surveyed absence.
    pub model: PrefabModelSource,
    /// The authored selectors when the object instances another prefab.
    pub reference: Option<PrefabReferenceSource>,
    /// Original coordinates in the parsed `EntityLinks` group and link order.
    pub entity_links: Option<Box<PrefabEntityLinksSource>>,
    /// The selectors evaluated for the object's attachment flag.
    pub attachment: PrefabAttachmentSource,
    /// The object's own local transform operands.
    pub transform: PrefabTransformInputs,
    /// The immediate containing Object and its outer chain, when grouped.
    pub groups: Option<Arc<PrefabTransformAncestor>>,
}
