// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use gltf_json as json;
use json::{Index, Root};
use p4kx_model::{Animation, Lod, Model, Transform};

use crate::layout::Layout;
use crate::materials::{MaterialPlan, MaterialTextures, Slot, build_material, display_name};
use crate::scene_light::SceneLight;
use crate::scene_rig::SceneRig;
use crate::texture_plan::TextureSet;

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
}

pub(crate) type MaterialIds = BTreeMap<u16, Index<json::Material>>;

/// Pushes the glTF materials of every instance, one per distinct material, and returns the glTF
/// material of each material ID per instance. Equal materials of different instances are pushed
/// once.
pub(crate) fn materials(
    root: &mut Root,
    layout: &Layout<'_>,
    instances: &[Instance<'_>],
    textures: &TextureSet,
    image_textures: &[Index<json::Texture>],
) -> Vec<MaterialIds> {
    let mut shared: HashMap<String, Index<json::Material>> = HashMap::new();
    instances
        .iter()
        .enumerate()
        .map(|(part, instance)| {
            let mut pushed: BTreeMap<Slot, Index<json::Material>> = BTreeMap::new();
            let mut by_id = MaterialIds::new();
            if instance.twin.is_some() {
                return by_id;
            }
            for id in layout.part_material_ids(part) {
                let Some(slot) = instance.plan.slot(id) else {
                    continue;
                };
                let index = if let Some(&index) = pushed.get(&slot) {
                    index
                } else {
                    let built = built_material(instance, (part, slot), textures, image_textures);
                    let index = if instances.len() > 1 {
                        push_shared(root, &mut shared, built)
                    } else {
                        root.push(built)
                    };
                    pushed.insert(slot, index);
                    index
                };
                by_id.insert(id, index);
            }
            by_id
        })
        .collect()
}

/// Returns how many materials the instances ask for: one per distinct material slot of each
/// instance that plans meshes of its own.
pub(crate) fn material_requests(layout: &Layout<'_>, instances: &[Instance<'_>]) -> usize {
    instances
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
            slots.len()
        })
        .sum()
}

fn built_material(
    instance: &Instance<'_>,
    (part, slot): (usize, Slot),
    textures: &TextureSet,
    image_textures: &[Index<json::Texture>],
) -> json::Material {
    let texture = |image: usize| image_textures.get(image).copied();
    let material = instance.plan.material(slot);
    let images = textures.of(part, slot);
    let drawn_with = MaterialTextures {
        base: images
            .base
            .and_then(|base| Some((texture(base.image)?, base.tinted, base.layered))),
        normal: images.normal.and_then(texture),
        metallic_roughness: images
            .metallic_roughness
            .and_then(|packed| Some((texture(packed.image)?, packed.roughness, packed.metallic))),
    };
    build_material(material, &display_name(material, slot), &drawn_with)
}

fn push_shared(
    root: &mut Root,
    shared: &mut HashMap<String, Index<json::Material>>,
    material: json::Material,
) -> Index<json::Material> {
    match serde_json::to_string(&material) {
        Ok(key) => *shared
            .entry(key)
            .or_insert_with(|| root.push(material.clone())),
        Err(_) => root.push(material),
    }
}
