// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{
    DIFFUSE_SLOT, Material, TextureSlot, is_diffuse_map, texture_suffix_lowercase as suffix,
};

use crate::material_alpha::is_hair_cards;
use crate::texture_report::TextureRole;

const DATA_PREFIX: &str = "data/";
const SOURCE_EXTENSIONS: [&str; 7] = ["tif", "tiff", "tga", "png", "bmp", "jpg", "dds"];
const NORMAL_SLOTS: [&str; 2] = ["TexSlot2", "TexSlot3"];
const SPECULAR_SLOT: &str = "TexSlot4";
const NORMAL_GLOSS_SUFFIX: &str = "ddna";
const NORMAL_SUFFIX: &str = "ddn";
const SPECULAR_SUFFIX: &str = "spec";
const OPACITY_SUFFIX: &str = "opac";
const SCALP_SUFFIX: &str = "scalp";

/// The texture files of a material that map to glTF's PBR textures, as canonical paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Sources {
    pub(crate) diffuse: Option<Source>,
    pub(crate) normal: Option<Source>,
    pub(crate) specular: Option<Source>,
    /// The files that would have been used but have no safe path, with their role.
    pub(crate) refused: Vec<(TextureRole, String)>,
}

/// One texture file of a material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
    /// The path as the loader takes it: below `Data`, `/` as separator, `.dds`.
    pub(crate) path: String,
    /// The path as the material file gives it, for reports.
    pub(crate) file: String,
    /// Whether the texture carries the smoothness in its alpha channel.
    pub(crate) gloss: bool,
    /// Whether the texture is an opacity map standing in for the diffuse map: its red channel is
    /// the opacity and the colour comes from the material alone.
    pub(crate) opacity: bool,
}

impl Source {
    /// Returns the path folded to lower case, the identity of the texture.
    pub(crate) fn identity(&self) -> String {
        self.path.to_ascii_lowercase()
    }
}

/// Returns the texture files of `material` that glTF can use: the diffuse map of `TexSlot1`, the
/// normal map of `TexSlot2` or `TexSlot3` whose name ends in `_ddna` (with the smoothness in its
/// alpha channel) or `_ddn`, and the specular map of `TexSlot4` ending in `_spec`. Engine
/// placeholders such as `$RenderToTexture` are no files; a file whose path is unsafe is refused.
pub(crate) fn sources(material: &Material) -> Sources {
    sources_with(material, false)
}

/// Returns the texture files of `material` like [`sources`]; with `opacity_maps`, a material
/// without a diffuse map whose `TexSlot1` ends in `_opac`, such as hair cards, has that opacity
/// map in place of the diffuse map, see [`Source::opacity`].
///
/// A hair cap ([`Material::is_hair_cap`]) always has its `_scalp` density map in that place. With
/// `opacity_maps`, a stencil decal has its [stencil map](Material::stencil_texture) there too,
/// red channel as the opacity.
pub(crate) fn sources_with(material: &Material, opacity_maps: bool) -> Sources {
    if material.is_hair_cap() {
        collect(&material.textures, Some(SCALP_SUFFIX))
    } else if opacity_maps && let Some(stencil) = material.stencil_texture() {
        stencil_sources(&material.textures, stencil)
    } else {
        sources_of(&material.textures, opacity_maps)
    }
}

/// Returns whether `material` is drawn by an opacity map in place of a diffuse map: hair cards and
/// a stencil decal with a stencil map.
pub(crate) fn draws_by_opacity_map(material: &Material) -> bool {
    is_hair_cards(material) || material.stencil_texture().is_some()
}

/// Returns the texture files of the slots `textures` with the stencil map `stencil` as the opacity
/// map in place of the diffuse map.
fn stencil_sources(textures: &[TextureSlot], stencil: &TextureSlot) -> Sources {
    let mut found = collect(textures, None);
    match canonical(&stencil.file) {
        Some(path) => {
            found.diffuse = Some(Source {
                path,
                file: stencil.file.trim().to_owned(),
                gloss: false,
                opacity: true,
            });
        }
        None => found
            .refused
            .push((TextureRole::Diffuse, stencil.file.trim().to_owned())),
    }
    found
}

/// Returns the texture files of the slots `textures` like [`sources_with`].
pub(crate) fn sources_of(textures: &[TextureSlot], opacity_maps: bool) -> Sources {
    collect(textures, opacity_maps.then_some(OPACITY_SUFFIX))
}

fn collect(textures: &[TextureSlot], opacity_suffix: Option<&str>) -> Sources {
    let pick = |slots: &[&str], wanted: &dyn Fn(&str) -> bool| {
        slots.iter().find_map(|slot| {
            textures.iter().find_map(|texture| {
                let file = texture.file.as_str();
                (texture.map == *slot
                    && !file.trim_start().starts_with('$')
                    && wanted(&suffix(file)))
                .then_some(file)
            })
        })
    };
    let mut found = Sources::default();
    let mut resolve = |role: TextureRole, file: Option<&str>, gloss: bool, opacity: bool| {
        let file = file?;
        if let Some(path) = canonical(file) {
            Some(Source {
                path,
                file: file.to_owned(),
                gloss,
                opacity,
            })
        } else {
            found.refused.push((role, file.to_owned()));
            None
        }
    };
    let diffuse = textures
        .iter()
        .find(|texture| is_diffuse_map(&texture.map, &texture.file))
        .map(|texture| texture.file.as_str());
    let normal = pick(&NORMAL_SLOTS, &|name| {
        name == NORMAL_GLOSS_SUFFIX || name == NORMAL_SUFFIX
    });
    let specular = pick(&[SPECULAR_SLOT], &|name| name == SPECULAR_SUFFIX);
    let gloss = normal.is_some_and(|file| suffix(file) == NORMAL_GLOSS_SUFFIX);
    let opacity = opacity_suffix
        .and_then(|wanted| pick(&[DIFFUSE_SLOT], &|name| name == wanted))
        .filter(|_| diffuse.is_none());
    let diffuse = match opacity {
        Some(file) => resolve(TextureRole::Diffuse, Some(file), false, true),
        None => resolve(TextureRole::Diffuse, diffuse, false, false),
    };
    let normal = resolve(TextureRole::Normal, normal, gloss, false);
    let specular = resolve(TextureRole::Specular, specular, false, false);
    found.diffuse = diffuse;
    found.normal = normal;
    found.specular = specular;
    found
}

/// Returns the path of the diffuse map of `material` as a [`TextureLoader`](crate::TextureLoader)
/// takes it, the texture the GLB writer draws the base colour from, or `None` when the material
/// has none a loader can use.
#[must_use]
pub fn diffuse_texture_path(material: &Material) -> Option<String> {
    sources(material).diffuse.map(|source| source.path)
}

/// Returns the path of the texture `file` as a loader takes it, or `None` for a placeholder such
/// as `$RenderToTexture` and for a path that is empty, absolute, drive-qualified, has control
/// characters or an empty, `.` or `..` component.
///
/// The extension of a source image (`.tif`, `.tga`, …) becomes `.dds`, the form the Archive holds.
pub(crate) fn canonical(file: &str) -> Option<String> {
    let unified = file.trim().replace('\\', "/");
    let relative = match unified.get(..DATA_PREFIX.len()) {
        Some(prefix) if prefix.eq_ignore_ascii_case(DATA_PREFIX) => {
            unified.get(DATA_PREFIX.len()..)?
        }
        _ => &unified,
    };
    if relative.is_empty()
        || relative.starts_with(['$', '/'])
        || relative.contains(':')
        || relative.chars().any(char::is_control)
    {
        return None;
    }
    let mut components: Vec<&str> = relative.split('/').collect();
    if components
        .iter()
        .any(|part| matches!(*part, "" | "." | ".."))
    {
        return None;
    }
    let name = components.pop()?;
    let stem = match name.rsplit_once('.') {
        Some((stem, extension))
            if !stem.is_empty()
                && SOURCE_EXTENSIONS
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(extension)) =>
        {
            stem
        }
        _ => name,
    };
    let mut path = String::with_capacity(relative.len() + ".dds".len());
    for part in components {
        path.push_str(part);
        path.push('/');
    }
    path.push_str(stem);
    path.push_str(".dds");
    Some(path)
}

#[cfg(test)]
mod tests {
    use p4kx_model::MaterialLibrary;

    use super::{canonical, sources};

    #[test]
    fn a_packed_detail_map_in_the_diffuse_slot_is_no_diffuse_map()
    -> Result<(), Box<dyn std::error::Error>> {
        let xml = |file: &str| {
            format!(
                r#"<Material><SubMaterials><Material Name="m" Shader="HardSurface"><Textures><Texture Map="TexSlot1" File="{file}"/></Textures></Material></SubMaterials></Material>"#
            )
        };
        for (file, expected) in [
            ("a/aegs_ext_detail_01.tif", None),
            ("a/hull_detail.tif", None),
            ("a/hull_diff_01.tif", Some("a/hull_diff_01.dds")),
            ("a/hull.tif", Some("a/hull.dds")),
        ] {
            let library = MaterialLibrary::parse("a", xml(file).as_bytes())?;
            let material = library.materials.first().ok_or("a material")?;
            let found = sources(material).diffuse.map(|source| source.path);
            assert_eq!(found.as_deref(), expected, "{file}");
        }
        Ok(())
    }

    #[test]
    fn source_images_become_dds_below_data() {
        for (file, expected) in [
            ("Objects/a/b_diff.tif", "Objects/a/b_diff.dds"),
            ("Objects\\a\\b_diff.TIF", "Objects/a/b_diff.dds"),
            ("Data/Objects/a/b_diff.dds", "Objects/a/b_diff.dds"),
            ("data\\a\\b", "a/b.dds"),
            ("a/b.v2", "a/b.v2.dds"),
            ("b.tga", "b.dds"),
        ] {
            assert_eq!(canonical(file).as_deref(), Some(expected), "{file}");
        }
    }

    #[test]
    fn placeholders_and_unsafe_paths_are_refused() {
        for file in [
            "",
            "$RenderToTexture",
            "/etc/x.dds",
            "C:/x.dds",
            "C:\\x.dds",
            "../x.dds",
            "a/../x.dds",
            "a/./x.dds",
            "a//x.dds",
            "a/x\u{0}.dds",
            "a/",
            "Data/",
        ] {
            assert_eq!(canonical(file), None, "{file:?}");
        }
    }
}
