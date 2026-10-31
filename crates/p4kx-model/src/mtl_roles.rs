// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Material;
use crate::mtl_texture::{DIFFUSE_SLOT, texture_suffix_lowercase as suffix};

/// What a texture of a material is used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextureRole {
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
    /// The stencil map of a stencil decal: its red channel is the opacity of the motif.
    Stencil,
    /// Any other map; the slot and file say more.
    Other,
}

impl TextureRole {
    /// Returns what the texture `file` in the slot `slot` is used for, by the suffix of the file
    /// name (`a/b_ddna.tif` is [`Self::Normal`]) and, for a file without a known suffix, the slot:
    /// `TexSlot1` is the diffuse map.
    #[must_use]
    pub fn of(slot: &str, file: &str) -> Self {
        match suffix(file).as_str() {
            "ddna" | "ddn" => Self::Normal,
            "spec" => Self::Specular,
            "mask" | "opac" => Self::Mask,
            "blend" => Self::Blend,
            "detail" => Self::Detail,
            "wear" => Self::Wear,
            "ao" => Self::Occlusion,
            "displ" | "disp" => Self::Displacement,
            "gloss" => Self::Gloss,
            "id" | "scalp" | "dir" | "dirm" | "hal" => Self::Other,
            _ if slot.eq_ignore_ascii_case(DIFFUSE_SLOT) => Self::Diffuse,
            _ => Self::Other,
        }
    }
}

/// Returns whether `file` in the slot `slot` is a diffuse map: a file, not an engine placeholder
/// such as `$RenderToTexture`, whose [role](TextureRole::of) is [`TextureRole::Diffuse`].
#[must_use]
pub fn is_diffuse_map(slot: &str, file: &str) -> bool {
    let file = file.trim();
    !file.is_empty()
        && !file.starts_with('$')
        && TextureRole::of(slot, file) == TextureRole::Diffuse
}

impl Material {
    /// Returns what the texture `file` in the slot `slot` of the material is used for like
    /// [`TextureRole::of`]; the `_scalp` map of a hair cap is its density mask and the map in the
    /// stencil slot of a [stencil decal](Material::stencil_texture) is its stencil.
    #[must_use]
    pub fn texture_role(&self, slot: &str, file: &str) -> TextureRole {
        if self.is_hair_cap() && suffix(file) == "scalp" {
            TextureRole::Mask
        } else if self.stencil_texture().is_some_and(|texture| {
            texture.map.eq_ignore_ascii_case(slot) && texture.file.trim() == file.trim()
        }) {
            TextureRole::Stencil
        } else {
            TextureRole::of(slot, file)
        }
    }

    /// Returns whether a texture slot holds a diffuse map: a file, not an engine placeholder, whose
    /// [role](TextureRole::of) is [`TextureRole::Diffuse`]. The GLB writer and the stencil rule
    /// share this test.
    #[must_use]
    pub fn has_diffuse_map(&self) -> bool {
        self.textures
            .iter()
            .any(|texture| is_diffuse_map(&texture.map, &texture.file))
    }

    /// Returns the roles of the material's textures that name a file, leaving out an engine
    /// placeholder such as `$RenderToTexture`, which no file backs.
    pub(crate) fn file_roles(&self) -> impl Iterator<Item = TextureRole> {
        self.textures
            .iter()
            .filter(|texture| {
                let file = texture.file.trim();
                !file.is_empty() && !file.starts_with('$')
            })
            .map(|texture| self.texture_role(&texture.map, texture.file.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::TextureRole;
    use crate::MaterialLibrary;

    #[test]
    fn the_file_suffix_decides_and_the_diffuse_slot_is_the_fallback() {
        for (slot, file, expected) in [
            ("TexSlot1", "a/b_diff.tif", TextureRole::Diffuse),
            ("TexSlot1", "a/b.tif", TextureRole::Diffuse),
            ("TexSlot2", "a/b_ddna.tif", TextureRole::Normal),
            ("TexSlot3", "a\\b_DDN.tif", TextureRole::Normal),
            ("TexSlot6", "a/b_spec.tif", TextureRole::Specular),
            ("TexSlot8", "a/b_blend.tif", TextureRole::Blend),
            ("TexSlot7", "a/b_detail.tif", TextureRole::Detail),
            ("TexSlot1", "a/aegs_ext_detail_01.tif", TextureRole::Detail),
            ("TexSlot1", "a/b_diff_01.tif", TextureRole::Diffuse),
            ("TexSlot6", "a/b_wear.tif", TextureRole::Wear),
            ("TexSlot1", "a/b_wear.tif", TextureRole::Wear),
            ("TexSlot8", "a/b_displ.tif", TextureRole::Displacement),
            ("TexSlot9", "a/b.tif", TextureRole::Other),
            ("TexSlot1", "a/b_id.tif", TextureRole::Other),
            ("TexSlot1", "a/b_scalp.tif", TextureRole::Other),
        ] {
            assert_eq!(TextureRole::of(slot, file), expected, "{slot} {file}");
        }
    }

    #[test]
    fn a_scalp_map_is_a_mask_only_for_a_hair_cap() -> Result<(), Box<dyn std::error::Error>> {
        let xml = |mask: &str| {
            format!(
                r#"<Material><SubMaterials><Material Name="m" Shader="HairPBR" StringGenMask="{mask}"/></SubMaterials></Material>"#
            )
        };
        for (mask, expected) in [
            ("%HAIR_CAP", TextureRole::Mask),
            ("%HAIR_CARDS", TextureRole::Other),
        ] {
            let library = MaterialLibrary::parse("a", xml(mask).as_bytes())?;
            let material = library.materials.first().ok_or("a material")?;
            assert_eq!(
                material.texture_role("TexSlot1", "a/b_scalp.tif"),
                expected,
                "{mask}"
            );
        }
        Ok(())
    }
}
