// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttributePosition, SourceElementPosition};

use crate::mtl_xml::XmlElement;
use crate::{PrefabLibrary, PrefabObjectSource};

/// A Prefab library and its original source coordinates.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PositionedPrefabLibrary {
    /// The semantic library.
    pub library: PrefabLibrary,
    /// The positions, absent when XML decoding repaired the input.
    pub source: Option<PrefabLibrarySource>,
}

/// The selected source of the library name.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrefabLibraryNameSource {
    /// The root's nonempty `Name` attribute.
    Attribute(SourceAttributePosition),
    /// The name of the root element.
    Element(SourceElementPosition),
}

/// Original coordinates for the library detail projection.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabLibrarySource {
    /// The library root that owns the selected Prefab rows.
    pub root: SourceElementPosition,
    /// The origin of the emitted library name.
    pub name: PrefabLibraryNameSource,
    /// Prefab positions in the same order as the semantic library's rows.
    pub prefabs: Vec<PrefabSource>,
}

impl PrefabLibrarySource {
    pub(crate) fn new<'a>(root: impl XmlElement<'a>, authored_name: Option<&str>) -> Option<Self> {
        let position = root.position();
        let name = if authored_name.is_some() {
            PrefabLibraryNameSource::Attribute(root.attribute_position("Name")?)
        } else {
            PrefabLibraryNameSource::Element(position.clone())
        };
        Some(Self {
            root: position,
            name,
            prefabs: Vec::new(),
        })
    }
}

/// Original coordinates for the fields of one Prefab row.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabSource {
    /// The owning `Prefab` node, which also records the scope of absent fields.
    pub element: SourceElementPosition,
    /// The `Name` attribute, when authored.
    pub name: Option<SourceAttributePosition>,
    /// The `Id` attribute, when authored.
    pub id: Option<SourceAttributePosition>,
    /// The selected `Objects` subtree used for object, nested Prefab and omitted counts.
    pub objects: Option<SourceElementPosition>,
    /// Object positions in flattened semantic order when requested.
    pub object_sources: Option<Vec<PrefabObjectSource>>,
}

impl PrefabSource {
    pub(crate) fn new<'a, E: XmlElement<'a>>(element: E, objects: Option<E>) -> Self {
        Self {
            element: element.position(),
            name: element.attribute_position("Name"),
            id: element.attribute_position("Id"),
            objects: objects.map(XmlElement::position),
            object_sources: None,
        }
    }
}
