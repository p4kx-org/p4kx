// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ByteOrder, ChunkFile, ChunkType, crch};

use crate::build::{root_node, usage};
use crate::crch_mesh::{physics_slots, read_physics_mesh};
use crate::crch_node::{CrchNode, read_nodes};
use crate::error::failure;
use crate::material::material_library;
use crate::mtl_load::load;
use crate::{
    CompanionResolver, CorruptChunk, LayerCache, Lod, LodSelection, MaterialReading, Mesh, Model,
    ModelError, ModelFile, ModelIssue, UnsupportedLayout,
};

const MESH_VERSION: u32 = 0x801;
const PHYSICS_DATA_VERSION: u32 = 0x800;
const MTL_NAME_VERSION: u32 = 0x802;

/// Builds the model of a `CrCh` file: its nodes, the meshes of the nodes that refer to a `Mesh`
/// chunk and the material library its `MtlName` chunk names. A `CrCh` model has one level of
/// detail and no geometry companion.
pub(crate) fn build<B: AsRef<[u8]>>(
    name: &str,
    file: &ChunkFile<B>,
    companions: &mut dyn CompanionResolver,
    selection: LodSelection,
    materials: MaterialReading,
) -> Result<Model, ModelError> {
    let mut read = Vec::new();
    let crch_nodes = read_nodes(name, file, &mut read)?;
    let mut lods = Vec::new();
    let mut issues = Vec::new();
    if selection.wants(0) || selection.is_lenient() {
        let meshes = read_meshes(name, file, &crch_nodes, &mut read, &mut issues)?;
        let level = match selection {
            LodSelection::OnlyOrBase(level) if level != 0 => {
                issues.push(ModelIssue::LodFallback {
                    level,
                    file: name.to_owned(),
                });
                level
            }
            _ => 0,
        };
        lods.push(Lod {
            level,
            meshes,
            bones: None,
        });
    }
    if let LodSelection::Only(level) = selection
        && lods.is_empty()
    {
        return Err(ModelError::NoSuchLod {
            file: name.to_owned(),
            lod: level,
        });
    }

    let library = first_of_version(file, crch::MTL_NAME, MTL_NAME_VERSION).and_then(|index| {
        read.push((ModelFile::Model, index));
        material_library(file.chunk_data(index).unwrap_or_default())
    });
    let materials = load(
        library.as_deref(),
        companions,
        materials,
        &LayerCache::new(),
    );
    let mut nodes: Vec<_> = crch_nodes.into_iter().map(|crch| crch.node).collect();
    if nodes.is_empty() {
        nodes.push(root_node(name));
    }
    Ok(Model {
        name: name.to_owned(),
        geometry: None,
        nodes,
        lods,
        material_library: library,
        materials,
        bones: Vec::new(),
        material_indices: Vec::new(),
        issues,
        chunks: usage(file, ModelFile::Model, &read),
        rig_logic: None,
    })
}

fn read_meshes<B: AsRef<[u8]>>(
    name: &str,
    file: &ChunkFile<B>,
    nodes: &[CrchNode],
    read: &mut Vec<(ModelFile, usize)>,
    issues: &mut Vec<ModelIssue>,
) -> Result<Vec<Mesh>, ModelError> {
    let mut meshes = Vec::new();
    for (position, node) in nodes.iter().enumerate() {
        if let Some(mesh) = read_node_mesh(name, file, position, node, read, issues)? {
            meshes.push(mesh);
        }
    }
    Ok(meshes)
}

fn first_of_version<B: AsRef<[u8]>>(
    file: &ChunkFile<B>,
    chunk_type: ChunkType,
    version: u32,
) -> Option<usize> {
    file.chunks_of(chunk_type)
        .find(|(_, chunk)| chunk.version() == version)
        .map(|(index, _)| index)
}

fn read_node_mesh<B: AsRef<[u8]>>(
    name: &str,
    file: &ChunkFile<B>,
    position: usize,
    node: &CrchNode,
    read: &mut Vec<(ModelFile, usize)>,
    issues: &mut Vec<ModelIssue>,
) -> Result<Option<Mesh>, ModelError> {
    let Some((mesh_index, mesh_chunk)) = file.chunk_by_id(node.object) else {
        return Ok(None);
    };
    if mesh_chunk.chunk_type() != crch::MESH {
        return Ok(None);
    }
    if mesh_chunk.version() != MESH_VERSION {
        return Err(failure(name, mesh_index, "Mesh")(
            UnsupportedLayout::MeshVersion(mesh_chunk.version()).into(),
        ));
    }
    let fail = failure(name, mesh_index, "Mesh");
    if mesh_chunk.byte_order() != ByteOrder::Little {
        return Err(fail(UnsupportedLayout::BigEndian.into()));
    }
    read.push((ModelFile::Model, mesh_index));
    let slots = physics_slots(file.chunk_data(mesh_index).unwrap_or_default()).map_err(&fail)?;
    if slots.ignored > 0 {
        issues.push(ModelIssue::PhysicsSlotsIgnored {
            node: position,
            slots: slots.ignored,
        });
    }
    let Some(physics_id) = slots.chunk_id else {
        return Ok(None);
    };

    let physics_index = match file.chunk_by_id(physics_id) {
        Some((index, chunk)) if chunk.chunk_type() == crch::MESH_PHYSICS_DATA => {
            if chunk.version() != PHYSICS_DATA_VERSION {
                return Err(failure(name, index, "MeshPhysicsData")(
                    UnsupportedLayout::PhysicsVersion(chunk.version()).into(),
                ));
            }
            index
        }
        _ => return Err(fail(CorruptChunk::ReferencedChunk(physics_id).into())),
    };
    let fail = failure(name, physics_index, "MeshPhysicsData");
    if file
        .chunks()
        .get(physics_index)
        .is_none_or(|chunk| chunk.byte_order() != ByteOrder::Little)
    {
        return Err(fail(UnsupportedLayout::BigEndian.into()));
    }
    read.push((ModelFile::Model, physics_index));
    read_physics_mesh(position, file.chunk_data(physics_index).unwrap_or_default()).map_err(fail)
}
