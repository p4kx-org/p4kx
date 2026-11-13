// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::{DdsPngEncoding, DdsTransfer};

/// A physical texture read identifier supplied by the loader for this export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureReadId(usize);

impl TextureReadId {
    /// Binds a read to the loader's own evidence slot.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the evidence slot in the loader that minted this ID.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// The parts of a decoded texture actually used by an embedded image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextureComponents {
    /// Only the dimensions of the source image.
    Dimensions,
    /// The RGB pixel data.
    Color,
    /// The alpha or smoothness data.
    Alpha,
    /// RGB and alpha pixel data.
    ColorAndAlpha,
}

/// One selected decoded texture used to form an embedded image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureReadUse {
    /// The observed loader read, if the loader supplied evidence.
    pub read: Option<TextureReadId>,
    /// The components consumed by the output image.
    pub components: TextureComponents,
    /// Whether the loaded bytes came through a secondary texture-name fallback.
    pub fallback: bool,
    /// The effective original DDS declaration, when established.
    pub declared_transfer: DdsTransfer,
    /// The transfer and decoder transform consulted when the input supplied colour.
    pub transfer: Option<DdsPngEncoding>,
}

/// The material values a texture conversion used.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum TexturePaletteUse {
    /// The image conversion used no palette-controlled values.
    #[default]
    None,
    /// The image conversion used diffuse and specular shading.
    DiffuseAndSpecular,
    /// The image conversion used specular shading.
    Specular,
    /// The image conversion used stencil decal colours.
    Decal,
    /// The image conversion used tint values of the selected layers.
    Layers,
}

/// The material that supplied the selected job for an embedded image.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TextureMaterialSource {
    /// The part that supplied the selected job.
    pub part: usize,
    /// The material's library slot, if the source is not the built-in material.
    pub slot: Option<usize>,
    /// The original material-layer positions used by the conversion.
    pub layers: Vec<usize>,
    /// Whether the image conversion used palette-controlled values.
    pub uses_palette: bool,
    /// Whether the image conversion used the material's own diffuse colour.
    pub uses_diffuse: bool,
}
