// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

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
}
