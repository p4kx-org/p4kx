// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use gltf_json as json;
use json::{Index, Root};
use p4kx_model::{Animation, Lod, Model, Transform};
use serde_json::Value;

use crate::factory_bow_reflectance;
use crate::layout::{Layout, MaterialUse};
use crate::materials::{MaterialPlan, MaterialTextures, Slot, build_material, display_name};
use crate::scene_light::SceneLight;
use crate::scene_rig::SceneRig;
use crate::texture_plan::{SlotImages, TextureSet};

/// A glTF node of the scene that holds no model of its own.
pub(crate) struct Wrapper<'m> {
    pub(crate) name: &'m str,
    pub(crate) parent: Option<usize>,
    pub(crate) transform: &'m Transform,
    pub(crate) light: Option<&'m SceneLight>,
}

/// One model written into the document: its nodes hang below the `wrapper` node, or are roots of
/// the scene without one.
pub(crate) struct Instance<'m> {
    pub(crate) model: &'m Model,
    pub(crate) lod: Option<&'m Lod>,
    pub(crate) wrapper: Option<usize>,
    pub(crate) plan: MaterialPlan<'m>,
    /// The earlier instance of the same model, level, material library and palette whose meshes this one
    /// reuses; such an instance plans no blocks and no materials of its own.
    pub(crate) twin: Option<usize>,
}

/// What a document is written from: the `wrappers`, the model `instances` below them, and the
/// skeleton the skinned instances are bound to, if there is one.
pub(crate) struct Content<'a, 'm> {
    pub(crate) wrappers: &'a [Wrapper<'m>],
    pub(crate) instances: &'a [Instance<'m>],
    pub(crate) rig: Option<&'a SceneRig<'m>>,
    pub(crate) animations: &'a [&'m Animation],
    pub(crate) asset_extras: Option<&'a Value>,
}

pub(crate) type MaterialIds = BTreeMap<MaterialUse, Index<json::Material>>;

/// Pushes the materials of every instance and returns their indices and the number shared.
pub(crate) fn materials(
    root: &mut Root,
    layout: &Layout<'_>,
    instances: &[Instance<'_>],
    textures: &TextureSet,
    image_textures: &[Index<json::Texture>],
) -> (Vec<MaterialIds>, usize) {
    let mut shared: HashMap<(String, bool), Index<json::Material>> = HashMap::new();
    let mut requests = 0_usize;
    let ids = instances
        .iter()
        .enumerate()
        .map(|(part, instance)| {
            let mut pushed: BTreeMap<(Slot, bool), Index<json::Material>> = BTreeMap::new();
            let mut by_id = MaterialIds::new();
            if instance.twin.is_some() {
                return by_id;
            }
            for usage in layout.part_materials(part) {
                let Some(slot) = instance.plan.slot(usage.id) else {
                    continue;
                };
                let images = if usage.has_uvs {
                    textures.of(part, slot)
                } else {
                    SlotImages::default()
                };
                let has_images = images.base.is_some()
                    || images.normal.is_some()
                    || images.metallic_roughness.is_some()
                    || images.specular_colour.is_some();
                let key = (slot, has_images);
                let index = if let Some(&index) = pushed.get(&key) {
                    index
                } else {
                    requests += 1;
                    let built = built_material(instance, slot, images, image_textures);
                    let index = if instances.len() > 1 {
                        let physical =
                            slot == Slot::Default && instance.plan.has_physical_fallback();
                        push_shared(root, &mut shared, built, physical)
                    } else {
                        root.push(built)
                    };
                    pushed.insert(key, index);
                    index
                };
                by_id.insert(usage, index);
            }
            by_id
        })
        .collect();
    declare_material_extensions(root);
    (ids, requests.saturating_sub(root.materials.len()))
}

fn declare_material_extensions(root: &mut Root) {
    if root.materials.iter().any(|material| {
        material
            .extensions
            .as_ref()
            .is_some_and(|extensions| extensions.emissive_strength.is_some())
    }) {
        root.extensions_used
            .push("KHR_materials_emissive_strength".to_owned());
    }
    if root.materials.iter().any(|material| {
        material
            .extensions
            .as_ref()
            .is_some_and(|extensions| extensions.specular.is_some())
    }) {
        root.extensions_used
            .push(factory_bow_reflectance::EXTENSION.to_owned());
    }
}

fn built_material(
    instance: &Instance<'_>,
    slot: Slot,
    images: SlotImages,
    image_textures: &[Index<json::Texture>],
) -> json::Material {
    let texture = |image: usize| image_textures.get(image).copied();
    let material = instance.plan.material(slot);
    let drawn_with = MaterialTextures {
        base: images
            .base
            .and_then(|base| Some((texture(base.image)?, base.tinted, base.layered))),
        normal: images.normal.and_then(texture),
        metallic_roughness: images
            .metallic_roughness
            .and_then(|packed| Some((texture(packed.image)?, packed.roughness, packed.metallic))),
        specular_colour: images.specular_colour.and_then(texture),
    };
    build_material(material, &display_name(material, slot), &drawn_with)
}

fn push_shared(
    root: &mut Root,
    shared: &mut HashMap<(String, bool), Index<json::Material>>,
    material: json::Material,
    physical_fallback: bool,
) -> Index<json::Material> {
    match serde_json::to_string(&material) {
        Ok(key) => *shared
            .entry((key, physical_fallback))
            .or_insert_with(|| root.push(material.clone())),
        Err(_) => root.push(material),
    }
}
