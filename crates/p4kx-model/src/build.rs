// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ChunkFile, ChunkType, ivo};

use crate::bone_map::check_joints;
use crate::build_lod::read_lod_companions;
use crate::material::material_library;
use crate::mesh::split_by_node;
use crate::model::MESH_TYPES;
use crate::mtl_load::load;
use crate::nmc::read_nodes;
use crate::skeleton::{read_bones, read_compact_bones};
use crate::skin::read_skin;
use crate::{
    Bone, ChunkUsage, CompanionPath, CompanionResolver, CorruptChunk, LayerCache, Lod,
    LodSelection, MaterialReading, Mesh, Model, ModelChunk, ModelError, ModelFile, ModelIssue,
    Node, PositionBox, RigLogicBytes, Transform,
};

const MATERIAL_TYPES: [ChunkType; 2] = [ivo::MTL_NAME_320, ivo::MTL_NAME];

pub(crate) const MAX_LOD: u32 = 15;

pub(crate) type Companion = (String, ChunkFile<Vec<u8>>);

/// What the meshes of a model are checked against and read with: its node and bone counts and
/// the box that dequantises positions.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Rig<'a> {
    pub(crate) nodes: Option<usize>,
    pub(crate) bones: &'a [Bone],
    pub(crate) position_box: PositionBox,
}

pub(crate) fn read_skeleton<B: AsRef<[u8]>>(
    name: &str,
    file: &ChunkFile<B>,
    read: &mut Vec<(ModelFile, usize)>,
) -> Result<Vec<Bone>, ModelError> {
    let Some((index, chunk)) = [ivo::COMPILED_BONES_2, ivo::COMPILED_BONES]
        .into_iter()
        .find_map(|chunk_type| file.chunks_of(chunk_type).next())
    else {
        return Ok(Vec::new());
    };
    read.push((ModelFile::Model, index));
    let data = file.chunk_data(index).unwrap_or_default();
    let (chunk_name, bones) = if chunk.chunk_type() == ivo::COMPILED_BONES_2 {
        ("CompiledBones2", read_bones(data))
    } else {
        ("CompiledBones", read_compact_bones(data))
    };
    bones.map_err(corrupt(name, index, chunk_name))
}

pub(crate) fn build<B: AsRef<[u8]>>(
    name: &str,
    file: &ChunkFile<B>,
    companions: &mut dyn CompanionResolver,
    selection: LodSelection,
    materials: MaterialReading,
    position_box: PositionBox,
) -> Result<Model, ModelError> {
    let mut read = Vec::new();
    let nmc = match file.chunks_of(ivo::NODE_MESH_COMBO).next() {
        Some((index, _)) => {
            read.push((ModelFile::Model, index));
            let data = file.chunk_data(index).unwrap_or_default();
            Some(read_nodes(data).map_err(corrupt(name, index, "NodeMeshCombo"))?)
        }
        None => None,
    }
    .filter(|nmc| !nmc.nodes.is_empty());
    let (nodes, material_indices) = nmc.map_or((None, Vec::new()), |nmc| {
        (Some(nmc.nodes), nmc.material_indices)
    });
    let bones = read_skeleton(name, file, &mut read)?;
    let rig = Rig {
        nodes: nodes.as_ref().map(Vec::len),
        bones: &bones,
        position_box,
    };
    let material = first_of(file, &MATERIAL_TYPES).map(|index| {
        read.push((ModelFile::Model, index));
        material_library(file.chunk_data(index).unwrap_or_default())
    });

    let rig_logic = first_of(file, &[ivo::RIG_LOGIC]).and_then(|index| {
        read.push((ModelFile::Model, index));
        file.chunk_data(index).map(RigLogicBytes::new)
    });

    let Geometry {
        lods,
        base_name,
        chunks: companion_chunks,
        issues,
    } = read_geometry(name, file, companions, selection, rig, &mut read)?;

    let mut chunks = usage(file, ModelFile::Model, &read);
    chunks.extend(companion_chunks);
    let material_library = material.flatten();
    let materials = load(
        material_library.as_deref(),
        companions,
        materials,
        &LayerCache::new(),
    );
    Ok(Model {
        name: name.to_owned(),
        geometry: base_name,
        nodes: nodes.unwrap_or_else(|| vec![root_node(name)]),
        lods,
        material_library,
        materials,
        bones,
        material_indices,
        issues,
        chunks,
        rig_logic,
    })
}

/// The meshes of a model, the name of its geometry companion, the chunk usage of every
/// companion, computed as each was read so that no companion's bytes outlive its reading, and
/// what a lenient read worked around.
pub(crate) struct Geometry {
    pub(crate) lods: Vec<Lod>,
    pub(crate) base_name: Option<String>,
    pub(crate) chunks: Vec<ModelChunk>,
    pub(crate) issues: Vec<ModelIssue>,
}

fn read_geometry<B: AsRef<[u8]>>(
    name: &str,
    file: &ChunkFile<B>,
    companions: &mut dyn CompanionResolver,
    selection: LodSelection,
    rig: Rig<'_>,
    read: &mut Vec<(ModelFile, usize)>,
) -> Result<Geometry, ModelError> {
    let own_mesh = first_of(file, &MESH_TYPES);
    let mut issues = Vec::new();
    let base = if own_mesh.is_some() || !selection.wants(0) {
        None
    } else {
        geometry_companion(name, file, companions, selection, &mut issues)?
    };
    let mut lods = Vec::new();
    let mut chunks = Vec::new();
    let mut geometry = None;
    if selection.wants(0) {
        lods.push(read_base_lod(
            name,
            file,
            base.as_ref(),
            own_mesh,
            rig,
            read,
        )?);
    }
    if let Some((companion_name, companion)) = base {
        chunks.extend(usage(&companion, ModelFile::Geometry, read));
        geometry = Some(companion_name);
    }
    let mut found = Geometry {
        lods,
        base_name: geometry,
        chunks,
        issues,
    };
    if own_mesh.is_none() && announces_mesh(file) {
        read_lod_companions(name, companions, selection, rig, read, &mut found)?;
    }
    if let LodSelection::OnlyOrBase(level) = selection
        && found.lods.is_empty()
    {
        read_fallback_lod(name, file, companions, level, rig, read, &mut found)?;
    }
    if let LodSelection::Only(level) | LodSelection::AtMost(level) = selection
        && found.lods.is_empty()
    {
        return Err(ModelError::NoSuchLod {
            file: name.to_owned(),
            lod: level,
        });
    }
    Ok(found)
}

/// Reads the meshes of level 0 as level `level`, which has no companion, and reports it.
fn read_fallback_lod<B: AsRef<[u8]>>(
    name: &str,
    file: &ChunkFile<B>,
    companions: &mut dyn CompanionResolver,
    level: u32,
    rig: Rig<'_>,
    read: &mut Vec<(ModelFile, usize)>,
    found: &mut Geometry,
) -> Result<(), ModelError> {
    let own_mesh = first_of(file, &MESH_TYPES);
    let base = if own_mesh.is_some() {
        None
    } else {
        geometry_companion(
            name,
            file,
            companions,
            LodSelection::OnlyOrBase(level),
            &mut found.issues,
        )?
    };
    let mut lod = read_base_lod(name, file, base.as_ref(), own_mesh, rig, read)?;
    lod.level = level;
    found.lods.push(lod);
    let source = base
        .as_ref()
        .map(|(companion_name, _)| companion_name.clone());
    if let Some(file) = source.or_else(|| own_mesh.map(|_| name.to_owned())) {
        found.issues.push(ModelIssue::LodFallback { level, file });
    }
    if let Some((companion_name, companion)) = base {
        found
            .chunks
            .extend(usage(&companion, ModelFile::Geometry, read));
        found.base_name = Some(companion_name);
    }
    Ok(())
}

fn read_base_lod<B: AsRef<[u8]>>(
    name: &str,
    file: &ChunkFile<B>,
    base: Option<&Companion>,
    own_mesh: Option<usize>,
    rig: Rig<'_>,
    read: &mut Vec<(ModelFile, usize)>,
) -> Result<Lod, ModelError> {
    let meshes = match (base, own_mesh) {
        (Some((companion_name, companion)), _) => match first_of(companion, &MESH_TYPES) {
            Some(index) => {
                read.push((ModelFile::Geometry, index));
                read_meshes(
                    companion_name,
                    index,
                    companion.chunk_data(index),
                    rig,
                    true,
                )?
            }
            None if companion.chunks_of(ivo::VIS_AREAS).next().is_some() => Vec::new(),
            None => {
                return Err(ModelError::NoMesh {
                    file: companion_name.clone(),
                });
            }
        },
        (None, Some(index)) => {
            read.push((ModelFile::Model, index));
            let data = file.chunk_data(index);
            read_meshes(name, index, data, rig, true)?
        }
        (None, None) => Vec::new(),
    };
    Ok(Lod {
        level: 0,
        meshes,
        bones: None,
    })
}

fn announces_mesh<B: AsRef<[u8]>>(file: &ChunkFile<B>) -> bool {
    file.chunks_of(ivo::MESH_INFO).next().is_some()
}

fn geometry_companion<B: AsRef<[u8]>>(
    name: &str,
    file: &ChunkFile<B>,
    companions: &mut dyn CompanionResolver,
    selection: LodSelection,
    issues: &mut Vec<ModelIssue>,
) -> Result<Option<Companion>, ModelError> {
    let companion_name = format!("{name}m");
    let announced = file.chunks_of(ivo::MESH_INFO).next().is_some();
    match fetch(companions, &companion_name)? {
        Some(companion) => Ok(Some((companion_name, companion))),
        None if announced && selection.is_lenient() => {
            issues.push(ModelIssue::MissingGeometry {
                file: companion_name,
            });
            Ok(None)
        }
        None if announced => Err(ModelError::MissingGeometry {
            file: companion_name,
        }),
        None => Ok(None),
    }
}

pub(crate) fn read_meshes(
    file: &str,
    index: usize,
    data: Option<&[u8]>,
    rig: Rig<'_>,
    check_bones: bool,
) -> Result<Vec<Mesh>, ModelError> {
    let corrupt = corrupt(file, index, "Skin");
    let skin = read_skin(data.unwrap_or_default(), rig.position_box).map_err(&corrupt)?;
    let meshes = split_by_node(&skin, rig.nodes).map_err(&corrupt)?;
    if check_bones {
        check_joints(&meshes, rig.bones.len()).map_err(&corrupt)?;
    }
    Ok(meshes)
}

pub(crate) fn fetch(
    companions: &mut dyn CompanionResolver,
    name: &str,
) -> Result<Option<ChunkFile<Vec<u8>>>, ModelError> {
    let bytes = companions
        .resolve(CompanionPath::Sibling(name))
        .map_err(|source| ModelError::Companion {
            file: name.to_owned(),
            source,
        })?;
    bytes
        .map(ChunkFile::parse)
        .transpose()
        .map_err(|source| ModelError::CompanionChunks {
            file: name.to_owned(),
            source,
        })
}

pub(crate) fn first_of<B: AsRef<[u8]>>(file: &ChunkFile<B>, types: &[ChunkType]) -> Option<usize> {
    types
        .iter()
        .find_map(|&chunk_type| file.chunks_of(chunk_type).next())
        .map(|(index, _)| index)
}

pub(crate) fn usage<B: AsRef<[u8]>>(
    file: &ChunkFile<B>,
    which: ModelFile,
    read: &[(ModelFile, usize)],
) -> Vec<ModelChunk> {
    file.chunks()
        .iter()
        .enumerate()
        .map(|(index, chunk)| {
            let chunk_type = chunk.chunk_type();
            let usage = if read.contains(&(which, index)) {
                ChunkUsage::Read
            } else if chunk_type.is_known() {
                ChunkUsage::Skipped
            } else {
                ChunkUsage::Unknown
            };
            ModelChunk {
                file: which,
                index,
                chunk_type,
                usage,
            }
        })
        .collect()
}

pub(crate) fn root_node(name: &str) -> Node {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    Node::new(stem.to_owned(), None, Transform::IDENTITY)
}

pub(crate) fn corrupt(
    file: &str,
    index: usize,
    chunk: &'static str,
) -> impl Fn(CorruptChunk) -> ModelError + use<> {
    let file = file.to_owned();
    move |source| ModelError::Corrupt {
        file: file.clone(),
        index,
        chunk,
        source,
    }
}
