// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{MaterialStatus, Model, Transform};

use crate::axes::decompose;
use crate::materials::{MaterialPlan, Slot, display_name};
use crate::scene_light::LIGHT_FRAME;
use crate::texture_report::TextureRole;
use crate::texture_roles::{Source, canonical, sources};

/// A node transform in glTF axes (Y up, metres) as the GLB writer writes it.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct NodeTransform {
    /// The translation.
    pub translation: [f32; 3],
    /// The rotation as a unit quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    /// The scale; a negative X scale marks a mirroring transform.
    pub scale: [f32; 3],
}

/// Returns `transform`, given in `CryEngine` coordinates (Z up), as the GLB writer writes it for a
/// scene node: in glTF axes, split into translation, rotation and scale; a shear is dropped.
#[must_use]
pub fn node_transform(transform: &Transform) -> NodeTransform {
    let trs = decompose(transform);
    NodeTransform {
        translation: trs.translation,
        rotation: trs.rotation,
        scale: trs.scale,
    }
}

/// Returns `transform`, the placement of a light, as the GLB writer writes its node.
///
/// `transform` is in `CryEngine` coordinates; the node is turned so that the light shines along its
/// -Z axis, as glTF and Blender lights do.
#[must_use]
pub fn light_node_transform(transform: &Transform) -> NodeTransform {
    node_transform(&transform.then(&LIGHT_FRAME))
}

/// One texture file a material names for a glTF role.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TextureFile {
    /// The path as a [`TextureLoader`](crate::TextureLoader) takes it: below `Data`, `/` as
    /// separator, extension `.dds`.
    pub path: String,
    /// The path as the material file names it.
    pub file: String,
    /// Whether the texture carries the smoothness in its alpha channel.
    pub gloss: bool,
    /// Whether the authored map supplies opacity or density rather than diffuse RGB.
    pub opacity: bool,
}

fn texture_file(source: Source) -> TextureFile {
    TextureFile {
        path: source.path,
        file: source.file,
        gloss: source.gloss,
        opacity: source.opacity,
    }
}

/// A material a model is drawn with and the texture files that feed glTF's PBR textures.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DrawnMaterial {
    /// The name of the glTF material of the GLB writer: the material file's, else
    /// `material_<position>`, else `default`.
    pub name: String,
    /// The position of the material in its library's sub-material list (0 for a library that is
    /// one material); `None` for the engine's default material.
    pub library_index: Option<usize>,
    /// The diffuse map.
    pub diffuse: Option<TextureFile>,
    /// The normal map.
    pub normal: Option<TextureFile>,
    /// The specular map.
    pub specular: Option<TextureFile>,
    /// The files that would have been used but have no safe path, with their role.
    pub refused: Vec<(TextureRole, String)>,
}

/// Returns the materials `model` is drawn with at `level` (or the nearest lower level it has),
/// one per distinct material in the order of the material IDs, with their texture files.
///
/// The materials are those [`write_scene_glb`](crate::write_scene_glb) writes: submeshes whose
/// material draws nothing are left out and a material ID the library lacks gets the engine's
/// default material, which has no textures.
#[must_use]
pub fn drawn_materials(
    model: &Model,
    materials: &MaterialStatus,
    level: u32,
) -> Vec<DrawnMaterial> {
    let Some(lod) = model.nearest_lod(level) else {
        return Vec::new();
    };
    let plan = MaterialPlan::new(materials.library(), lod);
    plan.drawn_slots()
        .into_iter()
        .map(|slot| {
            let material = plan.material(slot);
            let found = sources(material);
            DrawnMaterial {
                name: display_name(material, slot),
                library_index: match slot {
                    Slot::Library(position) => Some(position),
                    Slot::Default => None,
                },
                diffuse: found.diffuse.map(texture_file),
                normal: found.normal.map(texture_file),
                specular: found.specular.map(texture_file),
                refused: found.refused,
            }
        })
        .collect()
}

/// Returns the path of the texture `file` as a loader takes it.
///
/// The path is below `Data`, with `/` as separator and the extension `.dds`. An engine placeholder
/// such as `$RenderToTexture` and a path that is no safe path give `None`.
#[must_use]
pub fn texture_path(file: &str) -> Option<String> {
    canonical(file)
}
