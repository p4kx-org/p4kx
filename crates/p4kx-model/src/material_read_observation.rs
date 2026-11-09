// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// One interpreted output whose original material inputs are being observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Material ID position, including unknown slots that use the engine default.
    pub material: usize,
    /// The interpreted result receiving this contribution.
    pub field: MaterialReadField,
    /// Original element-child path, never a compacted typed-list position.
    pub indices: &'a [usize],
    /// Original field spelling or the semantic source query used by this operation.
    pub source_field: &'a str,
    /// The actual use of this source site.
    pub use_kind: MaterialReadUse,
}
