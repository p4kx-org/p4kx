// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{Material, MaterialLayer as ModelLayer, PaletteSlot, RawElement, TextureSlot};

use crate::manifest_materials::{
    AlphaMode, Element, MaterialDescription, MaterialLayer, MaterialStencil, MaterialTexture,
    PaletteEntry, Parameter, TextureUse,
};
use crate::roles::classify_in;

const DECAL_SHADERS: [&str; 2] = ["MeshDecal", "Decal"];
const GLASS_SHADERS: [&str; 1] = ["GlassPBR"];
const NO_DRAW_SHADER: &str = "NoDraw";

/// Describes `material` from the library `library` at the sub-material `index` as the file
/// defines it, with no texture ids and no layer materials yet.
pub(crate) fn describe(
    id: String,
    library: &str,
    index: Option<usize>,
    material: &Material,
    default_palette: Option<&str>,
) -> MaterialDescription {
    MaterialDescription {
        id,
        name: material.name.clone(),
        library: library.to_owned(),
        index,
        shader: material.shader.clone(),
        gen_mask: material
            .string_gen_mask
            .split('%')
            .filter(|flag| !flag.is_empty())
            .map(str::to_owned)
            .collect(),
        surface_type: material.surface_type.clone(),
        flags: material.flags.0,
        two_sided: material.flags.two_sided(),
        no_draw: material.flags.no_draw() || material.shader.eq_ignore_ascii_case(NO_DRAW_SHADER),
        diffuse: material.diffuse,
        specular: material.specular,
        emissive: material.emissive,
        shininess: material.shininess,
        opacity: material.opacity,
        alpha_test: material.alpha_test,
        glow: material.glow,
        alpha_mode: alpha_mode(material),
        reads_vertex_colours: material.reads_vertex_colours(),
        layer_shares: layer_shares(material, |_| false),
        stencil: MaterialStencil::of(material),
        palette: material.palette.and_then(entry),
        default_palette: default_palette.map(str::to_owned),
        textures: material
            .textures
            .iter()
            .map(|slot| texture(material, slot))
            .collect(),
        layers: material.layers.iter().map(layer).collect(),
        parameters: parameters(&material.public_params),
        attributes: parameters(&material.other_attributes),
        elements: material.elements.iter().map(element).collect(),
    }
}

/// States that the stencil decal `description` draws only what was written: with no stencil or
/// diffuse texture written (`texture` set) it draws nothing, as the GLB does for a decal whose map
/// cannot be drawn.
pub(crate) fn settle_stencil(description: &mut MaterialDescription) {
    let written = description.textures.iter().any(|slot| {
        matches!(slot.usage, TextureUse::Stencil | TextureUse::Diffuse) && slot.texture.is_some()
    });
    if let Some(stencil) = &mut description.stencil {
        stencil.drawn &= written;
    }
}

/// Returns the share of each layer of `material` (see [`Material::layer_shares`]), all 0 for the
/// material's own colours, empty for a material without layers.
pub(crate) fn layer_shares(material: &Material, resolved: impl Fn(usize) -> bool) -> Vec<f32> {
    material
        .layer_shares(resolved)
        .unwrap_or_else(|| vec![0.0; material.layers.len()])
}

/// Returns the path of a material library as the manifest gives it: `/` as separator, without a
/// leading `Data/` and without the `.mtl` extension.
pub(crate) fn library_path(path: &str) -> String {
    let unified = path.trim().replace('\\', "/");
    let unified = match unified.get(..5) {
        Some(prefix) if prefix.eq_ignore_ascii_case("data/") => {
            unified.get(5..).unwrap_or_default()
        }
        _ => unified.as_str(),
    };
    match unified
        .len()
        .checked_sub(4)
        .and_then(|at| unified.get(at..))
    {
        Some(extension) if extension.eq_ignore_ascii_case(".mtl") => unified
            .get(..unified.len() - 4)
            .unwrap_or_default()
            .to_owned(),
        _ => unified.to_owned(),
    }
}

/// Returns the palette entry of `slot`, `None` for a slot this writer does not know.
const fn entry(slot: PaletteSlot) -> Option<PaletteEntry> {
    match slot {
        PaletteSlot::Primary => Some(PaletteEntry::Primary),
        PaletteSlot::Secondary => Some(PaletteEntry::Secondary),
        PaletteSlot::Tertiary => Some(PaletteEntry::Tertiary),
        PaletteSlot::Glass => Some(PaletteEntry::Glass),
        PaletteSlot::Decal => Some(PaletteEntry::Decal),
        _ => None,
    }
}

fn alpha_mode(material: &Material) -> AlphaMode {
    let named = |names: &[&str]| {
        names
            .iter()
            .any(|name| material.shader.eq_ignore_ascii_case(name))
    };
    if material.alpha_test > 0.0 {
        AlphaMode::Mask
    } else if named(&GLASS_SHADERS)
        || named(&DECAL_SHADERS)
        || material.is_hair_cap()
        || material.opacity < 1.0
    {
        AlphaMode::Blend
    } else {
        AlphaMode::Opaque
    }
}

fn parameters(attributes: &[(String, String)]) -> Vec<Parameter> {
    attributes
        .iter()
        .map(|(name, value)| Parameter {
            name: name.clone(),
            value: value.clone(),
        })
        .collect()
}

fn element(raw: &RawElement) -> Element {
    Element {
        name: raw.name.clone(),
        attributes: parameters(&raw.attributes),
        children: raw.children.iter().map(element).collect(),
    }
}

fn texture(material: &Material, slot: &TextureSlot) -> MaterialTexture {
    MaterialTexture {
        slot: slot.map.clone(),
        file: slot.file.clone(),
        usage: classify_in(material, &slot.map, &slot.file),
        texture: None,
        attributes: parameters(&slot.attributes),
        elements: slot.children.iter().map(element).collect(),
    }
}

fn layer(layer: &ModelLayer) -> MaterialLayer {
    let text = |name: &str| layer.attribute(name).map(str::to_owned);
    MaterialLayer {
        name: text("Name").unwrap_or_default(),
        path: text("Path").unwrap_or_default(),
        sub_material: text("Submtl").filter(|name| !name.is_empty()),
        tint_colour: layer.attribute("TintColor").and_then(colour),
        wear_tint: layer.attribute("WearTint").and_then(colour),
        uv_tiling: layer.attribute("UVTiling").and_then(number),
        gloss_mult: layer.attribute("GlossMult").and_then(number),
        attributes: parameters(&layer.attributes),
        palette: layer.palette_slot().and_then(entry),
        material: None,
    }
}

fn number(value: &str) -> Option<f32> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

fn colour(value: &str) -> Option<[f32; 3]> {
    let mut parts = value.split(',').map(number);
    let colour = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(colour)
}

#[cfg(test)]
mod tests {
    use p4kx_model::MaterialLibrary;

    use super::{colour, describe, library_path, number, settle_stencil};
    use crate::manifest_materials::{MaterialStencil, PaletteEntry, TextureUse};

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    const MAP: &str = r#"<Texture Map="TexSlot7" File="a/logo_stencil.tif"/><Texture Map="TexSlot8" File="a/b_blend.tif"/>"#;

    fn stencil_decal(textures: &str, params: &str) -> Result<crate::MaterialDescription> {
        let xml = format!(
            r#"<Material><SubMaterials><Material Name="d" Shader="MeshDecal" StringGenMask="%VERTDATA%STENCIL_MAP">
            <Textures>{textures}</Textures><PublicParams {params}/></Material></SubMaterials></Material>"#
        );
        let library = MaterialLibrary::parse("a", xml.as_bytes())?;
        let material = library.materials.first().ok_or("a material")?;
        Ok(describe("x0001".to_owned(), "a", Some(0), material, None))
    }

    fn written(description: &mut crate::MaterialDescription) {
        for (at, slot) in description.textures.iter_mut().enumerate() {
            slot.texture = Some(format!("t{:04}", at + 1));
        }
    }

    #[test]
    fn a_stencil_decal_states_its_stencil_map_colour_and_opacity() -> Result {
        let description = stencil_decal(
            MAP,
            r#"StencilDiffuseColor="0.1,0.2,0.3" StencilOpacity="0.5""#,
        )?;
        let usages: Vec<_> = description.textures.iter().map(|slot| slot.usage).collect();
        assert_eq!(usages, [TextureUse::Stencil, TextureUse::Blend]);
        assert_eq!(
            description.stencil,
            Some(MaterialStencil {
                drawn: true,
                colour: Some([0.1, 0.2, 0.3]),
                opacity: 0.5
            })
        );
        assert_eq!(description.palette, None);
        Ok(())
    }

    #[test]
    fn a_tint_override_states_the_primary_entry_or_the_decal_colours() -> Result {
        let one = stencil_decal(MAP, r#"StencilTintOverride="1""#)?;
        assert_eq!(one.palette, Some(PaletteEntry::Primary));
        let two = stencil_decal(MAP, r#"StencilTintOverride="2""#)?;
        assert_eq!(two.palette, Some(PaletteEntry::Decal));
        Ok(())
    }

    #[test]
    fn a_decal_drawn_by_its_diffuse_map_states_no_stencil_colour_and_no_palette() -> Result {
        let diffuse = r#"<Texture Map="TexSlot1" File="a/b_diff.tif"/>"#;
        let params =
            r#"StencilDiffuseColor="0.1,0.2,0.3" StencilOpacity="0.5" StencilTintOverride="1""#;
        let description = stencil_decal(diffuse, params)?;
        assert_eq!(
            description.stencil,
            Some(MaterialStencil {
                drawn: true,
                colour: None,
                opacity: 1.0
            })
        );
        assert_eq!(description.palette, None);
        Ok(())
    }

    #[test]
    fn a_decal_without_a_motif_is_not_drawn_and_takes_no_palette() -> Result {
        let placeholder = r#"<Texture Map="TexSlot7" File="$TintPaletteDecal"/>"#;
        let description = stencil_decal(placeholder, r#"StencilTintOverride="1""#)?;
        assert_eq!(
            description.stencil.map(|stencil| stencil.drawn),
            Some(false)
        );
        assert_eq!(description.palette, None);
        Ok(())
    }

    #[test]
    fn a_decal_whose_maps_could_not_be_written_is_not_drawn() -> Result {
        let mut description = stencil_decal(MAP, "")?;
        settle_stencil(&mut description);
        assert_eq!(
            description.stencil.map(|stencil| stencil.drawn),
            Some(false)
        );

        written(&mut description);
        description.stencil = stencil_decal(MAP, "")?.stencil;
        settle_stencil(&mut description);
        assert_eq!(description.stencil.map(|stencil| stencil.drawn), Some(true));

        let mut by_diffuse = stencil_decal(r#"<Texture Map="TexSlot1" File="a/b_diff.tif"/>"#, "")?;
        settle_stencil(&mut by_diffuse);
        assert_eq!(by_diffuse.stencil.map(|stencil| stencil.drawn), Some(false));
        Ok(())
    }

    #[test]
    fn a_library_path_has_no_data_prefix_extension_or_backslash() {
        assert_eq!(library_path("Data\\A\\b.MTL"), "A/b");
        assert_eq!(library_path("a/b"), "a/b");
        assert_eq!(library_path("a/b.mtl"), "a/b");
    }

    #[test]
    fn colours_need_three_finite_numbers() {
        assert_eq!(colour("0.5, 1,0"), Some([0.5, 1.0, 0.0]));
        assert_eq!(colour("1,2"), None);
        assert_eq!(colour("1,2,3,4"), None);
        assert_eq!(colour("1,x,3"), None);
        assert_eq!(number("inf"), None);
        assert_eq!(number(" 2 "), Some(2.0));
    }
}
