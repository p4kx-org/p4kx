// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::model::{Material, PaletteSlot};
use schemars::JsonSchema;
use serde::Serialize;

use crate::Float32;

/// Numeric material values interpreted by the existing surface reader, including its defaults.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(extend("allOf" = [{"required": ["palette", "decal_colours", "diffuse_map_is_grey"]}]))]
#[non_exhaustive]
pub struct MaterialSurface<'source> {
    /// Fixed interpretation boundary; `raw_bits` describe interpreted `f32` values, not XML bytes.
    pub basis: MaterialSurfaceBasis,
    /// Existing interpreted flag word; original `MtlFlags` text remains in authored syntax.
    pub flags: u32,
    /// Existing unmodified shader declaration.
    pub shader: Cow<'source, str>,
    /// Existing physical surface declaration.
    pub surface_type: Cow<'source, str>,
    /// Existing unmodified generation-mask declaration.
    pub string_gen_mask: Cow<'source, str>,
    /// Interpreted diffuse colour.
    pub diffuse: [Float32; 3],
    /// Interpreted specular colour.
    pub specular: [Float32; 3],
    /// Interpreted emissive colour.
    pub emissive: [Float32; 3],
    /// Interpreted glossiness.
    pub shininess: Float32,
    /// Interpreted opacity.
    pub opacity: Float32,
    /// Interpreted alpha-test threshold.
    pub alpha_test: Float32,
    /// Interpreted glow multiplier.
    pub glow: Float32,
    /// Derived palette selection, separate from raw layer attributes.
    pub palette: Option<MaterialPalette>,
    /// Whether a loaded palette modified the interpreted colours.
    pub tinted: bool,
    /// Derived stencil-mask colours, if applied by a palette.
    pub decal_colours: Option<[[Float32; 3]; 3]>,
    /// Optional existing decoded-texture fact, absent during plain source reading.
    pub diffuse_map_is_grey: Option<bool>,
}

/// Origin of interpreted surface values, independent of exact raw source tokens.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MaterialSurfaceBasis {
    /// Existing material reader rules and documented fallbacks.
    ExistingReaderWithDefaults,
}

/// Existing derived palette entry, without changing rendering rules.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MaterialPalette {
    /// Primary palette entry.
    Primary,
    /// Secondary palette entry.
    Secondary,
    /// Tertiary palette entry.
    Tertiary,
    /// Glass entry.
    Glass,
    /// Stencil decal colours.
    Decal,
    /// An unsupported future reader entry.
    Unsupported,
}

impl<'source> MaterialSurface<'source> {
    pub(crate) fn of(material: &'source Material) -> Self {
        Self {
            basis: MaterialSurfaceBasis::ExistingReaderWithDefaults,
            flags: material.flags.0,
            shader: Cow::Borrowed(&material.shader),
            surface_type: Cow::Borrowed(&material.surface_type),
            string_gen_mask: Cow::Borrowed(&material.string_gen_mask),
            diffuse: material.diffuse.map(Float32::new),
            specular: material.specular.map(Float32::new),
            emissive: material.emissive.map(Float32::new),
            shininess: Float32::new(material.shininess),
            opacity: Float32::new(material.opacity),
            alpha_test: Float32::new(material.alpha_test),
            glow: Float32::new(material.glow),
            palette: material.palette.map(|palette| match palette {
                PaletteSlot::Primary => MaterialPalette::Primary,
                PaletteSlot::Secondary => MaterialPalette::Secondary,
                PaletteSlot::Tertiary => MaterialPalette::Tertiary,
                PaletteSlot::Glass => MaterialPalette::Glass,
                PaletteSlot::Decal => MaterialPalette::Decal,
                _ => MaterialPalette::Unsupported,
            }),
            tinted: material.tinted,
            decal_colours: material
                .decal_colours
                .map(|colours| colours.map(|colour| colour.map(Float32::new))),
            diffuse_map_is_grey: material.diffuse_map_is_grey,
        }
    }
}
