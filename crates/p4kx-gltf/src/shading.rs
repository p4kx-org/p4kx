// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{LayerSurface, Material, MaterialLayer};

use crate::texture_pixels::metal_share;

const METAL_SPECULAR: f32 = 0.5;
const DARK_DIFFUSE: f32 = 0.2;
const TEXTURE_GLOSS_ROUGHNESS: f32 = 0.5;
const MAX_SHININESS: f32 = 255.0;

const TEXTURE_GLOSS_SHADERS: [&str; 3] = ["HardSurface", "LayerBlend_V2", "Layer"];

/// How a material is shaded without textures: the colours, whether it is a metal and how rough it
/// is.
///
/// The GLB writer and the viewers take their plain colours from here, so that what is seen is
/// what is exported.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Shading {
    /// The base colour, linear, clamped to 0 to 1: the specular colour of a metal, else the
    /// diffuse colour; for a layered material the weighted blend of the layers' colours, each
    /// moving from its tinted diffuse towards its specular colour by its metal share.
    pub colour: [f32; 3],
    /// Whether the surface is a metal.
    pub metal: bool,
    /// The roughness from 0 to 1.
    pub roughness: f32,
    /// The diffuse colour, linear, as the material gives it or its layers blend it.
    pub diffuse: [f32; 3],
    /// The specular colour, linear, as the material gives it or its layers blend it.
    pub specular: [f32; 3],
}

/// Returns how `material` is shaded.
///
/// A material with `MatLayers` whose layer files were read (see
/// [`MaterialStatus::load`](p4kx_model::MaterialStatus::load)) is shaded by its layers: each
/// layer has its own file's diffuse colour times the layer's tint, or the palette entry of its
/// `PaletteTint` with that entry's specular colour and glossiness, all clamped to 0 to 1 first.
/// The layers are weighted by position: the first with `1 - WearBlendBase`, the others with
/// `WearBlendBase` between them. A layer whose file is missing has no weight; when nothing
/// resolved has weight, the material is shaded by its own colours, as is any material without
/// layers.
#[must_use]
pub fn shading(material: &Material) -> Shading {
    layered(material).unwrap_or_else(|| plain(material))
}

fn plain(material: &Material) -> Shading {
    let metal = is_metal(material);
    let colour = if metal {
        material.specular
    } else {
        stencil_colour(material).unwrap_or(material.diffuse)
    };
    Shading {
        colour: colour.map(|channel| channel.clamp(0.0, 1.0)),
        metal,
        roughness: plain_roughness(material),
        diffuse: material.diffuse,
        specular: material.specular,
    }
}

/// Returns the colour of the motif of a stencil decal drawn by its stencil map that takes its own
/// colour: its `StencilDiffuseColor`; a palette tint (which sets the diffuse colour) wins.
fn stencil_colour(material: &Material) -> Option<[f32; 3]> {
    material
        .stencil_motif()
        .and_then(|stencil| stencil.colour)
        .filter(|_| !material.tinted)
}

/// Returns the roughness of `material` from its shininess, or half where gloss comes from
/// textures and layers; a palette gives the glossiness of its paint.
fn plain_roughness(material: &Material) -> f32 {
    let texture_gloss = TEXTURE_GLOSS_SHADERS
        .iter()
        .any(|name| material.shader.eq_ignore_ascii_case(name));
    if texture_gloss && !material.tinted {
        TEXTURE_GLOSS_ROUGHNESS
    } else {
        1.0 - (material.shininess / MAX_SHININESS).clamp(0.0, 1.0)
    }
}

fn luminance([red, green, blue]: [f32; 3]) -> f32 {
    0.0722_f32.mul_add(blue, 0.2126_f32.mul_add(red, 0.7152 * green))
}

/// Metals have a bright specular colour, and either a metal surface type or a dark diffuse colour.
///
/// A palette whose entry has a bright specular colour, such as the bare metal of the Prospector,
/// paints a metal; the usual paints have a specular colour far below the threshold.
pub(crate) fn is_metal(material: &Material) -> bool {
    let named = material
        .surface_type
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("metal"));
    luminance(material.specular) >= METAL_SPECULAR
        && (named || luminance(material.diffuse) < DARK_DIFFUSE)
}

/// What one layer gives without its textures, all colours linear and clamped to 0 to 1.
pub(crate) struct Layer {
    /// The layer's own diffuse colour times its tint; a diffuse texture multiplies it.
    pub(crate) diffuse: [f32; 3],
    pub(crate) specular: [f32; 3],
    colour: [f32; 3],
    /// How metallic the layer is, 0 to 1.
    pub(crate) metal: f32,
    roughness: f32,
}

/// A layer whose file was read, with its position and its share of the material.
pub(crate) struct Shaded {
    pub(crate) at: usize,
    /// The share by position, see [`shading`]; 0 for a layer the wear share does not reach.
    pub(crate) share: f32,
    pub(crate) layer: Layer,
}

fn unit(colour: [f32; 3]) -> [f32; 3] {
    colour.map(|channel| channel.clamp(0.0, 1.0))
}

fn shade(layer: &MaterialLayer, surface: &LayerSurface) -> Layer {
    let (tint, specular, smoothness) = layer.palette_entry.map_or_else(
        || {
            (
                layer.tint(),
                surface.specular,
                1.0 - TEXTURE_GLOSS_ROUGHNESS,
            )
        },
        |entry| (entry.tint, entry.specular, entry.glossiness / MAX_SHININESS),
    );
    let [red, green, blue] = unit(surface.diffuse);
    let [tint_red, tint_green, tint_blue] = unit(tint);
    let diffuse = [red * tint_red, green * tint_green, blue * tint_blue];
    let specular = unit(specular);
    let metal = metal_share(specular);
    let [diffuse_red, diffuse_green, diffuse_blue] = diffuse;
    let [specular_red, specular_green, specular_blue] = specular;
    let towards = |from: f32, to: f32| (to - from).mul_add(metal, from);
    Layer {
        diffuse,
        specular,
        colour: [
            towards(diffuse_red, specular_red),
            towards(diffuse_green, specular_green),
            towards(diffuse_blue, specular_blue),
        ],
        metal,
        roughness: smoothness
            .clamp(0.0, 1.0)
            .mul_add(-layer.gloss_mult().clamp(0.0, 1.0), 1.0),
    }
}
fn mix(into: &mut [f32; 3], from: [f32; 3], share: f32) {
    for (target, value) in into.iter_mut().zip(from) {
        *target = value.mul_add(share, *target);
    }
}

/// Returns the layers of `material` whose files were read, each with its share by position, or
/// `None` when the material is shaded by its own colours.
pub(crate) fn shaded_layers(material: &Material) -> Option<Vec<Shaded>> {
    let layers = &material.layers;
    let shares =
        material.layer_shares(|at| layers.get(at).is_some_and(|layer| layer.surface.is_some()))?;
    Some(
        layers
            .iter()
            .zip(shares)
            .enumerate()
            .filter_map(|(at, (layer, share))| {
                Some(Shaded {
                    at,
                    share,
                    layer: shade(layer, layer.surface.as_ref()?),
                })
            })
            .collect(),
    )
}

fn layered(material: &Material) -> Option<Shading> {
    let shaded = shaded_layers(material)?;
    let mut mixed = Shading {
        colour: [0.0; 3],
        metal: false,
        roughness: 0.0,
        diffuse: [0.0; 3],
        specular: [0.0; 3],
    };
    let mut metal_total = 0.0_f32;
    for Shaded { share, layer, .. } in shaded {
        if share <= 0.0 {
            continue;
        }
        mix(&mut mixed.colour, layer.colour, share);
        mix(&mut mixed.diffuse, layer.diffuse, share);
        mix(&mut mixed.specular, layer.specular, share);
        mixed.roughness = layer.roughness.mul_add(share, mixed.roughness);
        metal_total = layer.metal.mul_add(share, metal_total);
    }
    mixed.colour = mixed.colour.map(|channel| channel.clamp(0.0, 1.0));
    mixed.metal = metal_total >= 0.5;
    Some(mixed)
}

#[cfg(test)]
mod tests {
    use p4kx_model::{Material, MaterialLibrary, TintEntry, TintPalette};

    use super::shading;

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    const MAP: &str = r#"<Texture Map="TexSlot7" File="a/b_stencil.tif"/>"#;
    const PARAMS: &str = r#"StencilDiffuseColor="0.1,0.2,0.3" StencilTintOverride="1""#;

    fn decal(textures: &str) -> Result<Material> {
        let xml = format!(
            r#"<Material><SubMaterials><Material Name="d" Shader="MeshDecal" StringGenMask="%STENCIL_MAP" Diffuse="1,1,1">
            <Textures>{textures}</Textures><PublicParams {PARAMS}/></Material></SubMaterials></Material>"#
        );
        let library = MaterialLibrary::parse("a", xml.as_bytes())?;
        Ok(library.materials.into_iter().next().ok_or("a material")?)
    }

    #[test]
    fn a_stencil_decal_is_drawn_in_its_stencil_colour_and_a_palette_tint_wins() -> Result {
        let material = decal(MAP)?;
        assert_eq!(shading(&material).colour, [0.1, 0.2, 0.3]);

        let entry = TintEntry {
            tint: [0.5, 0.6, 0.7],
            ..TintEntry::default()
        };
        let palette = TintPalette::new("p", [entry; 3], [1.0; 3]);
        let tinted = palette
            .apply(&material)
            .ok_or("the decal takes the palette")?;
        assert_eq!(shading(&tinted).colour, [0.5, 0.6, 0.7]);
        Ok(())
    }

    #[test]
    fn a_decal_drawn_by_its_diffuse_map_keeps_its_own_colour() -> Result {
        let by_diffuse = decal(r#"<Texture Map="TexSlot1" File="a/b_diff.tif"/>"#)?;
        assert_eq!(shading(&by_diffuse).colour, [1.0; 3]);
        Ok(())
    }
}
