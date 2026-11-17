// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

/// Which glTF texture a material file's texture would have fed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextureRole {
    /// The diffuse map, glTF's base colour texture.
    Diffuse,
    /// The normal map, whose alpha channel may hold the smoothness.
    Normal,
    /// The specular map, which gives the metalness.
    Specular,
    /// The explicit gloss map, whose RGB gives the smoothness.
    Gloss,
}

impl TextureRole {
    /// Returns the name used in texture diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Diffuse => "diffuse",
            Self::Normal => "normal",
            Self::Specular => "specular",
            Self::Gloss => "gloss",
        }
    }
}

impl fmt::Display for TextureRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A texture of a material that the GLB lacks because it could not be found or decoded, or
/// would exceed a budget: the material keeps its plain colour for it.
///
/// The names come from the Archive and are untrusted: sanitise them before printing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TextureFallback {
    /// The material's name, or `material_<position>` when the file gives none.
    pub material: String,
    /// What the texture would have been used for.
    pub role: TextureRole,
    /// The texture file as the material file names it.
    pub file: String,
    /// Why the texture is not used.
    pub reason: String,
}
