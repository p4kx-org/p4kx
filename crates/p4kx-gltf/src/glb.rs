// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use gltf_json as json;
use p4kx_model::{Animation, Lod, MAX_INFLUENCES, Model};

use crate::GltfError;
use crate::animations::AnimationSummary;
use crate::document::document;
use crate::glb_plan::{Skeletal, animation_plans, own_binds, plan_layout, summarise, texture_set};
use crate::instance::{Content, Instance, material_requests};
use crate::layout::Layout;
use crate::material_index::index_materials;
use crate::materials::MaterialPlan;
use crate::scene_rig::SceneRig;
use crate::texture_loader::{NoTextures, TextureLoader};
use crate::texture_plan::TextureSet;
use crate::texture_report::TextureFallback;

const MAGIC: &[u8; 4] = b"glTF";
const VERSION: u32 = 2;
const JSON_CHUNK: u32 = 0x4E4F_534A;
const BIN_CHUNK: u32 = 0x004E_4942;
const HEADER_LEN: u64 = 12;
const CHUNK_HEADER_LEN: u64 = 8;

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
    /// primitives whose material [reads them](p4kx_model::Material::reads_vertex_colours); viewers
    /// multiply the base colour of the material by them.
    pub vertex_colours: bool,
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

    /// Returns the options without vertex colours.
    #[must_use]
    pub const fn without_vertex_colours(mut self) -> Self {
        self.vertex_colours = false;
        self
    }
}

/// What a GLB export wrote.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct GlbSummary {
    /// The number of glTF nodes.
    pub nodes: usize,
    /// The number of glTF meshes.
    pub meshes: usize,
    /// The number of primitives, one per submesh.
    pub primitives: usize,
    /// The number of materials: one per sub-material the drawn submeshes use, and one for the
    /// engine's default material when any material ID falls back to it.
    pub materials: usize,
    /// The number of materials that have at least one texture.
    pub textured_materials: usize,
    /// The number of PNG images embedded; an image several materials use is embedded once.
    pub textures: usize,
    /// The number of pixels of all embedded images.
    pub texture_pixels: u64,
    /// The textures that were left out, with the reason: the material keeps its plain colour.
    pub texture_fallbacks: Vec<TextureFallback>,
    /// The number of material IDs drawn with the engine's default material, because the model
    /// has no readable material library or the library has no sub-material for them.
    pub default_material_ids: usize,
    /// The number of submeshes left out because their material draws nothing: collision,
    /// shield and helper geometry.
    pub hidden_primitives: usize,
    /// The number of bones written as nodes: those of a skinned model, and those of a skeleton
    /// file without meshes.
    pub bones: usize,
    /// The number of meshes that carry bone weights.
    pub skinned_meshes: usize,
    /// The number of vertices of all meshes.
    pub vertices: usize,
    /// The number of triangles of all meshes.
    pub triangles: usize,
    /// The number of morph targets written, over all meshes.
    pub morph_targets: usize,
    /// The animations that were asked for, in order, with what became of each; one without
    /// channels is not written.
    pub animations: Vec<AnimationSummary>,
    /// The length of the file in bytes.
    pub bytes: u64,
    /// The number of models written: 1 for a single model, one per scene node with a model.
    pub parts: usize,
    /// The number of scene models drawn at another level of detail than the one asked for,
    /// because they lack it.
    pub lod_fallbacks: usize,
    /// The number of scene meshes that are not written again because an earlier scene node holds
    /// the same model at the same level with the same material library; their nodes reference
    /// the first one's glTF mesh.
    pub shared_meshes: usize,
    /// The number of material requests of the written models that reuse an equal glTF material.
    pub shared_materials: usize,
    /// The number of texture references of materials that reuse an image another reference
    /// already uses, so the image is embedded once.
    pub shared_textures: usize,
    /// The number of lights written with `KHR_lights_punctual`.
    pub lights: usize,
}

/// Writes `model` as a GLB file to `out`: the JSON chunk first, then the binary data, streamed
/// block by block; `out` should be buffered.
///
/// # Errors
///
/// Returns [`GltfError::NoSuchLod`] when the model lacks the level of detail in `options`,
/// [`GltfError::TooLarge`] when it does not fit in a GLB file, and [`GltfError::Write`] when
/// `out` fails.
pub fn write_glb(
    model: &Model,
    options: &GlbOptions,
    out: &mut impl Write,
) -> Result<GlbSummary, GltfError> {
    write_glb_animated_with(model, &[], options, &NoTextures, out)
}

/// Writes `model` as a GLB file to `out` like [`write_glb`], taking the textures of
/// [`MaterialMode::Textures`] from `textures`.
///
/// The images are decoded, combined and PNG-encoded before the first byte is written, a few at a
/// time; one image is embedded once however many materials use it. At most
/// [`crate::MAX_TEXTURE_IMAGES`] images and [`crate::MAX_TEXTURE_PIXELS`] pixels are embedded; the textures
/// beyond are left out and listed like any texture that cannot be loaded.
///
/// # Errors
///
/// As [`write_glb`].
pub fn write_glb_with(
    model: &Model,
    options: &GlbOptions,
    textures: &dyn TextureLoader,
    out: &mut impl Write,
) -> Result<GlbSummary, GltfError> {
    write_glb_animated_with(model, &[], options, textures, out)
}

/// Writes `model` with `animations` as a GLB file to `out`; see [`write_glb`].
///
/// Each track of an animation moves the node of the bone with its controller ID: a bone node
/// when the skeleton is written (a skinned model or a skeleton file), otherwise the node a
/// static model's helper bone drives. Rotation and translation keys become one channel each,
/// linear between keys and constant for a single key; times are in seconds from the frame rate of
/// the animation. Tracks without such a bone are left out and listed in
/// [`GlbSummary::animations`].
///
/// # Errors
///
/// As [`write_glb`].
pub fn write_glb_animated(
    model: &Model,
    animations: &[Animation],
    options: &GlbOptions,
    out: &mut impl Write,
) -> Result<GlbSummary, GltfError> {
    write_glb_animated_with(model, animations, options, &NoTextures, out)
}

/// Writes `model` with `animations` as a GLB file to `out`, taking the textures from `textures`;
/// see [`write_glb_with`] and [`write_glb_animated`].
///
/// # Errors
///
/// As [`write_glb`].
pub fn write_glb_animated_with(
    model: &Model,
    animations: &[Animation],
    options: &GlbOptions,
    textures: &dyn TextureLoader,
    out: &mut impl Write,
) -> Result<GlbSummary, GltfError> {
    let lod = find_lod(model, options.lod)?;
    let instance = Instance {
        model,
        lod: Some(lod),
        wrapper: None,
        plan: MaterialPlan::new(model.materials.library(), lod),
        twin: None,
    };
    let animations: Vec<&Animation> = animations.iter().collect();
    let content = Content {
        wrappers: &[],
        instances: &[instance],
        rig: None,
        animations: &animations,
    };
    write_instances(&content, options, textures, 0, out)
}

/// Returns how many meshes the twin instances reuse instead of writing them again.
fn shared_meshes(instances: &[Instance<'_>], layout: &Layout<'_>) -> usize {
    instances
        .iter()
        .filter_map(|instance| instance.twin)
        .map(|twin| {
            layout
                .meshes
                .iter()
                .filter(|mesh| mesh.part == twin)
                .count()
        })
        .sum()
}

pub(crate) fn write_instances(
    content: &Content<'_, '_>,
    options: &GlbOptions,
    textures: &dyn TextureLoader,
    lod_fallbacks: usize,
    out: &mut impl Write,
) -> Result<GlbSummary, GltfError> {
    let Content {
        wrappers,
        instances,
        rig,
        ..
    } = *content;
    let single = match (wrappers, instances) {
        ([], [instance]) => Some(instance),
        _ => None,
    };
    let binds = single.and_then(own_binds);
    let skeletal = rig
        .map(SceneRig::skeletal)
        .or_else(|| binds.as_deref().map(Skeletal::own));
    let mut layout = plan_layout(instances, options, skeletal);
    let images = match options.materials {
        MaterialMode::Textures => texture_set(&layout, instances, options.texture_mip, textures),
        MaterialMode::Colours => TextureSet::default(),
    };
    let animation_plans = animation_plans(&mut layout, content, single);
    layout.push_images(images.images.iter().map(|image| image.png.len() as u64));
    let mut root = document(content, &layout, &images, &animation_plans);
    if options.material_indices {
        index_materials(&mut root);
    }
    let total = write_file(&root, &layout, &images, out)?;
    let defaulted = instances
        .iter()
        .map(|instance| instance.plan.defaulted())
        .sum();
    let drawn_nodes = wrappers.len()
        + instances
            .iter()
            .map(|instance| instance.model.nodes.len())
            .sum::<usize>();
    let mut summary = summarise(
        &root,
        &layout,
        defaulted,
        drawn_nodes,
        images,
        &animation_plans,
        total,
    );
    summary.parts = instances.len();
    summary.lod_fallbacks = lod_fallbacks;
    summary.shared_meshes = shared_meshes(instances, &layout);
    if instances.len() > 1 {
        summary.shared_materials =
            material_requests(&layout, instances).saturating_sub(root.materials.len());
    }
    Ok(summary)
}

pub(crate) fn write_file(
    root: &json::Root,
    layout: &Layout<'_>,
    images: &TextureSet,
    out: &mut impl Write,
) -> Result<u64, GltfError> {
    let mut json = root.to_vec()?;
    json.resize(json.len().next_multiple_of(4), b' ');
    let json_len = json.len() as u64;
    let bin_len = layout.len;
    let mut total = HEADER_LEN + CHUNK_HEADER_LEN + json_len;
    if bin_len > 0 {
        total += CHUNK_HEADER_LEN + bin_len;
    }
    let too_large = || GltfError::TooLarge { len: total };
    let total_word = u32::try_from(total).map_err(|_| too_large())?;
    let json_word = u32::try_from(json_len).map_err(|_| too_large())?;
    let bin_word = u32::try_from(bin_len).map_err(|_| too_large())?;

    out.write_all(MAGIC)?;
    out.write_all(&VERSION.to_le_bytes())?;
    out.write_all(&total_word.to_le_bytes())?;
    out.write_all(&json_word.to_le_bytes())?;
    out.write_all(&JSON_CHUNK.to_le_bytes())?;
    out.write_all(&json)?;
    if bin_len > 0 {
        out.write_all(&bin_word.to_le_bytes())?;
        out.write_all(&BIN_CHUNK.to_le_bytes())?;
        layout.write(out, &images.images)?;
    }
    out.flush()?;
    Ok(total)
}

fn find_lod(model: &Model, level: u32) -> Result<&Lod, GltfError> {
    model
        .lods
        .iter()
        .find(|lod| lod.level == level)
        .ok_or_else(|| GltfError::NoSuchLod {
            lod: level,
            available: model
                .lods
                .iter()
                .map(|lod| format!("LOD{}", lod.level))
                .collect::<Vec<_>>()
                .join(", "),
        })
}
