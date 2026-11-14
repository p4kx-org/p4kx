// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::{
    MaterialElementPart, MaterialEncodingInfo, MaterialParameterInfo, MaterialSourceEntryInfo,
    MaterialSourceLossInfo,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::material_surface::MaterialSurface;

/// Library metadata preceding the ordered material rows for one physical source.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("required" = ["default_palette"]))]
pub(crate) struct MaterialLibraryHeader<'source> {
    pub(crate) path: Cow<'source, str>,
    pub(crate) single: bool,
    pub(crate) default_palette: Option<Cow<'source, str>>,
    pub(crate) unknown_entries: usize,
    pub(crate) encoding: MaterialEncodingInfo,
    pub(crate) authored_root: bool,
    pub(crate) materials: usize,
    pub(crate) source_entries: usize,
    pub(crate) source_losses: usize,
    pub(crate) projection_complete: bool,
}

/// Exact record counts for the arrays in one streamed material library.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
pub(crate) struct MaterialLibraryEnd {
    pub(crate) parts: u64,
    pub(crate) references: u64,
    pub(crate) shader_references: u64,
}

/// An individual bounded projection from the material source.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub(crate) enum MaterialPart<'source> {
    /// One original submaterial disposition.
    SourceEntry {
        position: usize,
        entry: MaterialSourceEntryInfo<'source>,
    },
    /// One diagnosed authored-tree capture boundary.
    SourceLoss {
        position: usize,
        loss: MaterialSourceLossInfo,
    },
    /// One element, declaration, attribute or mixed-content value.
    Element {
        tree: MaterialTree,
        part: MaterialElementPart<'source>,
    },
    /// One retained material slot and its independent interpreted surface.
    Surface {
        index: usize,
        name: Cow<'source, str>,
        interpreted_surface: Box<MaterialSurface<'source>>,
        authored: MaterialAuthoredCounts,
    },
    /// One typed, ordered raw attribute of a material or one of its children.
    Attribute {
        material: usize,
        group: MaterialAttributeGroup,
        position: usize,
        value: MaterialParameterInfo<'source>,
    },
    /// One layer slot, including an empty declaration.
    Layer {
        material: usize,
        position: usize,
        attributes: usize,
    },
    /// One texture slot, including its original map and file tokens.
    Texture {
        material: usize,
        position: usize,
        map: Cow<'source, str>,
        file: Cow<'source, str>,
        attributes: usize,
        children: usize,
    },
    /// One explicit typed-reader truncation.
    Truncation {
        material: usize,
        position: usize,
        value: Cow<'source, str>,
    },
}

/// Original output group in which the raw element tree is retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub(crate) enum MaterialTree {
    /// The original document root.
    Library,
    /// One unknown block retained by the surface reader.
    Material { material: usize, position: usize },
    /// One separate tree retained under the original texture slot.
    Texture {
        material: usize,
        texture: usize,
        position: usize,
    },
}

/// Group whose own attribute positions are kept in source order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub(crate) enum MaterialAttributeGroup {
    /// Attributes outside the existing interpreted surface fields.
    Other,
    /// Original parameter attributes.
    PublicParam,
    /// Attributes of one selected layer.
    Layer { position: usize },
    /// Attributes of one selected texture slot.
    Texture { position: usize },
}

/// Cardinalities of the source data retained by the existing surface reader.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct MaterialAuthoredCounts {
    pub(crate) other_attributes: usize,
    pub(crate) public_params: usize,
    pub(crate) layers: usize,
    pub(crate) elements: usize,
    pub(crate) truncated: usize,
    pub(crate) textures: usize,
}

impl MaterialAuthoredCounts {
    pub(crate) const fn of(material: &p4kx_extraction::model::Material) -> Self {
        Self {
            other_attributes: material.other_attributes.len(),
            public_params: material.public_params.len(),
            layers: material.layers.len(),
            elements: material.elements.len(),
            truncated: material.truncated.len(),
            textures: material.textures.len(),
        }
    }
}
