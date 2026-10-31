// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What a texture of a material is used for, by the file naming convention of the game's
/// textures and the slot it sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextureUse {
    /// The colour map (`_diff`, and any other map in `TexSlot1`).
    Diffuse,
    /// The normal map (`_ddna` with the smoothness in alpha, or `_ddn`).
    Normal,
    /// The specular map (`_spec`).
    Specular,
    /// A mask or opacity map (`_mask`, `_opac`).
    Mask,
    /// A blend map between layers (`_blend`).
    Blend,
    /// A detail map (`_detail`).
    Detail,
    /// A wear map (`_wear`).
    Wear,
    /// An ambient occlusion map (`_ao`).
    Occlusion,
    /// A displacement or height map (`_displ`, `_disp`).
    Displacement,
    /// A gloss map (`_gloss`).
    Gloss,
    /// The stencil map of a stencil decal (`STENCIL_MAP`, 1.6): its red channel is the opacity of
    /// the motif.
    Stencil,
    /// Any other map; the slot and file say more.
    Other,
}

impl From<p4kx_model::TextureRole> for TextureUse {
    fn from(role: p4kx_model::TextureRole) -> Self {
        use p4kx_model::TextureRole;
        match role {
            TextureRole::Diffuse => Self::Diffuse,
            TextureRole::Normal => Self::Normal,
            TextureRole::Specular => Self::Specular,
            TextureRole::Mask => Self::Mask,
            TextureRole::Blend => Self::Blend,
            TextureRole::Detail => Self::Detail,
            TextureRole::Wear => Self::Wear,
            TextureRole::Occlusion => Self::Occlusion,
            TextureRole::Displacement => Self::Displacement,
            TextureRole::Gloss => Self::Gloss,
            TextureRole::Stencil => Self::Stencil,
            _ => Self::Other,
        }
    }
}

/// How a material is blended with what is behind it, derived from its opacity and alpha test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AlphaMode {
    /// Opaque.
    Opaque,
    /// Alpha tested: the surface is drawn where alpha reaches `alpha_test`.
    Mask,
    /// Blended by opacity: `opacity` below 1, or a glass or decal shader.
    Blend,
}

/// Which colour of a palette a material or layer takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PaletteEntry {
    /// The primary entry (`PaletteTint` 1).
    Primary,
    /// The secondary entry (`PaletteTint` 2).
    Secondary,
    /// The tertiary entry (`PaletteTint` 3).
    Tertiary,
    /// The glass colour.
    Glass,
}

/// A name and a value as the material file gives them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Parameter {
    /// The attribute's name.
    pub name: String,
    /// The attribute's value, text as in the file.
    pub value: String,
}

/// An XML element of a material file the writer does not interpret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Element {
    /// The element's name.
    pub name: String,
    /// The element's attributes in file order.
    pub attributes: Vec<Parameter>,
    /// The element's children in file order.
    pub children: Vec<Self>,
}

/// One texture slot of a material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MaterialTexture {
    /// The slot as the file names it: `TexSlot1`, `TexSlot2`, and so on.
    pub slot: String,
    /// The texture as the file names it, source image or engine placeholder such as
    /// `$TintPaletteDecal`.
    pub file: String,
    /// What the texture is used for.
    pub usage: TextureUse,
    /// The id of the texture in `textures`; absent for a placeholder or a texture that could not
    /// be written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    /// The other attributes of the slot in file order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<Parameter>,
    /// The child elements of the slot, such as `TexMod` with the tiling and offset of the UV set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Element>,
}

/// One layer of a material: another material blended into the surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MaterialLayer {
    /// The layer's name (`Name`), e.g. `Primary` or `Wear`; empty when the file gives none.
    pub name: String,
    /// The layer material's file (`Path`), a path below `Data` with its extension; empty when the
    /// file gives none.
    pub path: String,
    /// The sub-material of the layer file (`Submtl`) by name; absent when the layer names none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_material: Option<String>,
    /// The tint colour (`TintColor`), linear, that multiplies the layer's colour; absent when the
    /// file gives none or it is malformed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint_colour: Option<[f32; 3]>,
    /// The tint colour of the layer where it is worn (`WearTint`), linear.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wear_tint: Option<[f32; 3]>,
    /// The tiling of the layer's UVs (`UVTiling`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_tiling: Option<f32>,
    /// The factor of the layer's glossiness (`GlossMult`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gloss_mult: Option<f32>,
    /// All attributes of the layer in file order, the five above included, and `PaletteTint`,
    /// `WearGloss`, `HeightBias`, `HeightScale` and any other.
    pub attributes: Vec<Parameter>,
    /// The palette entry the layer takes, from its `PaletteTint`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<PaletteEntry>,
    /// The id of the material the layer's `Path` names, in `materials`; absent when the layer names
    /// no material, or the file is missing or unreadable (an issue is listed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
}

/// How a stencil decal is drawn (1.6); the stencil map itself is the texture with the usage
/// `stencil`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MaterialStencil {
    /// Whether the decal draws anything: it has a stencil map or a diffuse map that was written.
    /// `false` for a decal whose motif the game fills in at run time and for one whose maps could
    /// not be written; the add-on then draws nothing, as the GLB does.
    pub drawn: bool,
    /// The colour of the motif (`StencilDiffuseColor`), linear; stated for a decal drawn by its
    /// stencil map only. Absent when the file gives none or it is malformed, and the material's
    /// `diffuse` counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<[f32; 3]>,
    /// The opacity of the motif (`StencilOpacity`) from 0 to 1, 1 when the file gives none or the
    /// decal is drawn by a diffuse map; it multiplies the material's `opacity`.
    pub opacity: f32,
}

impl MaterialStencil {
    /// Returns how `material`, a stencil decal, is drawn when all its textures are written.
    pub(crate) fn of(material: &p4kx_model::Material) -> Option<Self> {
        material.stencil()?;
        let motif = material.stencil_motif();
        Some(Self {
            drawn: !material.is_stencil_without_motif(),
            colour: motif.and_then(|stencil| stencil.colour),
            opacity: motif.map_or(1.0, |stencil| stencil.opacity),
        })
    }
}

/// A material as the game defines it: the inputs of a Blender or any other renderer's material.
///
/// Colours are linear and exactly those of the file; a palette is not applied. The
/// `parameters`, `attributes` and `elements` hold what the writer does not interpret, name and
/// value as the file has them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MaterialDescription {
    /// The material's id, e.g. `x0001`.
    pub id: String,
    /// The material's name in its file; empty when the file gives none.
    pub name: String,
    /// The material library the material is in, a path below `Data` without extension.
    pub library: String,
    /// The position of the material in the library's sub-material list, the material ID of the
    /// submeshes; absent when the file is one material that serves every ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    /// The shader, e.g. `Illum`, `HardSurface`, `Layer`, `MeshDecal` or `GlassPBR`; empty when the
    /// file gives none.
    pub shader: String,
    /// The shader generation mask, split at `%`: `%NORMAL_MAP%SPECULAR_MAP` is `NORMAL_MAP` and
    /// `SPECULAR_MAP`.
    pub gen_mask: Vec<String>,
    /// The physical surface type, e.g. `metal_dense`; empty when the file gives none.
    pub surface_type: String,
    /// `MtlFlags`, the engine's material flags.
    pub flags: u32,
    /// Whether the engine draws both faces (flag `0x2`).
    pub two_sided: bool,
    /// Whether the engine draws nothing with the material (flag `0x400` or shader `NoDraw`).
    pub no_draw: bool,
    /// The diffuse colour, linear.
    pub diffuse: [f32; 3],
    /// The specular colour, linear.
    pub specular: [f32; 3],
    /// The emissive colour, linear; it counts times `glow`.
    pub emissive: [f32; 3],
    /// The glossiness from 0 to 255.
    pub shininess: f32,
    /// The opacity from 0 to 1.
    pub opacity: f32,
    /// The alpha-test threshold, 0 when the material is not alpha tested.
    pub alpha_test: f32,
    /// The glow amount the emissive colour is multiplied by.
    pub glow: f32,
    /// How the material is blended, derived from `opacity`, `alpha_test` and the shader.
    pub alpha_mode: AlphaMode,
    /// Whether the material reads the vertex colours of its mesh (1.4): glass with
    /// `PER_VERTEX_TINT`, and a layered surface without a blend map. The GLB meshes carry
    /// `COLOR_0` on the primitives of such materials only. A package before 1.4 lacks it and reads
    /// as `false` here, which is harmless: this crate only writes it, and the add-on tells an
    /// absent field from `false` itself.
    #[serde(default)]
    pub reads_vertex_colours: bool,
    /// The share of each layer of `layers` without a blend map or vertex colours (1.5), by
    /// position and `WearBlendBase` (`Material::layer_shares`, the rule the GLB shades by): the
    /// first layer `1 - WearBlendBase`, the others `WearBlendBase` between them, a layer without a
    /// `material` has 0, all shares sum to 1. All 0 means the material keeps its own colours.
    /// Empty when the material has no layers; a package before 1.5 lacks it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layer_shares: Vec<f32>,
    /// How a stencil decal (`MeshDecal` with `STENCIL_MAP`) is drawn (1.6); absent for any other
    /// material and in a package before 1.6. The motif is the texture with the usage `stencil`; a
    /// decal with a diffuse map is drawn by it, and one with neither draws nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stencil: Option<MaterialStencil>,
    /// The palette entry the material takes: that of its first layer with a `PaletteTint` from 1
    /// to 3, the glass colour of a glass material that uses the tint palette, or the primary entry
    /// of a stencil decal with `StencilTintOverride`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<PaletteEntry>,
    /// The tint palette the library names as its default, a Record path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_palette: Option<String>,
    /// The texture slots in file order.
    pub textures: Vec<MaterialTexture>,
    /// The layers in file order.
    pub layers: Vec<MaterialLayer>,
    /// The shader parameters (`PublicParams`) in file order, name and value as the file has them.
    pub parameters: Vec<Parameter>,
    /// The attributes of the material element that no field above holds, in file order.
    pub attributes: Vec<Parameter>,
    /// The child elements of the material that are none of textures, layers, parameters or
    /// sub-materials, as the file has them.
    pub elements: Vec<Element>,
}

/// The colours of one palette entry, linear.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PaletteColours {
    /// The colour of the paint.
    pub tint: [f32; 3],
    /// The specular colour.
    pub specular: [f32; 3],
    /// The glossiness from 0 to 255.
    pub glossiness: f32,
}

/// A tint palette: the colours a livery gives the materials that take a palette entry.
///
/// A palette is written once; meshes reference it by id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Palette {
    /// The palette's id, e.g. `p0001`.
    pub id: String,
    /// The palette's name.
    pub name: String,
    /// The primary entry.
    pub primary: PaletteColours,
    /// The secondary entry.
    pub secondary: PaletteColours,
    /// The tertiary entry.
    pub tertiary: PaletteColours,
    /// The colour of the tinted glass, linear.
    pub glass: [f32; 3],
}
