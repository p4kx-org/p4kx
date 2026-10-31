// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_parse::parse_library;
use crate::mtl_raw::Attributes;
use crate::{
    CompanionResolver, LayerCache, MaterialError, MaterialLayer, MaterialReading, PaletteSlot,
    RawElement,
};

/// The most sub-materials a material library may hold; the engine allows 128.
pub const MAX_SUB_MATERIALS: usize = 1024;

/// The most texture slots one material may hold.
pub const MAX_TEXTURE_SLOTS: usize = 64;

/// The `MtlFlags` of a material, as the engine's `EMaterialFlags` define them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct MaterialFlags(pub u32);

impl MaterialFlags {
    const TWO_SIDED: u32 = 0x0002;
    const NO_DRAW: u32 = 0x0400;

    /// Returns whether the engine draws both faces of a triangle.
    #[must_use]
    pub const fn two_sided(self) -> bool {
        self.0 & Self::TWO_SIDED != 0
    }

    /// Returns whether the engine draws nothing with the material: collision, shield and
    /// helper geometry.
    #[must_use]
    pub const fn no_draw(self) -> bool {
        self.0 & Self::NO_DRAW != 0
    }
}

/// One texture slot of a material.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TextureSlot {
    /// The slot, as the file names it: `TexSlot1`, `TexSlot2`, and so on.
    pub map: String,
    /// The texture's path as the file gives it, below `Data`, or an engine placeholder such as
    /// `$RenderToTexture`.
    pub file: String,
    /// The slot's other attributes in file order, e.g. `TexType`.
    pub attributes: Attributes,
    /// The slot's child elements, e.g. `TexMod` with the tiling and offset.
    pub children: Vec<RawElement>,
}

/// One material: the surface a submesh is drawn with.
///
/// Colours are linear, as the file stores them.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Material {
    /// The material's name; empty when the file gives none.
    pub name: String,
    /// The engine flags.
    pub flags: MaterialFlags,
    /// The shader's name, e.g. `Illum` or `NoDraw`; empty when the file gives none.
    pub shader: String,
    /// The physical surface type, e.g. `metal_dense`; empty when the file gives none.
    pub surface_type: String,
    /// The shader generation mask, e.g. `%NORMAL_MAP%SPECULAR_MAP`; empty when the file gives
    /// none.
    pub string_gen_mask: String,
    /// The diffuse colour.
    pub diffuse: [f32; 3],
    /// The specular colour.
    pub specular: [f32; 3],
    /// The emissive colour, drawn when [`Self::glow`] is above zero.
    pub emissive: [f32; 3],
    /// The glossiness from 0 to 255.
    pub shininess: f32,
    /// The opacity from 0 to 1.
    pub opacity: f32,
    /// The alpha-test threshold, 0 when the material is not alpha tested.
    pub alpha_test: f32,
    /// The glow amount the emissive colour is multiplied by.
    pub glow: f32,
    /// The texture slots in file order.
    pub textures: Vec<TextureSlot>,
    /// The palette colour the material takes: that of its first material layer with a
    /// `PaletteTint` from 1 to 3, the glass colour of a glass material that uses the tint palette,
    /// or the primary entry of a [stencil decal](Self::is_stencil_decal) whose
    /// [`Stencil::palette_tint`](crate::Stencil::palette_tint) is set.
    pub palette: Option<PaletteSlot>,
    /// Whether a [`TintPalette`](crate::TintPalette) replaced the colours of the file.
    pub tinted: bool,
    /// The attributes of the material element that no field above holds, in file order.
    pub other_attributes: Attributes,
    /// The `PublicParams` attributes: the shader's parameters by name, in file order.
    pub public_params: Attributes,
    /// The `MatLayers` layers in file order.
    pub layers: Vec<MaterialLayer>,
    /// The child elements that are neither `Textures`, `PublicParams`, `MatLayers` nor
    /// `SubMaterials`, as the file has them.
    pub elements: Vec<RawElement>,
    /// What the reader's limits cut off, e.g. `layers beyond 64`; empty when nothing was.
    pub truncated: Vec<String>,
    /// Whether the diffuse map of a [relief decal](Self::is_relief_decal) candidate holds no colour
    /// of its own, as the resolver that loaded the library found out; `None` when it was not asked
    /// or could not tell.
    pub diffuse_map_is_grey: Option<bool>,
}

/// A parsed `PublicParams` value.
enum PublicValue {
    Number(f32),
    Colour([f32; 3]),
}

impl Material {
    /// Returns the shader parameter `name` of the material's `PublicParams`, ignoring case, as a
    /// number in the first channel or as a colour in all three; `None` when the material sets
    /// none or its value is neither.
    #[must_use]
    pub fn public_param(&self, name: &str) -> Option<[f32; 3]> {
        match self.public_value(name)? {
            PublicValue::Number(first) => Some([first, 0.0, 0.0]),
            PublicValue::Colour(colour) => Some(colour),
        }
    }

    /// Returns the shader parameter `name` of the material's `PublicParams`, ignoring case, when
    /// it is a colour of exactly three finite components; `None` for a number or anything else.
    #[must_use]
    pub fn public_colour(&self, name: &str) -> Option<[f32; 3]> {
        match self.public_value(name)? {
            PublicValue::Colour(colour) => Some(colour),
            PublicValue::Number(_) => None,
        }
    }

    /// Parses the parameter `name` as one finite number or three finite components.
    fn public_value(&self, name: &str) -> Option<PublicValue> {
        let (_, text) = self
            .public_params
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))?;
        let number = |part: &str| {
            part.trim()
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
        };
        let mut parts = text.split(',');
        let first = number(parts.next()?)?;
        match (parts.next(), parts.next(), parts.next()) {
            (None, ..) => Some(PublicValue::Number(first)),
            (Some(second), Some(third), None) => Some(PublicValue::Colour([
                first,
                number(second)?,
                number(third)?,
            ])),
            _ => None,
        }
    }

    /// Returns the material the engine draws a material ID without a sub-material with: a
    /// light grey, opaque, matt surface.
    #[must_use]
    pub fn engine_default() -> Self {
        Self {
            name: String::new(),
            flags: MaterialFlags::default(),
            shader: String::new(),
            surface_type: String::new(),
            string_gen_mask: String::new(),
            diffuse: [0.8; 3],
            specular: [0.04; 3],
            emissive: [0.0; 3],
            shininess: 10.0,
            opacity: 1.0,
            alpha_test: 0.0,
            glow: 0.0,
            textures: Vec::new(),
            palette: None,
            tinted: false,
            other_attributes: Vec::new(),
            public_params: Vec::new(),
            layers: Vec::new(),
            elements: Vec::new(),
            truncated: Vec::new(),
            diffuse_map_is_grey: None,
        }
    }

    /// Returns whether the material is an overlay of the `Eye` shader, `EYE_AO_OVERLAY` or
    /// `EYE_SPECULAR_OVERLAY` in its generation mask: a shell over the eyeball that the engine
    /// blends in as ambient occlusion or as a specular highlight, which no plain glTF material
    /// says. Drawn as a surface it would cover the iris.
    #[must_use]
    pub fn is_eye_overlay(&self) -> bool {
        self.shader.eq_ignore_ascii_case("Eye")
            && ["EYE_AO_OVERLAY", "EYE_SPECULAR_OVERLAY"]
                .iter()
                .any(|mask| self.string_gen_mask.contains(mask))
    }

    /// Returns whether the material is a `HairPBR` cap: `HAIR_CAP` in its generation mask, a shell
    /// over the skin that its `TexSlot1` density map blends.
    #[must_use]
    pub fn is_hair_cap(&self) -> bool {
        self.shader.eq_ignore_ascii_case("HairPBR") && self.string_gen_mask.contains("HAIR_CAP")
    }

    /// Returns whether the material is a decal that draws no colour: a `MeshDecal` without any
    /// texture slot, a placeholder a livery fills in, a decal (`DECAL` in its generation mask)
    /// whose `DecalDiffuseOpacity` is 0, which only adds relief to the surface below, or a
    /// [relief decal](Self::is_relief_decal), a
    /// [render-to-texture decal](Self::is_render_to_texture_decal), or a
    /// [stencil decal without a motif](Self::is_stencil_without_motif).
    #[must_use]
    pub fn is_colourless_decal(&self) -> bool {
        let empty_mesh_decal =
            self.shader.eq_ignore_ascii_case("MeshDecal") && self.textures.is_empty();
        let relief_only = self.string_gen_mask.split('%').any(|flag| flag == "DECAL")
            && self
                .public_param("DecalDiffuseOpacity")
                .is_some_and(|opacity| opacity[0] <= 0.0);
        empty_mesh_decal
            || relief_only
            || self.is_relief_decal()
            || self.is_render_to_texture_decal()
            || self.is_stencil_without_motif()
    }

    /// Returns whether the engine draws nothing with the material, or nothing a plain glTF
    /// material can draw: its flags or shader say so, it is fully transparent, it is an
    /// [`Eye` overlay](Self::is_eye_overlay) or a [colourless decal](Self::is_colourless_decal).
    #[must_use]
    pub fn hidden(&self) -> bool {
        self.flags.no_draw()
            || self.shader.eq_ignore_ascii_case("NoDraw")
            || self.opacity <= 0.0
            || self.is_eye_overlay()
            || self.is_colourless_decal()
    }
}

/// The materials of one `.mtl` file.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MaterialLibrary {
    /// The path the model gave, below `Data` without `.mtl`.
    pub path: String,
    /// The tint palette the library names as its default, a Record path.
    pub default_palette: Option<String>,
    /// Whether the file is one material that serves every material ID, rather than a list of
    /// sub-materials.
    pub single: bool,
    /// How many entries of the sub-material list are no `Material` element and read as
    /// [`Material::engine_default`] so that the IDs after them keep their position.
    pub unknown_entries: usize,
    /// The paths of the layer files of the materials that are missing or unreadable, as the
    /// layers name them; their layers keep no [`LayerSurface`]. Set by
    /// [`MaterialStatus::load`], empty after [`Self::parse`] and a plain read.
    pub unresolved_layers: Vec<String>,
    /// The sub-materials in material ID order; one material when [`Self::single`].
    pub materials: Vec<Material>,
}

impl MaterialLibrary {
    /// Reads the `.mtl` file `bytes`, text XML or `CryXml`, of the library `path`.
    ///
    /// A file with a `SubMaterials` element is a list: the material ID is the position of the
    /// sub-material, and an entry that is no `Material` element counts as a position and reads
    /// as the engine's default material. A file without is one material that serves every ID. Numbers that are
    /// missing, malformed or not finite read as white diffuse, a dim dielectric specular, no
    /// emission, full opacity and a shininess of 10.
    ///
    /// # Errors
    ///
    /// Returns [`MaterialError::Xml`] when the file is no XML or `CryXml`,
    /// [`MaterialError::NotMaterial`] when its root is no `Material`, and
    /// [`MaterialError::TooMany`] when it holds more sub-materials or texture slots than the
    /// limits allow.
    pub fn parse(path: &str, bytes: &[u8]) -> Result<Self, MaterialError> {
        parse_library(path, bytes)
    }

    /// Returns the material of `id`, or `None` when the library has no sub-material at that
    /// position and the engine falls back to its default material.
    #[must_use]
    pub fn material(&self, id: u16) -> Option<&Material> {
        if self.single {
            self.materials.first()
        } else {
            self.materials.get(usize::from(id))
        }
    }
}

/// What reading a model's material library came to.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum MaterialStatus {
    /// The model names no material library.
    Unnamed,
    /// The library the model names does not exist.
    Missing {
        /// The path the model gave.
        path: String,
    },
    /// The library exists but cannot be read, or its name is no safe path.
    Unreadable {
        /// The path the model gave.
        path: String,
        /// What is wrong.
        reason: String,
    },
    /// The library was read.
    Loaded(MaterialLibrary),
    /// The model was read without its library, because the caller chooses the library.
    Skipped,
}

impl MaterialStatus {
    /// Fetches and reads the material library `name` through `companions`, as a model does for
    /// the library its `MtlName` chunk names: a bare name is looked for beside the model first,
    /// then below `Data`. `None` is [`Self::Unnamed`]. A library that is missing, unsafe to ask
    /// for or unreadable is a status, not a failure.
    pub fn load(name: Option<&str>, companions: &mut dyn CompanionResolver) -> Self {
        Self::load_with(name, companions, MaterialReading::Drawn, &LayerCache::new())
    }

    /// Reads the library like [`Self::load`] as `reading` says: [`MaterialReading::Plain`] leaves
    /// out the layer files and the diffuse maps that only a caller that draws needs, and
    /// [`MaterialReading::Skipped`] reads nothing. The layer files of a drawn read come from
    /// `cache` when it holds them and stay there, so that the libraries of one model set share
    /// each file.
    pub fn load_with(
        name: Option<&str>,
        companions: &mut dyn CompanionResolver,
        reading: MaterialReading,
        cache: &LayerCache,
    ) -> Self {
        crate::mtl_load::load(name, companions, reading, cache)
    }

    /// Returns the library when it was read.
    #[must_use]
    pub const fn library(&self) -> Option<&MaterialLibrary> {
        match self {
            Self::Loaded(library) => Some(library),
            _ => None,
        }
    }

    /// Returns what went wrong with the layer files of a read library, or `None` when all were
    /// read: the materials of the missing layers keep the colours of their own file.
    #[must_use]
    pub fn layer_problem(&self) -> Option<String> {
        let library = self.library()?;
        let first = library.unresolved_layers.first()?;
        Some(format!(
            "{} layer files of the material library {} are missing or unreadable, e.g. {first}",
            library.unresolved_layers.len(),
            library.path
        ))
    }

    /// Returns what went wrong with the library, or `None` when it was read or the model names
    /// none.
    #[must_use]
    pub fn problem(&self) -> Option<String> {
        match self {
            Self::Missing { path } => Some(format!("the material library {path} does not exist")),
            Self::Unreadable { path, reason } => Some(format!(
                "the material library {path} cannot be read: {reason}"
            )),
            Self::Unnamed | Self::Loaded(_) | Self::Skipped => None,
        }
    }
}
