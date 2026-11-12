// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// One output with an observed original material input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum MaterialReadField {
    /// The copied material name.
    Name,
    /// The parsed engine flags.
    Flags,
    /// The copied shader name.
    Shader,
    /// The copied physical surface type.
    SurfaceType,
    /// The copied shader generation mask.
    StringGenMask,
    /// The interpreted diffuse colour.
    Diffuse,
    /// The interpreted specular colour.
    Specular,
    /// The interpreted emissive colour.
    Emissive,
    /// The interpreted shininess.
    Shininess,
    /// The interpreted opacity.
    Opacity,
    /// The interpreted alpha-test threshold.
    AlphaTest,
    /// The interpreted glow.
    Glow,
    /// The selected layer, glass or stencil palette slot.
    Palette,
    /// Whether the selected library is a single material.
    LibrarySingle,
    /// The admitted nonempty default palette attribute.
    LibraryDefaultPalette,
    /// The selected material list's cardinality.
    LibraryTotal,
    /// The selected list's count of engine-default slots.
    LibraryUnknownCount,
    /// The selection and position of a material slot.
    Row,
    /// The material's admitted texture slots.
    Textures,
    /// The admitted texture slot's element.
    Texture {
        /// Position in the retained output collection.
        index: usize,
    },
    /// The slot name or default selected by the admitted texture.
    TextureMap {
        /// Position in the retained output collection.
        index: usize,
    },
    /// The admitted texture's authored filename.
    TextureFile {
        /// Position in the retained output collection.
        index: usize,
    },
    /// The material's authored public parameters.
    PublicParams,
    /// The retained parameter's name and value.
    PublicParam {
        /// Position in the retained output collection.
        index: usize,
    },
    /// The material's admitted layers.
    Layers,
    /// The retained layer's element.
    Layer {
        /// Position in the retained output collection.
        index: usize,
    },
    /// The layer's selected authored name.
    LayerName {
        /// Position in the retained output collection.
        index: usize,
    },
    /// The layer's selected authored path.
    LayerPath {
        /// Position in the retained output collection.
        index: usize,
    },
    /// The layer's selected authored palette tint.
    LayerPaletteTint {
        /// Position in the retained output collection.
        index: usize,
    },
}

/// The operation for which an original material input actually contributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MaterialReadUse {
    /// A token copied directly into this output.
    Authored,
    /// A source presence, disposition or default branch selecting this output.
    Selected,
    /// An original token used to parse or derive this output.
    Interpreted,
}

/// A borrowed original source site actually used by one interpreted material output.
///
/// Indices are XML element-child ordinals from the library root; labels name actual fields or
/// semantic absence/disposition queries. They carry neither copied values nor source authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct MaterialReadObservation<'a> {
    /// Material ID position, or zero for library fields.
    pub material: usize,
    /// The interpreted result receiving this contribution.
    pub field: MaterialReadField,
    /// Original element-child path, never a compacted typed-list position.
    pub indices: &'a [usize],
    /// Original field spelling or the semantic source query used by this operation.
    pub source_field: &'a str,
    /// The original attribute ordinal when the selected occurrence is explicit.
    pub attribute_ordinal: Option<usize>,
    /// The actual use of this source site.
    pub use_kind: MaterialReadUse,
}
