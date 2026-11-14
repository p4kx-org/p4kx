// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ChunkFile, ChunkType, ivo};

use crate::bone_map::check_joints;
use crate::build_lod::read_lod_companions;
use crate::material::{MaterialName, material_library};
use crate::mesh::split_by_node;
use crate::model::MESH_TYPES;
use crate::model_node_sources::ReadCapture;
use crate::mtl_load::load;
use crate::nmc;
use crate::skeleton_source;
use crate::skin::read_skin;
use crate::{
    Bone, ChunkUsage, CompanionResolver, CorruptChunk, LayerCache, Lod, LodSelection,
    LodSkeletonOrigin, MaterialReading, Mesh, MeshOrigin, Model, ModelBoneSources, ModelChunk,
    ModelChunkLayout, ModelError, ModelFile, ModelIssue, Node, PositionBox, RigLogicBytes,
    Transform,
};

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

pub(crate) fn build<B: AsRef<[u8]>>(
    name: &str,
    file: &ChunkFile<B>,
    companions: &mut dyn CompanionResolver,
    selection: LodSelection,
    materials: MaterialReading,
    position_box: PositionBox,
    mut capture: ReadCapture<'_>,
) -> Result<Model, ModelError> {
    let mut read = Vec::new();
    let nmc = nmc::read_file_nodes(file, &mut read, capture.nodes)
        .map_err(|error| corrupt(name, error.index, "NodeMeshCombo")(error.source))?;
    let (nodes, material_indices) = nmc.map_or((None, Vec::new()), |nmc| {
        (Some(nmc.nodes), nmc.material_indices)
    });
    let bones = skeleton_source::read_model_skeleton(
        name,
        file,
        ModelFile::Model,
        &mut read,
        capture.skeletons.as_deref_mut(),
        capture.primary_skeleton,
    )?;
    let rig = Rig {
        nodes: nodes.as_ref().map(Vec::len),
        bones: &bones,
        position_box,
    };
    let material_library = MaterialName::primary(file.chunks()).and_then(|index| {
        read.push((ModelFile::Model, index));
        file.chunk_data(index).and_then(material_library)
    });
    let rig_logic = read_primary_chunk(file, &[ivo::RIG_LOGIC], &mut read).map(RigLogicBytes::new);

    let Geometry {
        lods,
        base_name,
        chunks: companion_chunks,
        issues,
    } = read_geometry(
        name,
        file,
        companions,
        selection,
        rig,
        &mut read,
        capture.skeletons,
    )?;

    let mut chunks = usage(file, ModelFile::Model, &read);
    chunks.extend(companion_chunks);
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

fn read_primary_chunk<'a, B: AsRef<[u8]>>(
    file: &'a ChunkFile<B>,
    types: &[ChunkType],
    read: &mut Vec<(ModelFile, usize)>,
) -> Option<&'a [u8]> {
    let index = first_of(file, types)?;
    read.push((ModelFile::Model, index));
    file.chunk_data(index)
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
    skeleton_sources: Option<&mut Vec<ModelBoneSources>>,
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
        read_lod_companions(
            name,
            companions,
            selection,
            rig,
            read,
            &mut found,
            skeleton_sources,
        )?;
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
                    MeshOrigin::new(ModelFile::Geometry, index, 0),
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
            read_meshes(
                name,
                MeshOrigin::new(ModelFile::Model, index, 0),
                data,
                rig,
                true,
            )?
        }
        (None, None) => Vec::new(),
    };
    Ok(Lod {
        level: 0,
        meshes,
        bones: None,
        skeleton_origin: LodSkeletonOrigin::ModelFile,
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
    match fetch(companions, ModelFile::Geometry, &companion_name)? {
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
    origin: MeshOrigin,
    data: Option<&[u8]>,
    rig: Rig<'_>,
    check_bones: bool,
) -> Result<Vec<Mesh>, ModelError> {
    let corrupt = corrupt(file, origin.chunk, "Skin");
    let skin = read_skin(data.unwrap_or_default(), rig.position_box).map_err(&corrupt)?;
    let mut meshes = split_by_node(&skin, rig.nodes).map_err(&corrupt)?;
    if check_bones {
        check_joints(&meshes, rig.bones.len()).map_err(&corrupt)?;
    }
    for mesh in &mut meshes {
        mesh.origin = Some(origin);
    }
    Ok(meshes)
}

pub(crate) fn fetch(
    companions: &mut dyn CompanionResolver,
    file: ModelFile,
    name: &str,
) -> Result<Option<ChunkFile<Vec<u8>>>, ModelError> {
    let bytes = companions
        .resolve_model_file(file, name)
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
                layout: ModelChunkLayout::of(chunk),
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
