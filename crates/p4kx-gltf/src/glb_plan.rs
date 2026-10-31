// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use gltf_json as json;
use p4kx_model::{Animation, Influences};

use crate::animations::{AnimationPlan, Targets, plan as plan_animations};
use crate::document::skeleton_written;
use crate::glb::{GlbOptions, GlbSummary};
use crate::instance::{Content, Instance};
use crate::layout::{Layout, Rigging};
use crate::materials::Slot;
use crate::skin::inverse_binds;
use crate::texture_loader::TextureLoader;
use crate::texture_plan::{PartSlots, TextureSet};

/// The skeleton the skinned meshes of a document are weighted to: its inverse bind matrices and,
/// per instance and mesh, the influences to write instead of the meshes' own.
#[derive(Clone, Copy)]
pub(crate) struct Skeletal<'m> {
    pub(crate) binds: &'m [[f32; 16]],
    pub(crate) influences: Option<&'m [Vec<Vec<Influences>>]>,
}

impl<'m> Skeletal<'m> {
    /// Weights the meshes to `binds`, the skeleton of their own model, with their own influences.
    pub(crate) const fn own(binds: &'m [[f32; 16]]) -> Self {
        Self {
            binds,
            influences: None,
        }
    }
}

/// Returns the inverse bind matrices of the skeleton of a model that is written alone, if it has
/// bones and a skinned mesh at its level of detail.
pub(crate) fn own_binds(instance: &Instance<'_>) -> Option<Vec<[f32; 16]>> {
    let lod = instance.lod?;
    let bones = instance.model.bones_of(lod);
    (!bones.is_empty() && lod.meshes.iter().any(|mesh| mesh.influences.is_some()))
        .then(|| inverse_binds(bones))
}

pub(crate) fn plan_layout<'m>(
    instances: &[Instance<'m>],
    options: &GlbOptions,
    skeletal: Option<Skeletal<'m>>,
) -> Layout<'m> {
    let mut layout = Layout::new().limited(options.max_influences);
    let mut hidden = vec![0; instances.len()];
    for (part, instance) in instances.iter().enumerate() {
        if let Some(twin) = instance.twin {
            layout.hidden_primitives += hidden.get(twin).copied().unwrap_or(0);
        } else if let Some(lod) = instance.lod {
            let before = layout.hidden_primitives;
            let rigging = Rigging {
                binds: skeletal.map(|skeletal| skeletal.binds),
                influences: skeletal
                    .and_then(|skeletal| skeletal.influences?.get(part))
                    .map(Vec::as_slice),
                morphs: &[],
            };
            layout.add_part(part, lod, options.vertex_colours, &instance.plan, rigging);
            if let Some(slot) = hidden.get_mut(part) {
                *slot = layout.hidden_primitives - before;
            }
        }
    }
    layout
}

/// Plans the animations of `content`: on the skeleton of the one model written alone, else on the
/// skeleton of the scene.
pub(crate) fn animation_plans<'m>(
    layout: &mut Layout<'m>,
    content: &Content<'_, 'm>,
    single: Option<&Instance<'m>>,
) -> Vec<AnimationPlan> {
    match single {
        Some(instance) => single_animation_plans(layout, instance, content.animations),
        None => scene_animation_plans(layout, content),
    }
}

/// Plans the animations of a scene whose skinned meshes are bound to the skeleton of `rig`: they
/// move the bone nodes, which come after the nodes of the wrappers and of every model instance.
/// Nothing is planned when no mesh is skinned, as no bones are written then.
fn scene_animation_plans<'m>(
    layout: &mut Layout<'m>,
    content: &Content<'_, 'm>,
) -> Vec<AnimationPlan> {
    let Some(rig) = content.rig.filter(|_| layout.inverse_binds.is_some()) else {
        return Vec::new();
    };
    let nodes = content.wrappers.len()
        + content
            .instances
            .iter()
            .map(|instance| instance.model.nodes.len())
            .sum::<usize>();
    let targets = Targets {
        bones: rig.bones,
        first_bone_node: Some(nodes),
        model_nodes: nodes,
    };
    plan_animations(layout, content.animations.iter().copied(), &targets)
}

fn single_animation_plans<'m>(
    layout: &mut Layout<'m>,
    instance: &Instance<'m>,
    animations: &[&'m Animation],
) -> Vec<AnimationPlan> {
    let Some(lod) = instance.lod else {
        return Vec::new();
    };
    let model = instance.model;
    let written = skeleton_written(model, lod, layout);
    let targets = Targets {
        bones: if written {
            model.bones_of(lod)
        } else {
            &model.bones
        },
        first_bone_node: written.then_some(model.nodes.len()),
        model_nodes: model.nodes.len(),
    };
    plan_animations(layout, animations.iter().copied(), &targets)
}

pub(crate) fn texture_set(
    layout: &Layout<'_>,
    instances: &[Instance<'_>],
    mip: u32,
    textures: &dyn TextureLoader,
) -> TextureSet {
    let parts: Vec<PartSlots<'_>> = instances
        .iter()
        .enumerate()
        .filter(|(_, instance)| instance.twin.is_none())
        .map(|(part, instance)| {
            let mut slots: Vec<Slot> = Vec::new();
            for slot in layout
                .part_material_ids(part)
                .into_iter()
                .filter_map(|id| instance.plan.slot(id))
            {
                if !slots.contains(&slot) {
                    slots.push(slot);
                }
            }
            PartSlots {
                part,
                plan: &instance.plan,
                slots,
            }
        })
        .collect();
    TextureSet::build(&parts, mip, textures)
}

/// Returns how many texture references of the materials point at a texture another reference
/// already uses.
fn shared_textures(root: &json::Root) -> usize {
    let mut used = vec![false; root.textures.len()];
    let mut references = 0_usize;
    for material in &root.materials {
        let pbr = &material.pbr_metallic_roughness;
        let indices = [
            pbr.base_color_texture.as_ref().map(|info| info.index),
            pbr.metallic_roughness_texture
                .as_ref()
                .map(|info| info.index),
            material.normal_texture.as_ref().map(|info| info.index),
        ];
        for index in indices.into_iter().flatten() {
            references += 1;
            if let Some(slot) = used.get_mut(index.value()) {
                *slot = true;
            }
        }
    }
    references.saturating_sub(used.iter().filter(|used| **used).count())
}

pub(crate) fn summarise(
    root: &json::Root,
    layout: &Layout<'_>,
    defaulted: usize,
    drawn_nodes: usize,
    images: TextureSet,
    animations: &[AnimationPlan],
    bytes: u64,
) -> GlbSummary {
    let meshes = &layout.meshes;
    let vertices = meshes
        .iter()
        .filter_map(|mesh| layout.blocks.get(mesh.positions))
        .map(|block| block.data.count())
        .sum();
    let triangles = meshes
        .iter()
        .flat_map(|mesh| &mesh.primitives)
        .filter_map(|primitive| layout.blocks.get(primitive.indices))
        .map(|block| block.data.count() / 3)
        .sum();
    GlbSummary {
        nodes: root.nodes.len(),
        meshes: root.meshes.len(),
        primitives: meshes.iter().map(|mesh| mesh.primitives.len()).sum(),
        materials: root.materials.len(),
        textured_materials: root
            .materials
            .iter()
            .filter(|material| {
                material.pbr_metallic_roughness.base_color_texture.is_some()
                    || material
                        .pbr_metallic_roughness
                        .metallic_roughness_texture
                        .is_some()
                    || material.normal_texture.is_some()
            })
            .count(),
        textures: images.images.len(),
        texture_pixels: images.pixels(),
        texture_fallbacks: images.fallbacks,
        default_material_ids: defaulted,
        hidden_primitives: layout.hidden_primitives,
        bones: root.nodes.len().saturating_sub(drawn_nodes),
        skinned_meshes: meshes.iter().filter(|mesh| !mesh.joints.is_empty()).count(),
        vertices,
        triangles,
        morph_targets: meshes.iter().map(|mesh| mesh.morphs.len()).sum(),
        animations: animations.iter().map(|plan| plan.summary.clone()).collect(),
        bytes,
        parts: 0,
        lod_fallbacks: 0,
        shared_meshes: 0,
        shared_materials: 0,
        shared_textures: shared_textures(root),
        lights: root
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.khr_lights_punctual.as_ref())
            .map_or(0, |lights| lights.lights.len()),
    }
}
