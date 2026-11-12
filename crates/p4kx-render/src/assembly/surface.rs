// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::model::Material;
use p4kx_extraction::{base_colour_factor, diffuse_texture_path, layered_base_key};
use p4kx_settings::ModelMaterials;

use super::build::SharedMesh;
use super::pieces::Piece;

/// The start of the key a layered material's blended image has in place of a texture path.
pub(super) const LAYERED_PREFIX: &str = "layered:";

const GLASS_ALPHA: f32 = 0.3;
const DECAL_SHADERS: [&str; 2] = ["MeshDecal", "Decal"];

/// The material library, palette and material ID that decide how a range is drawn.
pub(super) type SurfaceKey = (usize, usize, u16);

/// How one material ID of a part is drawn.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Surface {
    pub(super) colour: [f32; 4],
    pub(super) translucent: bool,
    pub(super) library_slot: Option<usize>,
    pub(super) texture: Option<String>,
    /// The layered material whose blended image is the texture, which a path does not name.
    pub(super) layered: Option<Material>,
}

fn is_one_of(shader: &str, names: &[&str]) -> bool {
    names.iter().any(|name| shader.eq_ignore_ascii_case(name))
}

fn address<T>(value: &T) -> usize {
    std::ptr::from_ref(value) as usize
}

pub(super) fn surface_key(part: &Piece<'_>, id: u16) -> SurfaceKey {
    (address(part.materials), part.palette.map_or(0, address), id)
}

/// Resolves the material `id` of `part` the way the GLB export picks it: the sub-material of
/// its library, else the engine's default, with the part's tint palette applied. Returns `None`
/// when nothing is drawn with it: a hidden material or a decal, whose shape only a texture gives.
pub(super) fn resolve(part: &Piece<'_>, id: u16, mode: ModelMaterials) -> Option<Surface> {
    let fallback = Material::engine_default();
    let library = part.materials.library();
    let selected = library.and_then(|library| {
        let slot = if library.single { 0 } else { usize::from(id) };
        Some((slot, library.materials.get(slot)?))
    });
    let found = selected.map_or(&fallback, |(_, material)| material);
    if found.hidden() || is_one_of(&found.shader, &DECAL_SHADERS) {
        return None;
    }
    let tinted = part.palette.and_then(|palette| palette.apply(found));
    let mut surface = surface_of(tinted.as_ref().unwrap_or(found), mode)?;
    surface.library_slot = selected.map(|(slot, _)| slot);
    Some(surface)
}

/// Returns how `material` is drawn: the base colour the GLB writer uses (the specular colour of a
/// metal, else the diffuse colour), its opacity, and in the textures mode its diffuse texture.
pub(super) fn surface_of(material: &Material, mode: ModelMaterials) -> Option<Surface> {
    if is_one_of(&material.shader, &DECAL_SHADERS) {
        return None;
    }
    let opacity = material.opacity.clamp(0.0, 1.0);
    let glass = material.is_glass();
    let (alpha, translucent) = if glass {
        (opacity.min(GLASS_ALPHA), true)
    } else if opacity < 1.0 && material.alpha_test <= 0.0 {
        (opacity, true)
    } else {
        (1.0, false)
    };
    let textured = mode == ModelMaterials::Textures;
    let blend = textured.then(|| layered_base_key(material)).flatten();
    let [red, green, blue] = if blend.is_some() {
        [1.0; 3]
    } else {
        base_colour_factor(material)
    };
    Some(Surface {
        colour: [red, green, blue, alpha],
        translucent,
        library_slot: None,
        layered: blend.is_some().then(|| material.clone()),
        texture: blend
            .map(|key| format!("{LAYERED_PREFIX}{key}"))
            .or_else(|| textured.then(|| diffuse_texture_path(material)).flatten()),
    })
}

/// The surfaces of the material ranges of a scene, the distinct texture keys they want in order,
/// and the layered materials whose blended images some keys stand for.
pub(super) struct SceneSurfaces {
    pub(super) surfaces: HashMap<SurfaceKey, Option<Surface>>,
    pub(super) wanted: Vec<String>,
    pub(super) layered: HashMap<String, Material>,
}

/// Resolves the surface of every material range the parts use, once per library, palette and
/// material ID, and lists the distinct texture keys they want.
pub(super) fn surfaces_of(
    pieces: &[Piece<'_>],
    model_of: &[Option<usize>],
    meshes: &[Option<SharedMesh>],
    mode: ModelMaterials,
) -> SceneSurfaces {
    let mut surfaces = HashMap::new();
    let mut layered: HashMap<String, Material> = HashMap::new();
    let mut wanted: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (at, part) in pieces.iter().enumerate() {
        let mesh = model_of
            .get(at)
            .copied()
            .flatten()
            .and_then(|id| meshes.get(id))
            .and_then(Option::as_ref);
        let Some(mesh) = mesh else {
            continue;
        };
        for range in &mesh.ranges {
            surfaces
                .entry(surface_key(part, range.material))
                .or_insert_with(|| {
                    let surface = resolve(part, range.material, mode);
                    if let Some(path) = surface.as_ref().and_then(|s| s.texture.as_ref())
                        && seen.insert(path.clone())
                    {
                        wanted.push(path.clone());
                        if let Some(material) = surface.as_ref().and_then(|s| s.layered.as_ref()) {
                            layered.insert(path.clone(), material.clone());
                        }
                    }
                    surface
                });
        }
    }
    SceneSurfaces {
        surfaces,
        wanted,
        layered,
    }
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::model::Material;
    use p4kx_settings::ModelMaterials;

    use p4kx_extraction::model::{CompanionError, CompanionPath, MaterialStatus};
    use p4kx_testkit::{TestMtl, TestMtlMaterial};

    use super::surface_of;

    fn plain(diffuse: [f32; 3], specular: [f32; 3], surface: &str) -> Material {
        let mut material = Material::engine_default();
        material.diffuse = diffuse;
        material.specular = specular;
        material.surface_type = surface.to_owned();
        material
    }

    #[test]
    fn a_metal_with_a_dark_diffuse_and_a_bright_specular_is_drawn_in_its_specular_colour() {
        let metal = plain([0.02; 3], [0.8, 0.7, 0.6], "metal_dense");

        let surface = surface_of(&metal, ModelMaterials::Colours);

        assert_eq!(
            surface.map(|surface| surface.colour),
            Some([0.8, 0.7, 0.6, 1.0]),
            "the colour the GLB writer takes"
        );
    }

    #[test]
    fn a_painted_surface_keeps_its_diffuse_colour() {
        let paint = plain([0.3, 0.4, 0.5], [0.04; 3], "");

        let surface = surface_of(&paint, ModelMaterials::Colours);

        assert_eq!(
            surface.map(|surface| surface.colour),
            Some([0.3, 0.4, 0.5, 1.0]),
            "diffuse"
        );
    }

    #[test]
    fn glass_is_translucent_and_a_decal_is_not_drawn() {
        let mut glass = plain([0.1; 3], [0.04; 3], "");
        glass.shader = "GlassPBR".to_owned();
        let mut decal = plain([0.1; 3], [0.04; 3], "");
        decal.shader = "MeshDecal".to_owned();

        assert!(
            surface_of(&glass, ModelMaterials::Colours).is_some_and(|s| s.translucent),
            "glass blends"
        );
        assert!(
            surface_of(&decal, ModelMaterials::Colours).is_none(),
            "decals are left out"
        );
    }

    #[test]
    fn a_layered_material_in_the_textures_mode_is_drawn_with_its_blended_image() {
        let layer = TestMtl::single(
            &TestMtlMaterial::new("layer")
                .shader("Layer")
                .diffuse([1.0; 3])
                .texture("TexSlot1", "Objects/Test/a_diff.tif"),
        )
        .cryxml();
        let hull = TestMtlMaterial::new("hull")
            .shader("HardSurface")
            .layer_with(&[
                ("Name", "Primary"),
                ("Path", "layers/a.mtl"),
                ("TintColor", "0.5,0.5,0.5"),
            ]);
        let library = TestMtl::multi(&[hull]).cryxml();
        let mut companions =
            move |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
                Ok(match path {
                    CompanionPath::Material("objects/test/hull") => Some(library.clone()),
                    CompanionPath::Material("layers/a") => Some(layer.clone()),
                    _ => None,
                })
            };
        let status = MaterialStatus::load(Some("objects/test/hull"), &mut companions);
        let material = status
            .library()
            .and_then(|library| library.materials.first())
            .expect("the hull");

        let colours = surface_of(material, ModelMaterials::Colours).expect("a surface");
        let textures = surface_of(material, ModelMaterials::Textures).expect("a surface");

        assert!(colours.texture.is_none() && colours.layered.is_none());
        assert_eq!(colours.colour, [0.5, 0.5, 0.5, 1.0]);
        assert!(
            textures
                .texture
                .as_deref()
                .is_some_and(|key| key.starts_with(super::LAYERED_PREFIX))
        );
        assert_eq!(
            textures.colour,
            [1.0, 1.0, 1.0, 1.0],
            "the colours are in the image"
        );
        assert!(textures.layered.is_some());
    }
}
