// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use gltf_json::Root;

use crate::animations::{AnimationPlan, ChannelPath};
use crate::glb::WrittenGlb;
use crate::instance::{Instance, MaterialIds};
use crate::layout::{Layout, MeshLayout};
use crate::materials::Slot;

/// The writer input that supplied a rigged GLB datum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RiggedBufferSource {
    /// A model's mesh attributes and triangles.
    Geometry(usize),
    /// The positions and tangent frame of a model's surface.
    Surface(usize),
    /// A model's weights and remapped joint indices.
    Skin(usize),
    /// A model's sparse morph target data.
    Morph(usize),
    /// The shared skeleton's inverse bind matrices.
    Skeleton,
    /// The times of the prepared animation at this input index.
    AnimationTimes(usize),
    /// The pose data of the prepared animation at this input index.
    AnimationPose(usize),
    /// The morph weights of a prepared animation bound to a part.
    AnimationWeights {
        /// The prepared animation index.
        clip: usize,
        /// The model part index.
        part: usize,
    },
    /// The embedded PNG at this image index.
    Image(usize),
    /// The producer did not supply an identifiable input.
    Unknown,
}

/// A buffer view written into the GLB binary chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiggedBufferView {
    /// The buffer view's index in the JSON chunk.
    pub index: usize,
    /// The first byte of the view in the GLB file.
    pub start: u64,
    /// The first byte past the view in the GLB file.
    pub end_exclusive: u64,
    /// The input that supplied the view.
    pub source: RiggedBufferSource,
}

/// An authored value written into the GLB JSON chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RiggedJsonAddress {
    /// An embedded image by its output index.
    Image(usize),
    /// A glTF texture and the image it refers to.
    Texture {
        /// The GLB texture index.
        index: usize,
        /// The GLB image index.
        image: usize,
    },
    /// A node supplied by one original model part.
    Node {
        /// The GLB node index.
        index: usize,
        /// The originating model part.
        part: usize,
    },
    /// A bone node by its GLB node index.
    Bone(usize),
    /// An authored material and the actual part and slot that supplied it.
    Material {
        /// The GLB material index.
        index: usize,
        /// The originating model part.
        part: usize,
        /// The originating library slot, or none for the built-in default.
        slot: Option<usize>,
    },
    /// A mesh by its output and part indices.
    Mesh {
        /// The GLB mesh index.
        index: usize,
        /// The originating model part.
        part: usize,
    },
    /// A shared skeleton by its output skin index.
    Skin(usize),
    /// An emitted animation by its output and prepared input indices.
    Animation {
        /// The GLB animation index.
        index: usize,
        /// The prepared animation index.
        clip: usize,
    },
    /// An accessor by its output index and buffer view source.
    Accessor {
        /// The GLB accessor index.
        index: usize,
        /// The source of its binary values.
        source: RiggedBufferSource,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Addresses {
    pub(crate) views: Vec<RiggedBufferView>,
    pub(crate) json: Vec<RiggedJsonAddress>,
}

type Owners = HashMap<(u64, u64), RiggedBufferSource>;

impl Addresses {
    pub(crate) fn of(
        document: &Root,
        layout: &Layout<'_>,
        instances: &[Instance<'_>],
        materials: &[MaterialIds],
        plans: &[AnimationPlan],
        written: &WrittenGlb,
    ) -> Self {
        let mut addresses = Self::default();
        if let Some(binary_start) = written.binary_start {
            addresses.views = binary_views(
                document,
                &owners(layout, instances, plans),
                binary_start,
                written.len,
            );
        }
        addresses.json = json_addresses(document, layout, plans, &addresses.views);
        addresses
            .json
            .extend(scene_addresses(document, instances, materials));
        addresses
    }
}

fn binary_views(
    document: &Root,
    owners: &Owners,
    binary_start: u64,
    length: u64,
) -> Vec<RiggedBufferView> {
    document
        .buffer_views
        .iter()
        .enumerate()
        .filter_map(|(index, view)| {
            let relative = view.byte_offset.map_or(0, |value| value.0);
            let start = binary_start.checked_add(relative)?;
            let end_exclusive = start.checked_add(view.byte_length.0)?;
            if view.buffer.value() != 0 || start >= end_exclusive || end_exclusive > length {
                return None;
            }
            let source = owners
                .get(&(relative, view.byte_length.0))
                .copied()
                .unwrap_or(RiggedBufferSource::Unknown);
            Some(RiggedBufferView {
                index,
                start,
                end_exclusive,
                source,
            })
        })
        .collect()
}

fn owners(layout: &Layout<'_>, instances: &[Instance<'_>], plans: &[AnimationPlan]) -> Owners {
    let mut found = Owners::new();
    for mesh in &layout.meshes {
        mesh_owners(&mut found, layout, mesh);
    }
    if let Some(block) = layout.inverse_binds {
        add_block(&mut found, layout, block, RiggedBufferSource::Skeleton);
    }
    let mut end = 0;
    let node_ends: Vec<_> = instances
        .iter()
        .map(|instance| {
            end += instance.model.nodes.len();
            end
        })
        .collect();
    for (clip, plan) in plans.iter().enumerate() {
        for channel in &plan.channels {
            let source = match channel.path {
                ChannelPath::Weights => {
                    let part = node_ends.partition_point(|&end| end <= channel.node);
                    if part < node_ends.len() {
                        RiggedBufferSource::AnimationWeights { clip, part }
                    } else {
                        RiggedBufferSource::Unknown
                    }
                }
                _ => RiggedBufferSource::AnimationPose(clip),
            };
            add_block(
                &mut found,
                layout,
                channel.times,
                RiggedBufferSource::AnimationTimes(clip),
            );
            add_block(&mut found, layout, channel.values, source);
        }
    }
    for (index, image) in layout.images.iter().enumerate() {
        found.insert((image.offset, image.len), RiggedBufferSource::Image(index));
    }
    found
}

fn mesh_owners(owners: &mut Owners, layout: &Layout<'_>, mesh: &MeshLayout) {
    for index in [Some(mesh.positions), mesh.normals, mesh.tangents]
        .into_iter()
        .flatten()
    {
        add_block(
            owners,
            layout,
            index,
            RiggedBufferSource::Surface(mesh.part),
        );
    }
    for index in [mesh.uvs, mesh.colours]
        .into_iter()
        .flatten()
        .chain(mesh.primitives.iter().map(|primitive| primitive.indices))
    {
        add_block(
            owners,
            layout,
            index,
            RiggedBufferSource::Geometry(mesh.part),
        );
    }
    for &index in mesh.joints.iter().chain(&mesh.weights) {
        add_block(owners, layout, index, RiggedBufferSource::Skin(mesh.part));
    }
    for morph in &mesh.morphs {
        for index in [morph.indices, morph.deltas] {
            add_block(owners, layout, index, RiggedBufferSource::Morph(mesh.part));
        }
    }
}

fn add_block(owners: &mut Owners, layout: &Layout<'_>, index: usize, source: RiggedBufferSource) {
    if let Some(block) = layout.blocks.get(index).filter(|block| block.len > 0) {
        owners.insert((block.offset, block.len), source);
    }
}

fn json_addresses(
    document: &Root,
    layout: &Layout<'_>,
    plans: &[AnimationPlan],
    views: &[RiggedBufferView],
) -> Vec<RiggedJsonAddress> {
    let mut addresses = Vec::new();
    addresses.extend((0..document.images.len()).map(RiggedJsonAddress::Image));
    addresses.extend(
        document
            .textures
            .iter()
            .enumerate()
            .map(|(index, texture)| RiggedJsonAddress::Texture {
                index,
                image: texture.source.value(),
            }),
    );
    addresses.extend((0..document.skins.len()).map(RiggedJsonAddress::Skin));
    addresses.extend(document.meshes.iter().zip(&layout.meshes).enumerate().map(
        |(index, (_, mesh))| RiggedJsonAddress::Mesh {
            index,
            part: mesh.part,
        },
    ));
    addresses.extend(
        plans
            .iter()
            .enumerate()
            .filter(|(_, plan)| !plan.channels.is_empty())
            .zip(&document.animations)
            .enumerate()
            .map(|(index, ((clip, _), _))| RiggedJsonAddress::Animation { index, clip }),
    );
    let by_view: HashMap<_, _> = views.iter().map(|view| (view.index, view.source)).collect();
    for (index, accessor) in document.accessors.iter().enumerate() {
        let view = accessor.buffer_view.or_else(|| {
            accessor
                .sparse
                .as_ref()
                .map(|sparse| sparse.values.buffer_view)
        });
        if let Some(source) = view.and_then(|view| by_view.get(&view.value())) {
            addresses.push(RiggedJsonAddress::Accessor {
                index,
                source: *source,
            });
        }
    }
    addresses
}

fn scene_addresses(
    document: &Root,
    instances: &[Instance<'_>],
    materials: &[MaterialIds],
) -> Vec<RiggedJsonAddress> {
    let mut found = Vec::new();
    let mut next_node = 0;
    for (part, instance) in instances.iter().enumerate() {
        let end = (next_node + instance.model.nodes.len()).min(document.nodes.len());
        found.extend((next_node..end).map(|index| RiggedJsonAddress::Node { index, part }));
        next_node = end;
    }
    found.extend((next_node..document.nodes.len()).map(RiggedJsonAddress::Bone));
    let mut authors = vec![None; document.materials.len()];
    for (part, (instance, ids)) in instances.iter().zip(materials).enumerate() {
        for (&id, &index) in ids {
            let Some(author) = authors.get_mut(index.value()) else {
                continue;
            };
            if author.is_none()
                && let Some(slot) = instance.plan.slot(id)
            {
                let slot = match slot {
                    Slot::Library(slot) => Some(slot),
                    Slot::Default => None,
                };
                *author = Some(RiggedJsonAddress::Material {
                    index: index.value(),
                    part,
                    slot,
                });
            }
        }
    }
    found.extend(authors.into_iter().flatten());
    found
}
