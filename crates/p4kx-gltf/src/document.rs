// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use gltf_json as json;
use json::accessor::GenericComponentType;
use json::mesh::{Mode, Primitive, Semantic};
use json::validation::Checked::Valid;
use json::validation::USize64;
use json::{Accessor, Index, Root};
use p4kx_model::{Lod, Model};

use crate::animations::{AnimationPlan, push_animations};
use crate::block_data::BlockData;
use crate::instance::{Content, Instance, MaterialIds, materials};
use crate::layout::{Block, Layout, MeshLayout};
use crate::morphs::{mesh_extras, primitive_targets, push_targets};
use crate::nodes::{Skeleton, push_nodes};
use crate::scene_nodes::{SceneSkin, push_scene_nodes};
use crate::scene_rig::SceneRig;
use crate::texture_plan::TextureSet;

pub(crate) const GENERATOR: &str = concat!("P4KX ", env!("CARGO_PKG_VERSION"));
const PNG_MIME: &str = "image/png";

/// Builds the glTF document of the `wrappers` and the model `instances` over the planned binary
/// chunk `layout`.
///
/// Without wrappers and with one instance, the bones become nodes when a mesh of the level of
/// detail is skinned and when the level has no mesh at all, as in a skeleton file; the skinned
/// meshes then share one skin. With wrappers, the scene's skeleton gets one skin and its bones
/// become the last nodes. The animations of `animations` are written after the nodes they move.
pub(crate) fn document(
    content: &Content<'_, '_>,
    layout: &Layout<'_>,
    textures: &TextureSet,
    animations: &[AnimationPlan],
) -> Root {
    let Content {
        wrappers,
        instances,
        rig,
        ..
    } = *content;
    let mut root = Root {
        asset: json::Asset {
            generator: Some(GENERATOR.to_owned()),
            ..json::Asset::default()
        },
        ..Root::default()
    };
    let buffer = (layout.len > 0).then(|| {
        root.push(json::Buffer {
            byte_length: USize64(layout.len),
            name: None,
            uri: None,
            extensions: None,
            extras: json::extras::Extras::default(),
        })
    });
    let image_textures = buffer
        .map(|buffer| push_images(&mut root, buffer, layout))
        .unwrap_or_default();
    let materials = materials(&mut root, layout, instances, textures, &image_textures);
    let (node_meshes, skinned) = buffer.map_or_else(
        || empty_nodes(instances),
        |buffer| push_meshes(&mut root, buffer, layout, instances, &materials),
    );
    if let ([], [instance], [meshes], [skinned]) = (
        wrappers,
        instances,
        node_meshes.as_slice(),
        skinned.as_slice(),
    ) {
        push_single(
            &mut root, buffer, layout, instance, meshes, skinned, animations,
        );
    } else {
        let skin = scene_skin(&mut root, buffer, layout, rig, &skinned);
        push_scene_nodes(
            &mut root,
            (wrappers, instances),
            &node_meshes,
            skin.as_ref(),
        );
        if let Some(buffer) = buffer {
            push_animations(&mut root, buffer, layout, animations);
        }
    }
    root
}

/// Returns the skeleton of the scene when `rig` is given and a mesh is skinned, after writing the
/// accessor of its inverse bind matrices.
fn scene_skin<'a>(
    root: &mut Root,
    buffer: Option<Index<json::Buffer>>,
    layout: &Layout<'_>,
    rig: Option<&'a SceneRig<'_>>,
    skinned: &'a [Vec<bool>],
) -> Option<SceneSkin<'a>> {
    let rig = rig.filter(|_| skinned.iter().flatten().any(|&skinned| skinned))?;
    let block = layout
        .inverse_binds
        .and_then(|block| layout.blocks.get(block))?;
    Some(SceneSkin {
        bones: rig.bones,
        inverse_binds: push_accessor(root, buffer?, block, None),
        skinned,
    })
}

pub(crate) type NodeMeshes = Vec<Vec<Option<Index<json::Mesh>>>>;

fn empty_nodes(instances: &[Instance<'_>]) -> (NodeMeshes, Vec<Vec<bool>>) {
    let meshes = instances
        .iter()
        .map(|instance| vec![None; instance.model.nodes.len()])
        .collect();
    let skinned = instances
        .iter()
        .map(|instance| vec![false; instance.model.nodes.len()])
        .collect();
    (meshes, skinned)
}

/// Pushes the mesh of every mesh layout and returns, per instance and node, the mesh and whether
/// it is skinned.
pub(crate) fn push_meshes(
    root: &mut Root,
    buffer: Index<json::Buffer>,
    layout: &Layout<'_>,
    instances: &[Instance<'_>],
    materials: &[MaterialIds],
) -> (NodeMeshes, Vec<Vec<bool>>) {
    let (mut node_meshes, mut skinned) = empty_nodes(instances);
    for mesh in &layout.meshes {
        let name = instances
            .get(mesh.part)
            .and_then(|instance| instance.model.nodes.get(mesh.node))
            .map(|node| node.name.clone());
        let index = push_mesh(root, buffer, layout, mesh, materials.get(mesh.part), name);
        if let Some(slot) = node_meshes
            .get_mut(mesh.part)
            .and_then(|nodes| nodes.get_mut(mesh.node))
        {
            *slot = Some(index);
        }
        if let Some(slot) = skinned
            .get_mut(mesh.part)
            .and_then(|nodes| nodes.get_mut(mesh.node))
        {
            *slot = !mesh.joints.is_empty();
        }
    }
    for (part, instance) in instances.iter().enumerate() {
        if let Some(twin) = instance.twin
            && let Some(shared) = node_meshes.get(twin).cloned()
            && let Some(slot) = node_meshes.get_mut(part)
        {
            *slot = shared;
        }
        if let Some(twin) = instance.twin
            && let Some(shared) = skinned.get(twin).cloned()
            && let Some(slot) = skinned.get_mut(part)
        {
            *slot = shared;
        }
    }
    (node_meshes, skinned)
}

fn push_single(
    root: &mut Root,
    buffer: Option<Index<json::Buffer>>,
    layout: &Layout<'_>,
    instance: &Instance<'_>,
    meshes: &[Option<Index<json::Mesh>>],
    skinned: &[bool],
    animations: &[AnimationPlan],
) {
    let binds = buffer
        .zip(
            layout
                .inverse_binds
                .and_then(|block| layout.blocks.get(block)),
        )
        .map(|(buffer, block)| push_accessor(root, buffer, block, None));
    let has_skin = skinned.contains(&true) && binds.is_some();
    let bones = match instance.lod {
        Some(lod) if has_skin || lod.meshes.is_empty() => instance.model.bones_of(lod),
        _ => &[],
    };
    push_nodes(
        root,
        instance.model,
        meshes,
        &Skeleton {
            bones,
            skinned: if has_skin { skinned } else { &[] },
            inverse_binds: binds.filter(|_| has_skin),
        },
    );
    if let Some(buffer) = buffer {
        push_animations(root, buffer, layout, animations);
    }
}

/// Returns whether the document writes the bones of the level of detail as nodes: when a mesh
/// is skinned, and when the level has no mesh at all, as in a skeleton file.
pub(crate) fn skeleton_written(model: &Model, lod: &Lod, layout: &Layout<'_>) -> bool {
    let skinned = layout
        .meshes
        .iter()
        .any(|mesh| !mesh.joints.is_empty() && mesh.node < model.nodes.len());
    (skinned && layout.inverse_binds.is_some()) || lod.meshes.is_empty()
}

/// Pushes the sampler, an image of the binary chunk and a texture for each embedded image, and
/// returns the textures in the order of the images.
pub(crate) fn push_images(
    root: &mut Root,
    buffer: Index<json::Buffer>,
    layout: &Layout<'_>,
) -> Vec<Index<json::Texture>> {
    if layout.images.is_empty() {
        return Vec::new();
    }
    let sampler = root.push(json::texture::Sampler {
        mag_filter: Some(Valid(json::texture::MagFilter::Linear)),
        min_filter: Some(Valid(json::texture::MinFilter::LinearMipmapLinear)),
        wrap_s: Valid(json::texture::WrappingMode::Repeat),
        wrap_t: Valid(json::texture::WrappingMode::Repeat),
        name: None,
        extensions: None,
        extras: json::extras::Extras::default(),
    });
    layout
        .images
        .iter()
        .map(|block| {
            let view = root.push(json::buffer::View {
                buffer,
                byte_length: USize64(block.len),
                byte_offset: Some(USize64(block.offset)),
                byte_stride: None,
                target: None,
                name: None,
                extensions: None,
                extras: json::extras::Extras::default(),
            });
            let source = root.push(json::Image {
                buffer_view: Some(view),
                mime_type: Some(json::image::MimeType(PNG_MIME.to_owned())),
                name: None,
                uri: None,
                extensions: None,
                extras: json::extras::Extras::default(),
            });
            root.push(json::Texture {
                name: None,
                sampler: Some(sampler),
                source,
                extensions: None,
                extras: json::extras::Extras::default(),
            })
        })
        .collect()
}

fn push_mesh(
    root: &mut Root,
    buffer: Index<json::Buffer>,
    layout: &Layout<'_>,
    mesh: &MeshLayout,
    materials: Option<&MaterialIds>,
    name: Option<String>,
) -> Index<json::Mesh> {
    let mut attributes = BTreeMap::new();
    let block = |index: usize| layout.blocks.get(index).copied();
    if let Some(positions) = block(mesh.positions) {
        let accessor = push_accessor(
            root,
            buffer,
            &positions,
            Some((mesh.min.as_slice(), mesh.max.as_slice())),
        );
        attributes.insert(Valid(Semantic::Positions), accessor);
    }
    if let Some(normals) = mesh.normals.and_then(block) {
        let accessor = push_accessor(root, buffer, &normals, None);
        attributes.insert(Valid(Semantic::Normals), accessor);
    }
    if let Some(uvs) = mesh.uvs.and_then(block) {
        let accessor = push_accessor(root, buffer, &uvs, None);
        attributes.insert(Valid(Semantic::TexCoords(0)), accessor);
    }
    if let Some(tangents) = mesh.tangents.and_then(block) {
        let accessor = push_accessor(root, buffer, &tangents, None);
        attributes.insert(Valid(Semantic::Tangents), accessor);
    }
    push_skin_attributes(root, buffer, layout, mesh, &mut attributes);
    let colours = mesh
        .colours
        .and_then(block)
        .map(|colours| push_accessor(root, buffer, &colours, None));
    let targets = push_targets(root, buffer, layout, mesh);
    let primitives = mesh
        .primitives
        .iter()
        .filter_map(|primitive| {
            let indices = block(primitive.indices)?;
            let mut attributes = attributes.clone();
            if let Some(colours) = colours.filter(|_| primitive.colours) {
                attributes.insert(Valid(Semantic::Colors(0)), colours);
            }
            Some(Primitive {
                attributes,
                indices: Some(push_accessor(root, buffer, &indices, None)),
                material: materials.and_then(|ids| ids.get(&primitive.material_id).copied()),
                mode: Valid(Mode::Triangles),
                targets: primitive_targets(&targets),
                extensions: None,
                extras: json::extras::Extras::default(),
            })
        })
        .collect();
    root.push(json::Mesh {
        name,
        primitives,
        weights: (!targets.is_empty()).then(|| vec![0.0; targets.len()]),
        extensions: None,
        extras: mesh_extras(mesh),
    })
}

type Attributes = BTreeMap<json::validation::Checked<Semantic>, Index<Accessor>>;

/// Adds the `JOINTS_n` and `WEIGHTS_n` accessors of the mesh's influence sets to `attributes`.
fn push_skin_attributes(
    root: &mut Root,
    buffer: Index<json::Buffer>,
    layout: &Layout<'_>,
    mesh: &MeshLayout,
    attributes: &mut Attributes,
) {
    let block = |index: usize| layout.blocks.get(index).copied();
    for (set, (&joints, &weights)) in mesh.joints.iter().zip(&mesh.weights).enumerate() {
        let set = u32::try_from(set).unwrap_or(u32::MAX);
        if let (Some(joints), Some(weights)) = (block(joints), block(weights)) {
            let accessor = push_accessor(root, buffer, &joints, None);
            attributes.insert(Valid(Semantic::Joints(set)), accessor);
            let accessor = push_accessor(root, buffer, &weights, None);
            attributes.insert(Valid(Semantic::Weights(set)), accessor);
        }
    }
}

/// Writes the buffer view and accessor of `block`; `bounds` are the accessor's `min` and `max`.
pub(crate) fn push_accessor(
    root: &mut Root,
    buffer: Index<json::Buffer>,
    block: &Block<'_>,
    bounds: Option<(&[f32], &[f32])>,
) -> Index<Accessor> {
    let (component, kind, target) = block.data.layout();
    let view = root.push(json::buffer::View {
        buffer,
        byte_length: USize64(block.len),
        byte_offset: Some(USize64(block.offset)),
        byte_stride: None,
        target: target.map(Valid),
        name: None,
        extensions: None,
        extras: json::extras::Extras::default(),
    });
    let bound = |values: &[f32]| {
        json::Value::from(values.iter().copied().map(f64::from).collect::<Vec<_>>())
    };
    root.push(Accessor {
        buffer_view: Some(view),
        byte_offset: None,
        count: USize64(block.data.count() as u64),
        component_type: Valid(GenericComponentType(component)),
        type_: Valid(kind),
        min: bounds.map(|(min, _)| bound(min)),
        max: bounds.map(|(_, max)| bound(max)),
        normalized: matches!(block.data, BlockData::Colours(_) | BlockData::Weights(..)),
        sparse: None,
        name: None,
        extensions: None,
        extras: json::extras::Extras::default(),
    })
}
