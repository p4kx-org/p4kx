// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::MAX_INFLUENCES;

/// How an export writes the materials.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MaterialMode {
    /// Plain colours: base colour, opacity, roughness, metalness and emissive colour, no
    /// textures.
    #[default]
    Colours,
    /// The plain colours with the textures of the materials as embedded PNG images: the diffuse
    /// map as base colour, the normal map, and a metallic-roughness image made of the smoothness
    /// in the normal map's alpha channel and the metalness of the specular map. A texture that
    /// cannot be loaded leaves the material's plain colour, and is listed in the summary.
    Textures,
}

/// The choices of a GLB export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct GlbOptions {
    /// The level of detail to export, 0 for the most detailed.
    pub lod: u32,
    /// How to write the materials.
    pub materials: MaterialMode,
    /// The mip level of the textures, 0 for the largest; a texture with fewer mips gives its
    /// smallest.
    pub texture_mip: u32,
    /// Whether to write the vertex colours of the meshes that have any as `COLOR_0`, on the
    /// primitives whose material takes them as its colour
    /// ([tinted glass](p4kx_model::Material::tints_by_vertex_colours)); viewers multiply the base
    /// colour of the material by them.
    pub vertex_colours: bool,
    /// Whether the vertex colours of a layered surface without a blend map are written too, though
    /// they are the shares of its layers and no colour: a viewer would multiply them into the
    /// base colour. Only for consumers that read them as shares, such as the Blender package.
    /// Without [`vertex_colours`](Self::vertex_colours) it has no effect.
    pub layer_vertex_colours: bool,
    /// The most bones a vertex is weighted to in the skins of the file, 1 to 8: the strongest ones
    /// are kept and their weights scaled to add up to 1. At most 4 writes `JOINTS_0` and
    /// `WEIGHTS_0` only, for viewers that ignore the second set.
    pub max_influences: usize,
    /// Whether every glTF material gets the `extras` `{"p4kx_index": <index>}`, its index in the
    /// file; the rigged writers ignore it.
    pub material_indices: bool,
}

impl Default for GlbOptions {
    fn default() -> Self {
        Self {
            lod: 0,
            materials: MaterialMode::Colours,
            texture_mip: 0,
            vertex_colours: true,
            layer_vertex_colours: false,
            max_influences: MAX_INFLUENCES,
            material_indices: false,
        }
    }
}

impl GlbOptions {
    /// Returns the options with the level of detail `lod`.
    #[must_use]
    pub const fn with_lod(mut self, lod: u32) -> Self {
        self.lod = lod;
        self
    }

    /// Returns the options with the material mode `materials`.
    #[must_use]
    pub const fn with_materials(mut self, materials: MaterialMode) -> Self {
        self.materials = materials;
        self
    }

    /// Returns the options with the texture mip level `mip`, 0 for the largest.
    #[must_use]
    pub const fn with_texture_mip(mut self, mip: u32) -> Self {
        self.texture_mip = mip;
        self
    }

    /// Returns the options with at most `count` bones per vertex, clamped to 1 to 8.
    #[must_use]
    pub fn with_max_influences(mut self, count: usize) -> Self {
        self.max_influences = count.clamp(1, MAX_INFLUENCES);
        self
    }

    /// Returns the options with the `extras` `p4kx_index` on every material.
    #[must_use]
    pub const fn with_material_indices(mut self) -> Self {
        self.material_indices = true;
        self
    }

    /// Returns the options that also write the vertex colours of layered surfaces, the shares of
    /// their layers.
    #[must_use]
    pub const fn with_layer_vertex_colours(mut self) -> Self {
        self.layer_vertex_colours = true;
        self
    }

    /// Returns the options without vertex colours.
    #[must_use]
    pub const fn without_vertex_colours(mut self) -> Self {
        self.vertex_colours = false;
        self
    }
}
