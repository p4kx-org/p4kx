// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The one rule for stencil decals: which texture is the stencil map and how the stencil is
//! coloured.

use crate::mtl_texture::DIFFUSE_SLOT;
use crate::{Material, TextureSlot};

/// The material slot of the stencil map of a stencil decal.
pub const STENCIL_SLOT: &str = "TexSlot7";

const STENCIL_FLAG: &str = "STENCIL_MAP";
const MESH_DECAL: &str = "MeshDecal";
const COLOUR: &str = "StencilDiffuseColor";
const OPACITY: &str = "StencilOpacity";
const TINT_OVERRIDE: &str = "StencilTintOverride";

/// How a stencil decal is coloured: the parameters of its shader, which colour the motif the
/// stencil map cuts out.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Stencil {
    /// The colour of the motif (`StencilDiffuseColor`), linear; `None` when the material sets none
    /// or it is malformed, and its diffuse colour counts.
    pub colour: Option<[f32; 3]>,
    /// The opacity of the motif (`StencilOpacity`), 1 when the material sets none; it multiplies the
    /// material's opacity.
    pub opacity: f32,
    /// Whether the decal takes the tint of the palette in place of its own colour
    /// (`StencilTintOverride` above 0). The material then takes the primary entry
    /// ([`Material::palette`]); the palette's own decal colours are not read.
    pub palette_tint: bool,
}

impl Material {
    /// Returns whether the material is a stencil decal: a `MeshDecal` with `STENCIL_MAP` in its
    /// generation mask, whose [stencil map](Self::stencil_texture) cuts the motif out.
    #[must_use]
    pub fn is_stencil_decal(&self) -> bool {
        self.shader.eq_ignore_ascii_case(MESH_DECAL)
            && self
                .string_gen_mask
                .split('%')
                .any(|flag| flag == STENCIL_FLAG)
    }

    /// Returns the stencil parameters of a [stencil decal](Self::is_stencil_decal), `None` for any
    /// other material.
    ///
    /// The GLB writer and the package take the colour and opacity of a stencil from here, so that
    /// what the add-on draws is what is exported.
    #[must_use]
    pub fn stencil(&self) -> Option<Stencil> {
        self.is_stencil_decal().then(|| Stencil {
            colour: self.public_colour(COLOUR),
            opacity: self
                .public_param(OPACITY)
                .map_or(1.0, |opacity| opacity[0].clamp(0.0, 1.0)),
            palette_tint: self
                .public_param(TINT_OVERRIDE)
                .is_some_and(|tint| tint[0] > 0.0),
        })
    }

    /// Returns the stencil map a [stencil decal](Self::is_stencil_decal) is drawn by: the file of
    /// `TexSlot7`, whose red channel is the opacity of the motif. `None` for any other material,
    /// for a decal whose slot holds an engine placeholder such as `$TintPaletteDecal`, which the
    /// game fills with a livery's emblem at run time, and for a decal with a diffuse map of its
    /// own, which is drawn by that map.
    #[must_use]
    pub fn stencil_texture(&self) -> Option<&TextureSlot> {
        if !self.is_stencil_decal() || self.file_in(DIFFUSE_SLOT).is_some() {
            return None;
        }
        self.file_in(STENCIL_SLOT)
    }

    /// Returns whether the material is a stencil decal that has nothing to draw: neither a stencil
    /// map nor a diffuse map. The motif is a livery's emblem the game fills in; drawn as a surface
    /// it would be a solid quad.
    #[must_use]
    pub fn is_stencil_without_motif(&self) -> bool {
        self.is_stencil_decal()
            && self.file_in(DIFFUSE_SLOT).is_none()
            && self.file_in(STENCIL_SLOT).is_none()
    }

    /// Returns the slot `slot` when it names a file rather than a placeholder or nothing.
    fn file_in(&self, slot: &str) -> Option<&TextureSlot> {
        self.textures.iter().find(|texture| {
            let file = texture.file.trim();
            texture.map.eq_ignore_ascii_case(slot) && !file.is_empty() && !file.starts_with('$')
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{Material, MaterialLibrary};

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    fn decal(shader: &str, mask: &str, textures: &str, params: &str) -> Result<Material> {
        let xml = format!(
            r#"<Material><SubMaterials><Material Name="d" Shader="{shader}" StringGenMask="{mask}">
            <Textures>{textures}</Textures><PublicParams {params}/></Material></SubMaterials></Material>"#
        );
        let library = MaterialLibrary::parse("a", xml.as_bytes())?;
        Ok(library.materials.into_iter().next().ok_or("a material")?)
    }

    #[test]
    fn only_a_mesh_decal_with_the_flag_is_a_stencil_decal() -> Result {
        assert!(decal("MeshDecal", "%STENCIL_MAP%USE_DAMAGE_MAP", "", "")?.is_stencil_decal());
        assert!(decal("meshdecal", "STENCIL_MAP", "", "")?.is_stencil_decal());
        assert!(!decal("MeshDecal", "%NOT_STENCIL_MAP", "", "")?.is_stencil_decal());
        assert!(!decal("HardSurface", "%STENCIL_MAP", "", "")?.is_stencil_decal());
        Ok(())
    }

    #[test]
    fn the_stencil_takes_its_colour_opacity_and_tint_from_the_parameters() -> Result {
        let material = decal(
            "MeshDecal",
            "%STENCIL_MAP",
            "",
            r#"StencilDiffuseColor="0.1,0.2,0.3" StencilOpacity="0.5" StencilTintOverride="1""#,
        )?;
        let stencil = material.stencil().ok_or("a stencil")?;
        assert_eq!(stencil.colour, Some([0.1, 0.2, 0.3]));
        assert!((stencil.opacity - 0.5).abs() < f32::EPSILON);
        assert!(stencil.palette_tint);

        let bare = decal("MeshDecal", "%STENCIL_MAP", "", "")?
            .stencil()
            .ok_or("a stencil")?;
        assert_eq!(bare.colour, None);
        assert!((bare.opacity - 1.0).abs() < f32::EPSILON);
        assert!(!bare.palette_tint);
        assert_eq!(decal("MeshDecal", "", "", "")?.stencil(), None);
        Ok(())
    }

    #[test]
    fn the_stencil_map_is_the_file_of_the_stencil_slot() -> Result {
        let textures = r#"<Texture Map="TexSlot7" File="a/logo_stencil.tif"/>"#;
        let material = decal("MeshDecal", "%STENCIL_MAP", textures, "")?;
        assert_eq!(
            material.stencil_texture().map(|slot| slot.file.as_str()),
            Some("a/logo_stencil.tif")
        );
        assert!(!material.is_stencil_without_motif());
        Ok(())
    }

    #[test]
    fn a_decal_with_a_diffuse_map_is_drawn_by_it() -> Result {
        let material = decal(
            "MeshDecal",
            "%STENCIL_MAP",
            r#"<Texture Map="TexSlot1" File="a/b_diff.tif"/><Texture Map="TexSlot7" File="a/b_stencil.tif"/>"#,
            "",
        )?;
        assert!(material.stencil_texture().is_none());
        assert!(!material.is_stencil_without_motif());
        Ok(())
    }

    #[test]
    fn a_placeholder_or_a_misplaced_map_leaves_nothing_to_draw() -> Result {
        for textures in [
            r#"<Texture Map="TexSlot7" File="$TintPaletteDecal"/>"#,
            r#"<Texture Map="TexSlot1" File="$RenderToTexture"/><Texture Map="TexSlot7" File=" "/>"#,
            r#"<Texture Map="TexSlot8" File="a/b_stencil.tif"/>"#,
            "",
        ] {
            let material = decal("MeshDecal", "%STENCIL_MAP", textures, "")?;
            assert!(material.stencil_texture().is_none(), "{textures}");
            assert!(material.is_stencil_without_motif(), "{textures}");
        }
        Ok(())
    }

    #[test]
    fn a_decal_without_a_motif_is_hidden_and_one_with_a_stencil_map_is_not() -> Result {
        let placeholder = r#"<Texture Map="TexSlot7" File="$TintPaletteDecal"/>"#;
        assert!(decal("MeshDecal", "%STENCIL_MAP", placeholder, "")?.hidden());
        let map = r#"<Texture Map="TexSlot7" File="a/b_stencil.tif"/>"#;
        assert!(!decal("MeshDecal", "%STENCIL_MAP", map, "")?.hidden());
        assert!(!decal("MeshDecal", "", placeholder, "")?.hidden());
        Ok(())
    }

    #[test]
    fn a_tint_override_takes_the_primary_palette_entry() -> Result {
        use crate::PaletteSlot;
        let with = |params: &str| decal("MeshDecal", "%STENCIL_MAP", "", params);
        assert_eq!(
            with(r#"StencilTintOverride="1""#)?.palette,
            Some(PaletteSlot::Primary)
        );
        assert_eq!(with(r#"StencilTintOverride="0""#)?.palette, None);
        assert_eq!(with("")?.palette, None);
        Ok(())
    }

    #[test]
    fn the_stencil_slot_is_the_stencil_role_and_only_for_a_stencil_decal() -> Result {
        use crate::TextureRole;
        let xml = r#"<Texture Map="TexSlot7" File="a/b_a.tif"/>"#;
        let stencil = decal("MeshDecal", "%STENCIL_MAP", xml, "")?;
        assert_eq!(
            stencil.texture_role("TexSlot7", "a/b_a.tif"),
            TextureRole::Stencil
        );
        assert_eq!(
            stencil.texture_role("TexSlot8", "a/b_a.tif"),
            TextureRole::Other
        );
        let plain = decal("MeshDecal", "", xml, "")?;
        assert_eq!(
            plain.texture_role("TexSlot7", "a/b_a.tif"),
            TextureRole::Other
        );
        Ok(())
    }
}
